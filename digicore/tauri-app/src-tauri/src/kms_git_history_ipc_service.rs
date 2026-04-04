//! KMS note git history, version restore, and history prune.

use crate::kms_git_service::{KmsGitService, KmsVersion};

use super::*;

pub(crate) async fn kms_get_history(
    _host: ApiImpl,
    rel_path: String,
) -> Result<Vec<KmsVersion>, String> {
    let request_id = kms_request_id("history");
    KmsGitService::get_history(&rel_path).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_GIT_HISTORY",
            "KMS_HISTORY_LOAD_FAIL",
            "Failed to load note history",
            Some(e.to_string()),
        )
    })
}

pub(crate) async fn kms_get_note_revision_content(
    host: ApiImpl,
    hash: String,
    path: String,
) -> Result<String, String> {
    let request_id = kms_request_id("note_revision_content");
    let path_buf = std::path::PathBuf::from(&path);
    let rel_path = if path_buf.is_absolute() {
        host.get_relative_path(&path_buf).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_PATH_OUTSIDE_VAULT",
                "KMS_REVISION_CONTENT_REL_FAIL",
                "Failed to resolve note path",
                Some(e),
            )
        })?
    } else {
        path.replace('\\', "/")
    };

    KmsGitService::get_file_content_at_revision(&hash, &rel_path).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_GIT_REVISION",
            "KMS_REVISION_CONTENT_FAIL",
            "Failed to read file at revision",
            Some(e.to_string()),
        )
    })
}

pub(crate) async fn kms_restore_version(
    host: ApiImpl,
    hash: String,
    rel_path: String,
) -> Result<(), String> {
    let request_id = kms_request_id("restore_version");
    let app = get_app(&host.app_handle);
    KmsGitService::restore_version(&hash, &rel_path).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_GIT_RESTORE",
            "KMS_HISTORY_RESTORE_FAIL",
            "Failed to restore note version",
            Some(e.to_string()),
        )
    })?;

    let abs_path = host.resolve_absolute_path(&rel_path);
    if let Err(e) =
        crate::kms_sync_orchestration::sync_single_note_to_db_internal(&app, &abs_path).await
    {
        log::warn!(
            "[KMS][History] event_code=KMS_HISTORY_RESTORE_SYNC_WARN request_id={} path={} error={}",
            request_id,
            rel_path,
            e
        );
    }
    if let Err(e) = app.emit("kms-sync-complete", ()) {
        log::warn!(
            "[KMS][History] event_code=KMS_HISTORY_RESTORE_SYNC_COMPLETE_WARN request_id={} error={}",
            request_id,
            e
        );
    }

    Ok(())
}

pub(crate) async fn kms_prune_history(_host: ApiImpl) -> Result<String, String> {
    let request_id = kms_request_id("prune_history");
    KmsGitService::prune_history().map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_GIT_PRUNE",
            "KMS_PRUNE_HISTORY_FAIL",
            "Failed to prune git history",
            Some(e.to_string()),
        )
    })
}

