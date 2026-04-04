//! KMS vault filesystem watcher with debounced full-vault database sync.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use notify::{Config, Event, RecursiveMode, Watcher};
use tauri::Emitter;

use crate::kms_ipc_boundary::kms_request_id;
use crate::kms_sync_orchestration::sync_vault_files_to_db_internal;

static KMS_WATCHER: OnceLock<Mutex<Option<notify::RecommendedWatcher>>> = OnceLock::new();

fn stop_kms_watcher() {
    if let Some(guard_mutex) = KMS_WATCHER.get() {
        if let Ok(mut guard) = guard_mutex.lock() {
            *guard = None;
        }
    }
}

pub(crate) fn start_kms_watcher(app: tauri::AppHandle, path: PathBuf) {
    let request_id = kms_request_id("watcher_start");
    stop_kms_watcher();

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let request_id_in_watcher = request_id.clone();
    let watcher_res = notify::RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            if res.is_ok() {
                if let Err(e) = tx.blocking_send(()) {
                    log::warn!(
                        "[KMS][Watcher] event_code=KMS_WATCHER_EVENT_CHANNEL_SEND_WARN request_id={} error={}",
                        request_id_in_watcher,
                        e
                    );
                }
            }
        },
        Config::default(),
    );

    if let Ok(mut watcher) = watcher_res {
        if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
            log::warn!(
                "[KMS][Watcher] event_code=KMS_WATCHER_WATCH_PATH_WARN request_id={} path={} error={}",
                request_id,
                path.display(),
                e
            );
        }

        let watcher_mutex = KMS_WATCHER.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = watcher_mutex.lock() {
            *guard = Some(watcher);
        }

        let request_id_in_task = request_id.clone();
        tokio::spawn(async move {
            let mut last_event = std::time::Instant::now();
            let mut pending = false;

            loop {
                tokio::select! {
                    res = rx.recv() => {
                        if res.is_none() { break; }
                        last_event = std::time::Instant::now();
                        pending = true;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                        if pending && last_event.elapsed() >= std::time::Duration::from_millis(1000) {
                            log::info!(
                                "[KMS][Watcher] event_code=KMS_WATCHER_CHANGE_DETECTED request_id={} path={}",
                                request_id_in_task,
                                path.display()
                            );
                            if let Err(e) = app.emit("kms-sync-status", "Syncing...") {
                                log::warn!(
                                    "[KMS][Watcher] event_code=KMS_WATCHER_SYNC_STATUS_SYNCING_WARN request_id={} error={}",
                                    request_id_in_task,
                                    e
                                );
                            }
                            if let Err(e) = sync_vault_files_to_db_internal(&app, &path).await {
                                log::warn!(
                                    "[KMS][Watcher] event_code=KMS_WATCHER_SYNC_VAULT_WARN request_id={} path={} error={}",
                                    request_id_in_task,
                                    path.display(),
                                    e
                                );
                            }
                            if let Err(e) = app.emit("kms-sync-status", "Idle") {
                                log::warn!(
                                    "[KMS][Watcher] event_code=KMS_WATCHER_SYNC_STATUS_IDLE_WARN request_id={} error={}",
                                    request_id_in_task,
                                    e
                                );
                            }
                            if let Err(e) = app.emit("kms-sync-complete", ()) {
                                log::warn!(
                                    "[KMS][Watcher] event_code=KMS_WATCHER_SYNC_COMPLETE_WARN request_id={} error={}",
                                    request_id_in_task,
                                    e
                                );
                            }
                            pending = false;
                        }
                    }
                }
            }
        });
    }
}

