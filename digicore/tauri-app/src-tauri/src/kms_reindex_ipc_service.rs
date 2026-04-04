//! KMS reindex actions: full vault, per-provider, and single-note embedding sync.

use std::sync::Arc;

use tauri::Emitter;
use tauri::Manager;

use crate::embedding_service;
use crate::indexing_service;

use super::*;

pub(crate) async fn kms_reindex_all(host: ApiImpl) -> Result<(), String> {
    let app = get_app(&host.app_handle);
    let service = app
        .state::<Arc<indexing_service::KmsIndexingService>>()
        .inner()
        .clone();
    let request_id = kms_request_id("reindex_all");
    log::info!(
        "[KMS][Indexing] event_code=KMS_REINDEX_ALL_START request_id={}",
        request_id
    );
    service.spawn_index_all_providers(app.clone(), request_id);

    Ok(())
}

pub(crate) async fn kms_reindex_type(
    host: ApiImpl,
    provider_id: String,
) -> Result<u32, String> {
    let request_id = kms_request_id("reindex_type");
    let app = get_app(&host.app_handle);
    let service = app.state::<Arc<indexing_service::KmsIndexingService>>();

    match service.index_provider_by_id(&app, &provider_id).await {
        Ok(count) => {
            log::info!(
                "[KMS][Indexing] event_code=KMS_REINDEX_TYPE_OK request_id={} provider={} count={}",
                request_id,
                provider_id,
                count
            );
            Ok(count as u32)
        }
        Err(e) => Err(kms_ipc_error(
            &request_id,
            "KMS_REINDEX_TYPE",
            "KMS_REINDEX_TYPE_FAIL",
            "Failed to reindex provider",
            Some(format!("provider={} error={}", provider_id, e)),
        )),
    }
}

pub(crate) async fn kms_reindex_note(
    host: ApiImpl,
    rel_path: String,
) -> Result<(), String> {
    let request_id = kms_request_id("reindex_note");
    let abs_path = host.resolve_absolute_path(&rel_path);

    if !abs_path.exists() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_NOTE_NOT_FOUND",
            "KMS_REINDEX_NOTE_FILE_NOT_FOUND",
            "File not found on disk",
            Some(rel_path.clone()),
        ));
    }

    let current_title = abs_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    if let Err(e) = kms_repository::upsert_note(&rel_path, &current_title, "", "pending", None, &[]) {
        log::warn!(
            "[KMS][Indexing] event_code=KMS_REINDEX_NOTE_PENDING_UPSERT_WARN request_id={} path={} error={}",
            request_id,
            rel_path,
            e
        );
    }

    let app = get_app(&host.app_handle);
    if let Err(e) = app.emit("kms-sync-status", "Indexing...") {
        log::warn!(
            "[KMS][Indexing] event_code=KMS_REINDEX_NOTE_SYNC_STATUS_INDEXING_WARN request_id={} error={}",
            request_id,
            e
        );
    }
    if let Err(e) = app.emit("kms-sync-complete", ()) {
        log::warn!(
            "[KMS][Indexing] event_code=KMS_REINDEX_NOTE_SYNC_COMPLETE_PENDING_WARN request_id={} error={}",
            request_id,
            e
        );
    }

    let vault_pb = host.get_vault_path();
    let (embedding_model_id, chunk_cfg) = {
        let g = host.state.lock().map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_STATE_LOCK",
                "KMS_REINDEX_NOTE_STATE_LOCK_FAIL",
                "Failed to lock app state",
                Some(e.to_string()),
            )
        })?;
        (
            embedding_service::normalized_embedding_model_id(&g.kms_embedding_model_id),
            crate::kms_graph_effective_params::effective_kms_embedding_chunk_config(&*g, &vault_pb),
        )
    };

    match std::fs::read_to_string(&abs_path) {
        Ok(content) => {
            if let Err(e) = crate::kms_sync_orchestration::sync_note_index_internal(
                &rel_path,
                &current_title,
                &content,
                &embedding_model_id,
                chunk_cfg,
            )
            .await
            {
                log::error!("[KMS][Sync] Failed to reindex note {}: {}", rel_path, e);
                if let Err(upsert_err) =
                    kms_repository::upsert_note(&rel_path, &current_title, "", "failed", Some(&e), &[])
                {
                    log::warn!(
                        "[KMS][Indexing] event_code=KMS_REINDEX_NOTE_FAILED_UPSERT_WARN request_id={} path={} error={}",
                        request_id,
                        rel_path,
                        upsert_err
                    );
                }
            }
        }
        Err(e) => {
            if let Err(upsert_err) = kms_repository::upsert_note(
                &rel_path,
                &current_title,
                "",
                "failed",
                Some(&e.to_string()),
                &[],
            ) {
                log::warn!(
                    "[KMS][Indexing] event_code=KMS_REINDEX_NOTE_READ_FAIL_UPSERT_WARN request_id={} path={} error={}",
                    request_id,
                    rel_path,
                    upsert_err
                );
            }
        }
    }

    if let Err(e) = app.emit("kms-sync-complete", ()) {
        log::warn!(
            "[KMS][Indexing] event_code=KMS_REINDEX_NOTE_SYNC_COMPLETE_WARN request_id={} error={}",
            request_id,
            e
        );
    }
    if let Err(e) = app.emit("kms-sync-status", "Idle") {
        log::warn!(
            "[KMS][Indexing] event_code=KMS_REINDEX_NOTE_SYNC_STATUS_IDLE_WARN request_id={} error={}",
            request_id,
            e
        );
    }
    log::info!(
        "[KMS][Indexing] event_code=KMS_REINDEX_NOTE_OK request_id={} path={}",
        request_id,
        rel_path
    );
    Ok(())
}

