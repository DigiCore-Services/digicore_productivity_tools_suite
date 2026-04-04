//! KMS note embedding migration orchestration and embedding-policy diagnostics.

use std::path::PathBuf;

use crate::embedding_service;
use crate::kms_repository;

use super::*;

pub(crate) async fn kms_request_note_embedding_migration(host: ApiImpl) -> Result<u64, String> {
    let request_id = kms_request_id("embed_migration_request");
    let app = get_app(&host.app_handle);
    let (vault, model, batch, chunk_cfg) = {
        let g = host.state.lock().map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_STATE_LOCK",
                "KMS_EMBED_MIGRATION_STATE_LOCK_FAIL",
                "Failed to lock app state",
                Some(e.to_string()),
            )
        })?;
        let v = PathBuf::from(g.kms_vault_path.trim());
        if v.as_os_str().is_empty() {
            return Err(kms_ipc_error(
                &request_id,
                "KMS_VAULT_PATH_MISSING",
                "KMS_EMBED_MIGRATION_VAULT_PATH_MISSING",
                "KMS vault path is not set",
                None,
            ));
        }
        (
            v.clone(),
            embedding_service::normalized_embedding_model_id(&g.kms_embedding_model_id),
            g.kms_embedding_batch_notes_per_tick.max(1),
            crate::kms_graph_effective_params::effective_kms_embedding_chunk_config(&*g, &v),
        )
    };
    if !vault.exists() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_VAULT_NOT_FOUND",
            "KMS_EMBED_MIGRATION_VAULT_NOT_FOUND",
            "KMS vault folder does not exist",
            Some(vault.to_string_lossy().to_string()),
        ));
    }
    log::info!(
        "[KMS][Embed] event_code=KMS_EMBED_MIGRATION_REQUEST request_id={}",
        request_id
    );
    Ok(crate::kms_embedding_migrate::spawn_note_embedding_migration(
        &app,
        vault,
        model,
        batch,
        chunk_cfg,
        true,
    ))
}

pub(crate) async fn kms_cancel_note_embedding_migration(_host: ApiImpl) -> Result<(), String> {
    let request_id = kms_request_id("embed_migration_cancel");
    crate::kms_embedding_migrate::bump_embedding_migration_generation();
    log::info!(
        "[KMS][Embed] event_code=KMS_EMBED_MIGRATION_CANCEL request_id={}",
        request_id
    );
    Ok(())
}

pub(crate) async fn kms_get_embedding_policy_diagnostics(
    host: ApiImpl,
) -> Result<KmsEmbeddingPolicyDiagnosticsDto, String> {
    let request_id = kms_request_id("embed_policy_diag");
    let g = host.state.lock().map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_STATE_LOCK",
            "KMS_EMBED_POLICY_DIAG_STATE_LOCK_FAIL",
            "Failed to lock app state",
            Some(e.to_string()),
        )
    })?;
    let v = PathBuf::from(g.kms_vault_path.trim());
    if v.as_os_str().is_empty() {
        return Ok(KmsEmbeddingPolicyDiagnosticsDto {
            indexed_note_count: 0,
            stale_embedding_note_count: 0,
            expected_policy_signature: String::new(),
            total_notes_in_index: 0,
            pending_note_count: 0,
            failed_sync_note_count: 0,
            embedding_aligned_note_count: 0,
            other_sync_status_note_count: 0,
            vault_markdown_files_on_disk: 0,
            vault_all_files_on_disk: 0,
        });
    }
    let chunk_cfg = crate::kms_graph_effective_params::effective_kms_embedding_chunk_config(&*g, &v);
    let c = chunk_cfg.clamped();
    let mid = embedding_service::normalized_embedding_model_id(&g.kms_embedding_model_id);
    let expected = kms_repository::note_embedding_policy_sig(
        &mid,
        c.enabled,
        c.max_chars,
        c.overlap_chars,
        kms_repository::KMS_TEXT_EMBEDDING_VEC0_DIMENSIONS,
    );
    let (total_notes, indexed, pending, failed_sync) =
        kms_repository::count_kms_notes_sync_breakdown().map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_REPO_DIAG",
                "KMS_EMBED_POLICY_DIAG_SYNC_BREAKDOWN_FAIL",
                "Failed to read note sync breakdown",
                Some(e.to_string()),
            )
        })?;
    let stale =
        kms_repository::count_notes_needing_embedding_migration(&mid, &expected).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_REPO_DIAG",
                "KMS_EMBED_POLICY_DIAG_STALE_COUNT_FAIL",
                "Failed to count stale embeddings",
                Some(e.to_string()),
            )
        })?;
    let aligned = indexed.saturating_sub(stale);
    let other = total_notes
        .saturating_sub(indexed)
        .saturating_sub(pending)
        .saturating_sub(failed_sync);
    let (vault_all_files_on_disk, vault_markdown_files_on_disk) =
        kms_repository::count_vault_files_on_disk(&v).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_VAULT_SCAN",
                "KMS_EMBED_POLICY_DIAG_VAULT_SCAN_FAIL",
                "Failed to scan vault files on disk",
                Some(e.to_string()),
            )
        })?;
    Ok(KmsEmbeddingPolicyDiagnosticsDto {
        indexed_note_count: indexed,
        stale_embedding_note_count: stale,
        expected_policy_signature: expected,
        total_notes_in_index: total_notes,
        pending_note_count: pending,
        failed_sync_note_count: failed_sync,
        embedding_aligned_note_count: aligned,
        other_sync_status_note_count: other,
        vault_markdown_files_on_disk,
        vault_all_files_on_disk,
    })
}

pub(crate) async fn kms_get_embedding_diagnostic_log_path(
    _host: ApiImpl,
) -> Result<Option<String>, String> {
    Ok(crate::kms_embed_diagnostic_log::default_log_file_path()
        .map(|p| p.to_string_lossy().into_owned()))
}

