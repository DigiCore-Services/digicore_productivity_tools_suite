//! KMS vault root path read/update, optional migration, persistence, sync, and watcher.

use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::app_state::AppState;
use digicore_text_expander::ports::storage_keys;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tauri::Manager;

use super::*;

pub(crate) async fn kms_get_vault_path(host: ApiImpl) -> Result<String, String> {
    Ok(host.get_vault_path().to_string_lossy().to_string())
}

pub(crate) async fn kms_set_vault_path(host: ApiImpl, new_path: String, migrate: bool) -> Result<(), String> {
    let request_id = kms_request_id("set_vault_path");
    let app = get_app(&host.app_handle);
    let old_path = host.get_vault_path();
    let new_path_buf = PathBuf::from(&new_path);

    if migrate && old_path.exists() && old_path != new_path_buf {
        if !new_path_buf.exists() {
            std::fs::create_dir_all(&new_path_buf).map_err(|e| {
                kms_ipc_error(
                    &request_id,
                    "KMS_VAULT_PATH_CREATE",
                    "KMS_SET_VAULT_PATH_CREATE_FAIL",
                    "Failed to create target vault path",
                    Some(e.to_string()),
                )
            })?;
        }

        fn move_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
            if src.is_dir() {
                if !dest.exists() {
                    return std::fs::rename(src, dest);
                }
                for entry in std::fs::read_dir(src)? {
                    let entry = entry?;
                    move_recursive(&entry.path(), &dest.join(entry.file_name()))?;
                }
            } else if !dest.exists() {
                std::fs::rename(src, dest)?;
            }
            Ok(())
        }

        if let Ok(entries) = std::fs::read_dir(&old_path) {
            for entry in entries.flatten() {
                let src = entry.path();
                let file_name = entry.file_name();
                let dest = new_path_buf.join(&file_name);

                if file_name.to_string_lossy().starts_with('.') {
                    continue;
                }

                if let Err(e) = move_recursive(&src, &dest) {
                    log::warn!(
                        "[KMS][Vault] event_code=KMS_SET_VAULT_PATH_MOVE_ITEM_WARN request_id={} src={} dest={} error={}",
                        request_id,
                        src.display(),
                        dest.display(),
                        e
                    );
                }
            }
        }
    }

    {
        let app_state = app.state::<Arc<Mutex<AppState>>>();
        let mut state = app_state.lock().unwrap();
        state.kms_vault_path = new_path.clone();

        let mut storage = JsonFileStorageAdapter::load();
        storage.set(storage_keys::KMS_VAULT_PATH, &new_path);
        if let Err(e) = storage.persist() {
            log::warn!(
                "[KMS][Vault] event_code=KMS_SET_VAULT_PATH_PERSIST_WARN request_id={} error={}",
                request_id,
                e
            );
        }
    }

    let app_clone = app.clone();
    let sync_path = new_path_buf.clone();
    let request_id_in_task = request_id.clone();
    tokio::spawn(async move {
        if let Err(e) = app_clone.emit("kms-sync-status", "Indexing...") {
            log::warn!(
                "[KMS][Vault] event_code=KMS_SET_VAULT_PATH_SYNC_STATUS_INDEXING_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
        if let Err(e) =
            crate::kms_sync_orchestration::sync_vault_files_to_db_internal(&app_clone, &sync_path).await
        {
            log::warn!(
                "[KMS][Vault] event_code=KMS_SET_VAULT_PATH_SYNC_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
        if let Err(e) = app_clone.emit("kms-sync-status", "Idle") {
            log::warn!(
                "[KMS][Vault] event_code=KMS_SET_VAULT_PATH_SYNC_STATUS_IDLE_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
        if let Err(e) = app_clone.emit("kms-sync-complete", ()) {
            log::warn!(
                "[KMS][Vault] event_code=KMS_SET_VAULT_PATH_SYNC_COMPLETE_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
    });

    crate::kms_watcher::start_kms_watcher(app.clone(), new_path_buf);

    if let Err(e) = app.emit("kms-vault-path-changed", new_path) {
        log::warn!(
            "[KMS][Vault] event_code=KMS_SET_VAULT_PATH_CHANGE_EVENT_WARN request_id={} error={}",
            request_id,
            e
        );
    }

    Ok(())
}

