//! KMS window launch, vault initialization, background sync, and filesystem watcher.

use tauri::Emitter;
use tauri::Manager;

use super::*;

pub(crate) async fn kms_launch(host: ApiImpl) -> Result<(), String> {
    let request_id = kms_request_id("launch");
    let app = get_app(&host.app_handle);
    let vault = host.get_vault_path();

    if let Some(win) = app.get_webview_window("kms") {
        if let Err(e) = win.show() {
            log::warn!(
                "[KMS][Launch] event_code=KMS_LAUNCH_SHOW_WARN request_id={} error={}",
                request_id,
                e
            );
        }
        if let Err(e) = win.unminimize() {
            log::warn!(
                "[KMS][Launch] event_code=KMS_LAUNCH_UNMINIMIZE_WARN request_id={} error={}",
                request_id,
                e
            );
        }
        if let Err(e) = win.set_focus() {
            log::warn!(
                "[KMS][Launch] event_code=KMS_LAUNCH_FOCUS_WARN request_id={} error={}",
                request_id,
                e
            );
        }
    } else {
        let _win = tauri::WebviewWindowBuilder::new(
            &app,
            "kms",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("DigiCore Knowledge Management Suite")
        .inner_size(1000.0, 700.0)
        .min_inner_size(800.0, 500.0)
        .build()
        .map_err(|e| e.to_string())?;
    }

    kms_repository::init_database().map_err(|e| e.to_string())?;

    let app_clone = app.clone();
    let request_id_in_task = request_id.clone();
    tokio::spawn(async move {
        if let Err(e) =
            crate::kms_sync_orchestration::sync_vault_files_to_db_internal(&app_clone, &vault).await
        {
            log::warn!(
                "[KMS][Launch] event_code=KMS_LAUNCH_BG_SYNC_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
    });

    Ok(())
}

pub(crate) async fn kms_initialize(host: ApiImpl) -> Result<String, String> {
    let request_id = kms_request_id("initialize");
    let app = get_app(&host.app_handle);
    let vault_path = host.get_vault_path();
    log::info!(
        "[KMS][Init] event_code=KMS_INIT_START request_id={} vault={}",
        request_id,
        vault_path.display()
    );

    if !vault_path.exists() {
        std::fs::create_dir_all(&vault_path).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_INIT_FS",
                "KMS_INIT_VAULT_CREATE_FAIL",
                "Failed to create KMS vault folder",
                Some(e.to_string()),
            )
        })?;
        std::fs::create_dir_all(vault_path.join("notes")).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_INIT_FS",
                "KMS_INIT_NOTES_DIR_CREATE_FAIL",
                "Failed to create notes folder",
                Some(e.to_string()),
            )
        })?;
        std::fs::create_dir_all(vault_path.join("attachments")).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_INIT_FS",
                "KMS_INIT_ATTACH_DIR_CREATE_FAIL",
                "Failed to create attachments folder",
                Some(e.to_string()),
            )
        })?;

        let welcome_content = "# Welcome to DigiCore KMS\n\nThis is your local-first knowledge base.\n\n- **Private**: All notes are stored as flat Markdown files.\n- **Connected**: Use `[[Links]]` to build your knowledge graph.\n- **Unified**: Access your snippets and clipboard history directly.";
        std::fs::write(vault_path.join("notes").join("Welcome.md"), welcome_content).map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_INIT_FS",
                "KMS_INIT_WELCOME_NOTE_FAIL",
                "Failed to create welcome note",
                Some(e.to_string()),
            )
        })?;
    }

    kms_repository::init_database().map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_INIT_DB",
            "KMS_INIT_DB_FAIL",
            "Failed to initialize KMS database",
            Some(e.to_string()),
        )
    })?;

    let app_clone = app.clone();
    let vault_clone = vault_path.clone();
    let request_id_in_task = request_id.clone();
    tokio::spawn(async move {
        if let Err(e) = app_clone.emit("kms-sync-status", "Indexing...") {
            log::warn!(
                "[KMS][Init] event_code=KMS_INIT_SYNC_STATUS_INDEXING_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
        if let Err(e) =
            crate::kms_sync_orchestration::sync_vault_files_to_db_internal(&app_clone, &vault_clone).await
        {
            log::warn!(
                "[KMS][Init] event_code=KMS_INIT_BG_SYNC_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
        if let Err(e) = app_clone.emit("kms-sync-status", "Idle") {
            log::warn!(
                "[KMS][Init] event_code=KMS_INIT_SYNC_STATUS_IDLE_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
        if let Err(e) = app_clone.emit("kms-sync-complete", ()) {
            log::warn!(
                "[KMS][Init] event_code=KMS_INIT_SYNC_COMPLETE_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
    });

    crate::kms_watcher::start_kms_watcher(app.clone(), vault_path.clone());

    log::info!(
        "[KMS][Init] event_code=KMS_INIT_OK request_id={}",
        request_id
    );
    Ok(vault_path.to_string_lossy().to_string())
}

