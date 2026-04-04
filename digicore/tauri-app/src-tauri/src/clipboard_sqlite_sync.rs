//! Runtime clipboard history sync to SQLite, image capture, OCR side-effects, and KMS clipboard indexing.

use std::sync::Arc;

use digicore_core::domain::{ExtractionMimeType, ExtractionSource};
use digicore_text_expander::application::clipboard_history;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::ports::{storage_keys, StoragePort};
use digicore_text_expander::services::extraction_service::create_extraction_service;
use tauri::Manager;

use crate::clipboard_repository;
use crate::kms_repository;

pub(crate) fn sync_runtime_clipboard_entries_to_sqlite(app: &tauri::AppHandle) {
    let entries = clipboard_history::get_entries();
    if entries.is_empty() {
        sync_current_clipboard_image_to_sqlite(String::new(), String::new(), Some(app));
        return;
    }
    for entry in entries.into_iter().rev() {
        let _ = crate::clipboard_text_persistence::persist_clipboard_entry_with_settings(
            &entry.content,
            &entry.process_name,
            &entry.window_title,
            entry.file_list.clone(),
        );
    }
    sync_current_clipboard_image_to_sqlite(String::new(), String::new(), Some(app));
}

pub(crate) fn sync_current_clipboard_image_to_sqlite(
    process_name: String,
    window_title: String,
    app: Option<&tauri::AppHandle>,
) {
    let storage = JsonFileStorageAdapter::load();
    let max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20);
    let cfg = crate::clipboard_text_persistence::load_copy_to_clipboard_config(&storage, max_depth);
    if !cfg.enabled && !cfg.image_capture_enabled {
        return;
    }
    if !cfg.image_capture_enabled {
        return;
    }
    if crate::clipboard_text_persistence::process_is_blacklisted(&process_name, &cfg.blacklist_processes) {
        return;
    }
    let mut image_opt = None;
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        match arboard::Clipboard::new().and_then(|mut c| c.get_image()) {
            Ok(img) => {
                log::info!(
                    "[Clipboard][capture.image] Detected image: {}x{} ({} bytes) on attempt {}",
                    img.width,
                    img.height,
                    img.bytes.len(),
                    attempt + 1
                );
                image_opt = Some(img);
                break;
            }
            Err(e) => {
                if attempt == 2 {
                    log::warn!(
                        "[Clipboard][capture.image] Final failed to get image from clipboard: {}",
                        e
                    );
                } else {
                    log::debug!(
                        "[Clipboard][capture.image] Retryable failure to get image (attempt {}): {}",
                        attempt + 1,
                        e
                    );
                }
            }
        }
    }

    let image = match image_opt {
        Some(img) => img,
        None => return,
    };
    if image.width == 0 || image.height == 0 || image.bytes.is_empty() {
        return;
    }

    let rgba_bytes = image.bytes.to_vec();
    let width = image.width;
    let height = image.height;
    let proc = process_name.clone();
    let win = window_title.clone();
    let ocr_enabled = cfg.ocr_enabled;

    let inserted_id = clipboard_repository::insert_image_entry_returning_id(
        &rgba_bytes,
        width as u32,
        height as u32,
        &process_name,
        &window_title,
        Some("image/png"),
        &cfg.image_storage_dir,
    )
    .unwrap_or(0);

    if inserted_id > 0 {
        if let Some(handle) = app {
            let h = handle.clone();
            let service = h
                .state::<Arc<crate::indexing_service::KmsIndexingService>>()
                .inner()
                .clone();
            let entity_id = inserted_id.to_string();
            tauri::async_runtime::spawn(async move {
                let _ = service.index_single_item(&h, "clipboard", &entity_id).await;
            });
        }

        if cfg.max_history_entries > 0 {
            if let Ok(deleted_ids) = clipboard_repository::trim_to_depth(cfg.max_history_entries) {
                for id in deleted_ids {
                    let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
                }
            }
        }
        crate::app_diagnostics::diag_log("info", "[Clipboard][capture.image] persisted clipboard image");

        let app_handle_for_ocr = app.cloned();
        if ocr_enabled {
            tauri::async_runtime::spawn(async move {
                let dispatcher = create_extraction_service();
                let mut png_bytes = Vec::new();
                match image::RgbaImage::from_raw(width as u32, height as u32, rgba_bytes) {
                    Some(img) => {
                        if let Err(e) = image::DynamicImage::ImageRgba8(img).write_to(
                            &mut std::io::Cursor::new(&mut png_bytes),
                            image::ImageFormat::Png,
                        ) {
                            log::error!("[Clipboard][OCR] Failed to encode PNG for OCR: {}", e);
                            return;
                        }
                    }
                    None => {
                        log::error!(
                            "[Clipboard][OCR] Failed to construct RgbaImage from buffer ({}x{})",
                            width,
                            height
                        );
                        return;
                    }
                }

                let source = ExtractionSource::Buffer(png_bytes);
                let mime = ExtractionMimeType::Png;

                log::info!(
                    "[Clipboard][OCR] Starting background OCR for parent_id: {}",
                    inserted_id
                );
                match dispatcher.extract(source, mime).await {
                    Ok(result) => {
                        if !result.text.trim().is_empty() {
                            let text_id = clipboard_repository::insert_extracted_text_entry(
                                &result.text,
                                &proc,
                                &win,
                                inserted_id,
                                &result.metadata,
                            )
                            .unwrap_or(0);

                            if text_id > 0 {
                                if let Some(h) = app_handle_for_ocr {
                                    let service = h
                                        .state::<Arc<crate::indexing_service::KmsIndexingService>>()
                                        .inner()
                                        .clone();
                                    let entity_id = text_id.to_string();
                                    tauri::async_runtime::spawn(async move {
                                        let _ = service.index_single_item(&h, "clipboard", &entity_id).await;
                                    });
                                }
                            }
                            log::info!(
                                "[Clipboard][OCR] OCR completed and saved for parent_id: {}",
                                inserted_id
                            );
                        } else {
                            log::info!(
                                "[Clipboard][OCR] OCR completed but no text found for parent_id: {}",
                                inserted_id
                            );
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "[Clipboard][OCR] OCR failed for parent_id {}: {}",
                            inserted_id,
                            e
                        );
                    }
                }
            });
        }
    }
}
