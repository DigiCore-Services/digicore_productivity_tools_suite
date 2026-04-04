//! Bounded inbound service for text clipboard history listing/CRUD and copy-to-clipboard settings/stats.

use std::path::PathBuf;

use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};

use crate::clipboard_text_persistence::{
    default_copy_to_clipboard_config, load_copy_to_clipboard_config, normalize_clipboard_path_or_default,
};

use super::*;

fn clipboard_row_to_dto(r: clipboard_repository::ClipboardRow) -> ClipEntryDto {
    ClipEntryDto {
        id: r.id,
        content: r.content,
        process_name: r.process_name,
        window_title: r.window_title,
        length: r.char_count,
        word_count: r.word_count,
        created_at: r.created_at_unix_ms.to_string(),
        entry_type: r.entry_type,
        mime_type: r.mime_type,
        image_path: r.image_path,
        thumb_path: r.thumb_path,
        image_width: r.image_width,
        image_height: r.image_height,
        image_bytes: r.image_bytes,
        parent_id: r.parent_id,
        metadata: r.metadata,
        file_list: r.file_list,
    }
}

pub(crate) async fn get_clipboard_entries(_host: ApiImpl) -> Result<Vec<ClipEntryDto>, String> {
    let rows = clipboard_repository::list_entries(None, 500)?;
    Ok(rows.into_iter().map(clipboard_row_to_dto).collect())
}

pub(crate) async fn search_clipboard_entries(
    _host: ApiImpl,
    search: String,
    operator: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<ClipEntryDto>, String> {
    let rows = clipboard_repository::search_entries(
        &search,
        operator.as_deref(),
        limit.unwrap_or(500),
    )?;
    Ok(rows.into_iter().map(clipboard_row_to_dto).collect())
}

pub(crate) async fn delete_clip_entry(_host: ApiImpl, index: u32) -> Result<(), String> {
    let rows = clipboard_repository::list_entries(None, index.saturating_add(1))?;
    if let Some(row) = rows.get(index as usize) {
        let id = row.id;
        clipboard_repository::delete_entry_by_id(id)?;
        let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
        super::diag_log(
            "info",
            format!("[Clipboard][delete] removed entry id={} via index", id),
        );
    }
    clipboard_history::delete_entry_at(index as usize);
    Ok(())
}

pub(crate) async fn delete_clip_entry_by_id(_host: ApiImpl, id: u32) -> Result<(), String> {
    clipboard_repository::delete_entry_by_id(id)?;
    let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
    super::diag_log("info", format!("[Clipboard][delete] removed entry id={id}"));
    Ok(())
}

pub(crate) async fn clear_clipboard_history(_host: ApiImpl) -> Result<(), String> {
    clipboard_repository::clear_all()?;
    clipboard_history::clear_all();
    let _ = kms_repository::delete_all_embeddings_for_type("clipboard");
    super::diag_log("info", "[Clipboard][clear] cleared all clipboard history");
    Ok(())
}

pub(crate) async fn get_copy_to_clipboard_config(
    _host: ApiImpl,
) -> Result<CopyToClipboardConfigDto, String> {
    let storage = JsonFileStorageAdapter::load();
    let max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20);
    Ok(load_copy_to_clipboard_config(&storage, max_depth))
}

pub(crate) async fn save_copy_to_clipboard_config(
    host: ApiImpl,
    config: CopyToClipboardConfigDto,
) -> Result<(), String> {
    let mut normalized = config;
    normalized.min_log_length = normalized.min_log_length.clamp(1, 2000);
    let default_cfg = default_copy_to_clipboard_config(normalized.max_history_entries);
    let json_root = normalize_clipboard_path_or_default(
        &normalized.json_output_dir,
        PathBuf::from(&default_cfg.json_output_dir),
    );
    let image_root = normalize_clipboard_path_or_default(
        &normalized.image_storage_dir,
        PathBuf::from(&default_cfg.image_storage_dir),
    );
    normalized.json_output_dir = json_root.to_string_lossy().to_string();
    normalized.image_storage_dir = image_root.to_string_lossy().to_string();
    std::fs::create_dir_all(&normalized.json_output_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(PathBuf::from(&normalized.image_storage_dir).join("full"))
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(PathBuf::from(&normalized.image_storage_dir).join("thumbs"))
        .map_err(|e| e.to_string())?;

    let storage = JsonFileStorageAdapter::load();
    let max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20);
    let current = load_copy_to_clipboard_config(&storage, max_depth);

    crate::clipboard_text_persistence::save_copy_to_clipboard_config(&normalized)?;
    let migrated_assets = if current.image_storage_dir.trim() != normalized.image_storage_dir.trim() {
        clipboard_repository::migrate_image_assets_root(
            &current.image_storage_dir,
            &normalized.image_storage_dir,
        )?
    } else {
        0
    };
    {
        let mut guard = host.state.lock().map_err(|e| e.to_string())?;
        guard.clip_history_max_depth = normalized.max_history_entries as usize;
        clipboard_history::update_config(ClipboardHistoryConfig {
            enabled: normalized.enabled || normalized.image_capture_enabled,
            max_depth: if normalized.max_history_entries == 0 {
                usize::MAX
            } else {
                normalized.max_history_entries as usize
            },
        });
    }
    let deleted_ids = if normalized.max_history_entries > 0 {
        clipboard_repository::trim_to_depth(normalized.max_history_entries).unwrap_or_default()
    } else {
        Vec::new()
    };
    let trimmed = deleted_ids.len();
    for id in deleted_ids {
        let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
    }
    super::diag_log(
        "info",
        format!(
            "[Clipboard][config] saved enabled={} min_len={} max_entries={} trimmed={} migrated_assets={}",
            normalized.enabled,
            normalized.min_log_length,
            normalized.max_history_entries,
            trimmed,
            migrated_assets
        ),
    );
    Ok(())
}

pub(crate) async fn get_copy_to_clipboard_stats(_host: ApiImpl) -> Result<CopyToClipboardStatsDto, String> {
    Ok(CopyToClipboardStatsDto {
        total_entries: clipboard_repository::count()?,
    })
}

