//! Copy-to-clipboard settings (load/save), masking, optional JSON sidecar files, and SQLite text persistence.

use std::path::PathBuf;

use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_text_expander::ports::{storage_keys, StoragePort};
use regex::Regex;

use crate::clipboard_repository;
use crate::CopyToClipboardConfigDto;

pub(crate) fn default_copy_to_clipboard_config(max_history_entries: u32) -> CopyToClipboardConfigDto {
    let json_dir = digicore_text_expander::ports::data_path_resolver::DataPathResolver::clipboard_json_dir();
    let image_dir = digicore_text_expander::ports::data_path_resolver::DataPathResolver::clipboard_images_dir();
    CopyToClipboardConfigDto {
        enabled: true,
        image_capture_enabled: true,
        min_log_length: 1,
        mask_cc: false,
        mask_ssn: false,
        mask_email: false,
        blacklist_processes: String::new(),
        max_history_entries,
        json_output_enabled: true,
        json_output_dir: json_dir.to_string_lossy().to_string(),
        image_storage_dir: image_dir.to_string_lossy().to_string(),
        ocr_enabled: true,
    }
}

pub(crate) fn load_copy_to_clipboard_config(
    storage: &JsonFileStorageAdapter,
    max_history_entries: u32,
) -> CopyToClipboardConfigDto {
    let mut cfg = default_copy_to_clipboard_config(max_history_entries);
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_ENABLED) {
        cfg.enabled = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_IMAGE_ENABLED) {
        cfg.image_capture_enabled = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_MIN_LOG_LENGTH) {
        if let Ok(parsed) = v.parse::<u32>() {
            cfg.min_log_length = parsed.clamp(1, 2000);
        }
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_MASK_CC) {
        cfg.mask_cc = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_MASK_SSN) {
        cfg.mask_ssn = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_MASK_EMAIL) {
        cfg.mask_email = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_BLACKLIST_PROCESSES) {
        cfg.blacklist_processes = v;
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_JSON_OUTPUT_ENABLED) {
        cfg.json_output_enabled = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_JSON_OUTPUT_DIR) {
        cfg.json_output_dir = v;
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_IMAGE_STORAGE_DIR) {
        cfg.image_storage_dir = v;
    }
    if let Some(v) = storage.get(storage_keys::COPY_TO_CLIPBOARD_OCR_ENABLED) {
        cfg.ocr_enabled = v == "true";
    }
    cfg
}

pub(crate) fn save_copy_to_clipboard_config(config: &CopyToClipboardConfigDto) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    storage.set(storage_keys::COPY_TO_CLIPBOARD_ENABLED, &config.enabled.to_string());
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_IMAGE_ENABLED,
        &config.image_capture_enabled.to_string(),
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_MIN_LOG_LENGTH,
        &config.min_log_length.clamp(1, 2000).to_string(),
    );
    storage.set(storage_keys::COPY_TO_CLIPBOARD_MASK_CC, &config.mask_cc.to_string());
    storage.set(storage_keys::COPY_TO_CLIPBOARD_MASK_SSN, &config.mask_ssn.to_string());
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_MASK_EMAIL,
        &config.mask_email.to_string(),
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_BLACKLIST_PROCESSES,
        &config.blacklist_processes,
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_JSON_OUTPUT_ENABLED,
        &config.json_output_enabled.to_string(),
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_JSON_OUTPUT_DIR,
        &config.json_output_dir,
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_IMAGE_STORAGE_DIR,
        &config.image_storage_dir,
    );
    storage.set(
        storage_keys::COPY_TO_CLIPBOARD_OCR_ENABLED,
        &config.ocr_enabled.to_string(),
    );
    let result = storage.persist_if_safe().map(|_| ()).map_err(|e| e.to_string());
    if result.is_ok() {
        clipboard_history::update_config(ClipboardHistoryConfig {
            enabled: config.enabled || config.image_capture_enabled,
            max_depth: if config.max_history_entries == 0 {
                usize::MAX
            } else {
                config.max_history_entries as usize
            },
        });
    }
    result
}

pub(crate) fn process_is_blacklisted(process_name: &str, blacklist_csv: &str) -> bool {
    let process_norm = process_name.trim().to_ascii_lowercase();
    if process_norm.is_empty() {
        return false;
    }
    blacklist_csv
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .any(|blocked| process_norm == blocked || process_norm == format!("{blocked}.exe"))
}

fn apply_masking(mut content: String, cfg: &CopyToClipboardConfigDto) -> String {
    if cfg.mask_email {
        let email_re = Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").ok();
        if let Some(re) = email_re {
            content = re.replace_all(&content, "[masked_email]").to_string();
        }
    }
    if cfg.mask_ssn {
        let ssn_re = Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").ok();
        if let Some(re) = ssn_re {
            content = re.replace_all(&content, "[masked_ssn]").to_string();
        }
    }
    if cfg.mask_cc {
        let cc_re = Regex::new(r"\b(?:\d[ -]?){13,19}\b").ok();
        if let Some(re) = cc_re {
            content = re.replace_all(&content, "[masked_card]").to_string();
        }
    }
    content
}

pub(crate) fn normalize_clipboard_path_or_default(raw: &str, fallback: PathBuf) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        PathBuf::from(trimmed)
    }
}

pub(crate) fn write_clipboard_text_json_record(
    output_dir: &str,
    content: &str,
    process_name: &str,
    window_title: &str,
) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let output_root = normalize_clipboard_path_or_default(
        output_dir,
        digicore_text_expander::ports::data_path_resolver::DataPathResolver::clipboard_json_dir(),
    );
    std::fs::create_dir_all(&output_root).map_err(|e| e.to_string())?;
    let file_name = format!("clipboard_{:013}_{:06}.json", now.as_millis(), now.subsec_micros());
    let file_path = output_root.join(file_name);
    let payload = serde_json::json!({
        "schema_version": "1.0.0",
        "entry_type": "text",
        "created_at_unix_ms": now.as_millis().to_string(),
        "process_name": process_name,
        "window_title": window_title,
        "content": content
    });
    let serialized = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    std::fs::write(file_path, serialized).map_err(|e| e.to_string())
}

pub(crate) fn persist_clipboard_entry_with_settings(
    content: &str,
    process_name: &str,
    window_title: &str,
    file_list: Option<Vec<String>>,
) -> Result<Option<u32>, String> {
    let storage = JsonFileStorageAdapter::load();
    let max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20);
    let cfg = load_copy_to_clipboard_config(&storage, max_depth);
    if !cfg.enabled {
        return Ok(None);
    }
    if process_is_blacklisted(process_name, &cfg.blacklist_processes) {
        return Ok(None);
    }
    if content.trim().chars().count() < cfg.min_log_length as usize {
        return Ok(None);
    }
    let masked = apply_masking(content.to_string(), &cfg);
    let inserted_id = clipboard_repository::insert_entry(&masked, process_name, window_title, file_list)?;
    log::info!("[Clipboard] clipboard_repository::insert_entry returned {:?}", inserted_id);
    if let Some(_id) = inserted_id {
        if cfg.json_output_enabled {
            if let Err(err) = write_clipboard_text_json_record(
                &cfg.json_output_dir,
                &masked,
                process_name,
                window_title,
            ) {
                crate::app_diagnostics::diag_log("warn", format!("[Clipboard][json.write_err] {err}"));
            }
        }
        if cfg.max_history_entries > 0 {
            let _ = clipboard_repository::trim_to_depth(cfg.max_history_entries);
        }
    }
    Ok(inserted_id)
}

