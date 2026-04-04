//! Bounded service for KMS indexing/diagnostics IPC orchestration.

use std::sync::Arc;

use tauri::Manager;

use crate::indexing_service;
use crate::kms_repository;

use super::ApiImpl;
use super::{get_app, kms_ipc_error, kms_request_id, IndexingStatusDto, KmsDiagnosticsDto, KmsIndexStatusRow};

pub(crate) async fn kms_get_indexing_status(_host: ApiImpl) -> Result<Vec<IndexingStatusDto>, String> {
    let request_id = kms_request_id("indexing_status");
    let mut results = Vec::new();
    let categories = ["notes", "snippets", "clipboard"];

    for cat in categories {
        let (indexed, failed, total) = match kms_repository::get_category_counts(cat) {
            Ok(counts) => counts,
            Err(e) => {
                log::warn!(
                    "[KMS][Indexing] event_code=KMS_INDEXING_STATUS_COUNTS_FAIL request_id={} category={} error={}. Returning zeros.",
                    request_id,
                    cat,
                    e
                );
                (0, 0, 0)
            }
        };

        let last_error = if failed > 0 {
            kms_repository::get_detailed_status(cat)
                .ok()
                .and_then(|details| details.first().and_then(|r| r.error.clone()))
        } else {
            None
        };

        results.push(IndexingStatusDto {
            category: cat.to_string(),
            indexed_count: indexed,
            failed_count: failed,
            total_count: total,
            last_error,
        });
    }

    log::debug!("[KMS][Indexing] event_code=KMS_INDEXING_STATUS_OK request_id={}", request_id);
    Ok(results)
}

pub(crate) async fn kms_get_indexing_details(
    _host: ApiImpl,
    provider_id: String,
) -> Result<Vec<KmsIndexStatusRow>, String> {
    let request_id = kms_request_id("indexing_details");
    let rows = kms_repository::get_detailed_status(&provider_id).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_INDEXING_DETAILS",
            "KMS_INDEXING_DETAILS_FAIL",
            "Failed to load indexing details",
            Some(format!("provider={} error={}", provider_id, e)),
        )
    })?;
    Ok(rows
        .into_iter()
        .map(|r| KmsIndexStatusRow {
            entity_type: r.entity_type,
            entity_id: r.entity_id,
            status: r.status,
            error: r.error,
            updated_at: r.updated_at,
        })
        .collect())
}

pub(crate) async fn kms_retry_item(
    host: ApiImpl,
    provider_id: String,
    entity_id: String,
) -> Result<(), String> {
    let request_id = kms_request_id("retry_item");
    let app = get_app(&host.app_handle);
    let service = app.state::<Arc<indexing_service::KmsIndexingService>>();

    service
        .index_single_item(&app, &provider_id, &entity_id)
        .await
        .map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_RETRY_ITEM",
                "KMS_RETRY_ITEM_FAIL",
                "Failed to retry indexing item",
                Some(format!(
                    "provider={} entity_id={} error={}",
                    provider_id, entity_id, e
                )),
            )
        })?;
    Ok(())
}

pub(crate) async fn kms_retry_failed(host: ApiImpl, provider_id: String) -> Result<(), String> {
    let request_id = kms_request_id("retry_failed");
    let app = get_app(&host.app_handle);
    let service = app.state::<Arc<indexing_service::KmsIndexingService>>();

    let failures = kms_repository::get_detailed_status(&provider_id).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_INDEXING_DETAILS",
            "KMS_RETRY_FAILED_LOAD_FAIL",
            "Failed to load failed indexing items",
            Some(format!("provider={} error={}", provider_id, e)),
        )
    })?;
    for fail in failures {
        if let Err(e) = service
            .index_single_item(&app, &provider_id, &fail.entity_id)
            .await
        {
            log::warn!(
                "[KMS][Indexing] event_code=KMS_RETRY_FAILED_ITEM_WARN request_id={} provider={} entity_id={} error={}",
                request_id,
                provider_id,
                fail.entity_id,
                e
            );
        }
    }
    log::info!(
        "[KMS][Indexing] event_code=KMS_RETRY_FAILED_OK request_id={} provider={}",
        request_id,
        provider_id
    );
    Ok(())
}

pub(crate) async fn kms_repair_database(_host: ApiImpl) -> Result<(), String> {
    let request_id = kms_request_id("repair_database");
    kms_repository::repair_database().map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPAIR_DB",
            "KMS_REPAIR_DB_FAIL",
            "Failed to repair KMS database",
            Some(e.to_string()),
        )
    })?;
    crate::kms_link_adjacency_cache::invalidate_kms_link_adjacency_cache();
    log::info!("[KMS][DB] event_code=KMS_REPAIR_DB_OK request_id={}", request_id);
    Ok(())
}

pub(crate) async fn kms_get_diagnostics(_host: ApiImpl) -> Result<KmsDiagnosticsDto, String> {
    let request_id = kms_request_id("diagnostics");
    let stats = kms_repository::get_diag_summary().map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_DIAG",
            "KMS_DIAGNOSTICS_FAIL",
            "Failed to load KMS diagnostics",
            Some(e.to_string()),
        )
    })?;
    log::debug!(
        "[KMS][Diagnostics] event_code=KMS_DIAGNOSTICS_OK request_id={}",
        request_id
    );
    Ok(KmsDiagnosticsDto {
        note_count: stats.note_count,
        snippet_count: stats.snippet_count,
        clip_count: stats.clip_count,
        vector_count: stats.vector_count,
        error_log_count: stats.error_log_count,
    })
}

