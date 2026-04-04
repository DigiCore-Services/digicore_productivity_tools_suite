//! KMS note files, vault tree, backlinks, and folder operations (not git history).

use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn map_kms_note_row_to_dto(host: &ApiImpl, r: kms_repository::KmsNoteRow) -> KmsNoteDto {
    let abs_path = host.resolve_absolute_path(&r.path).to_string_lossy().to_string();
    let p = Path::new(&r.path);
    let node_type = if r.path.contains("/skills/") {
        "skill"
    } else if r.path.to_lowercase().ends_with(".png") || r.path.to_lowercase().ends_with(".jpg") {
        "image"
    } else {
        "note"
    };
    let folder_path = p
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "".to_string());

    KmsNoteDto {
        id: r.id,
        path: abs_path,
        title: r.title,
        preview: r.content_preview,
        last_modified: r.last_modified,
        is_favorite: r.is_favorite,
        sync_status: r.sync_status,
        node_type: node_type.to_string(),
        folder_path,
        embedding_model_id: r.embedding_model_id.clone(),
        tags: serde_json::from_str(&r.tags_json).unwrap_or_default(),
    }
}

pub(crate) async fn kms_get_note_links(host: ApiImpl, path: String) -> Result<KmsLinksDto, String> {
    let request_id = kms_request_id("note_links");
    let path_buf = PathBuf::from(&path);
    let rel_path = host.get_relative_path(&path_buf).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_PATH_OUTSIDE_VAULT",
            "KMS_NOTE_LINKS_REL_PATH_FAIL",
            "Failed to resolve note path",
            Some(e),
        )
    })?;
    let (outgoing_rows, incoming_rows) = kms_repository::get_links_for_note(&rel_path).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_LINKS",
            "KMS_NOTE_LINKS_LOAD_FAIL",
            "Failed to load note links",
            Some(e.to_string()),
        )
    })?;

    Ok(KmsLinksDto {
        outgoing: outgoing_rows
            .into_iter()
            .map(|r| map_kms_note_row_to_dto(&host, r))
            .collect(),
        incoming: incoming_rows
            .into_iter()
            .map(|r| map_kms_note_row_to_dto(&host, r))
            .collect(),
    })
}

pub(crate) async fn kms_list_notes(host: ApiImpl) -> Result<Vec<KmsNoteDto>, String> {
    let request_id = kms_request_id("list_notes");
    let rows = kms_repository::list_notes().map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_NOTES",
            "KMS_LIST_NOTES_FAIL",
            "Failed to list KMS notes",
            Some(e.to_string()),
        )
    })?;
    log::debug!(
        "[KMS][Notes] event_code=KMS_LIST_NOTES_OK request_id={} count={}",
        request_id,
        rows.len()
    );
    Ok(rows
        .into_iter()
        .map(|r| map_kms_note_row_to_dto(&host, r))
        .collect())
}

pub(crate) async fn kms_set_note_favorite(
    host: ApiImpl,
    path: String,
    favorite: bool,
) -> Result<(), String> {
    let request_id = kms_request_id("set_note_favorite");
    let path_buf = PathBuf::from(&path);
    let rel_path = host.get_relative_path(&path_buf).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_PATH_OUTSIDE_VAULT",
            "KMS_SET_FAVORITE_REL_PATH_FAIL",
            "Failed to resolve note path",
            Some(e),
        )
    })?;
    let rows = kms_repository::set_note_favorite(&rel_path, favorite).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_NOTES",
            "KMS_SET_FAVORITE_FAIL",
            "Failed to update favorite flag",
            Some(e.to_string()),
        )
    })?;
    if rows == 0 {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_NOTE_NOT_INDEXED",
            "KMS_SET_FAVORITE_UNKNOWN_PATH",
            "No indexed note matches this path",
            Some(rel_path),
        ));
    }
    log::debug!(
        "[KMS][Notes] event_code=KMS_SET_FAVORITE_OK request_id={} path={} favorite={}",
        request_id,
        rel_path,
        favorite
    );
    Ok(())
}

pub(crate) async fn kms_get_recent_note_paths(host: ApiImpl) -> Result<Vec<String>, String> {
    let request_id = kms_request_id("get_recent_note_paths");
    let rels = kms_repository::get_kms_ui_string_list(kms_repository::KMS_UI_STATE_KEY_RECENT).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_UI_STATE",
            "KMS_GET_RECENT_PATHS_FAIL",
            "Failed to load recent note paths",
            Some(e.to_string()),
        )
    })?;
    Ok(rels
        .into_iter()
        .map(|rel| {
            host.resolve_absolute_path(&rel)
                .to_string_lossy()
                .to_string()
        })
        .collect())
}

pub(crate) async fn kms_set_recent_note_paths(host: ApiImpl, paths: Vec<String>) -> Result<(), String> {
    let request_id = kms_request_id("set_recent_note_paths");
    let mut rels: Vec<String> = Vec::new();
    for abs in paths {
        let pb = PathBuf::from(&abs);
        if let Ok(r) = host.get_relative_path(&pb) {
            rels.push(r.replace('\\', "/"));
        }
    }
    rels.truncate(kms_repository::KMS_UI_RECENT_PATHS_CAP);
    kms_repository::set_kms_ui_string_list(kms_repository::KMS_UI_STATE_KEY_RECENT, &rels).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_UI_STATE",
            "KMS_SET_RECENT_PATHS_FAIL",
            "Failed to save recent note paths",
            Some(e.to_string()),
        )
    })?;
    Ok(())
}

pub(crate) async fn kms_get_favorite_path_order(host: ApiImpl) -> Result<Vec<String>, String> {
    let request_id = kms_request_id("get_favorite_path_order");
    let rels = kms_repository::get_kms_ui_string_list(kms_repository::KMS_UI_STATE_KEY_FAVORITE_ORDER).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_UI_STATE",
            "KMS_GET_FAVORITE_ORDER_FAIL",
            "Failed to load favorite path order",
            Some(e.to_string()),
        )
    })?;
    Ok(rels
        .into_iter()
        .map(|rel| {
            host.resolve_absolute_path(&rel)
                .to_string_lossy()
                .to_string()
        })
        .collect())
}

pub(crate) async fn kms_set_favorite_path_order(host: ApiImpl, paths: Vec<String>) -> Result<(), String> {
    let request_id = kms_request_id("set_favorite_path_order");
    let mut rels: Vec<String> = Vec::new();
    for abs in paths {
        let pb = PathBuf::from(&abs);
        if let Ok(r) = host.get_relative_path(&pb) {
            rels.push(r.replace('\\', "/"));
        }
    }
    rels.truncate(kms_repository::KMS_UI_FAVORITE_ORDER_CAP);
    kms_repository::set_kms_ui_string_list(kms_repository::KMS_UI_STATE_KEY_FAVORITE_ORDER, &rels).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_UI_STATE",
            "KMS_SET_FAVORITE_ORDER_FAIL",
            "Failed to save favorite path order",
            Some(e.to_string()),
        )
    })?;
    Ok(())
}

pub(crate) async fn kms_load_note(_host: ApiImpl, path: String) -> Result<String, String> {
    let request_id = kms_request_id("load_note");
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_NOTE_NOT_FOUND",
            "KMS_LOAD_NOTE_NOT_FOUND",
            "Note file not found on disk",
            Some(path),
        ));
    }
    std::fs::read_to_string(path_buf).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_NOTE_READ",
            "KMS_LOAD_NOTE_READ_FAIL",
            "Failed to read note content",
            Some(e.to_string()),
        )
    })
}

pub(crate) async fn kms_save_note(
    host: ApiImpl,
    path: String,
    content: String,
) -> Result<(), String> {
    let request_id = kms_request_id("save_note");
    let app = get_app(&host.app_handle);
    crate::kms_service::KmsService::save_note(&app, &path, &content)
        .await
        .map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_NOTE_SAVE",
                "KMS_SAVE_NOTE_SERVICE_FAIL",
                "Failed to save note via KMS service",
                Some(e.to_string()),
            )
        })?;

    let path_buf = PathBuf::from(&path);
    let rel_path = host.get_relative_path(&path_buf).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_PATH_OUTSIDE_VAULT",
            "KMS_SAVE_NOTE_REL_PATH_FAIL",
            "Failed to resolve vault-relative note path",
            Some(e),
        )
    })?;
    let title = path_buf
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    kms_repository::upsert_unified_fts("note", &rel_path, &title, &content).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_FTS",
            "KMS_SAVE_NOTE_FTS_FAIL",
            "Failed to update note FTS index",
            Some(e.to_string()),
        )
    })?;

    let vault = host.get_vault_path();
    let request_id_in_task = request_id.clone();
    tokio::spawn(async move {
        if let Err(e) =
            crate::kms_sync_orchestration::sync_vault_files_to_db_internal(&app, &vault).await
        {
            log::warn!(
                "[KMS][Note] event_code=KMS_SAVE_NOTE_BG_SYNC_WARN request_id={} error={}",
                request_id_in_task,
                e
            );
        }
    });
    log::info!(
        "[KMS][Note] event_code=KMS_SAVE_NOTE_OK request_id={} path={}",
        request_id,
        rel_path
    );

    Ok(())
}

pub(crate) async fn kms_delete_note(host: ApiImpl, path: String) -> Result<(), String> {
    let request_id = kms_request_id("delete_note");
    let app = get_app(&host.app_handle);
    crate::kms_service::KmsService::delete_note(&app, &path)
        .await
        .map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_NOTE_DELETE",
                "KMS_DELETE_NOTE_FAIL",
                "Failed to delete note",
                Some(e.to_string()),
            )
        })
}

pub(crate) async fn kms_rename_note(
    host: ApiImpl,
    old_path: String,
    new_name: String,
) -> Result<String, String> {
    let request_id = kms_request_id("rename_note");
    let app = get_app(&host.app_handle);
    crate::kms_service::KmsService::rename_note(&app, &old_path, &new_name)
        .await
        .map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_NOTE_RENAME",
                "KMS_RENAME_NOTE_FAIL",
                "Failed to rename note",
                Some(e.to_string()),
            )
        })
}

pub(crate) async fn kms_rename_folder(
    host: ApiImpl,
    old_path: String,
    new_name: String,
) -> Result<String, String> {
    let request_id = kms_request_id("rename_folder");
    let app = get_app(&host.app_handle);
    crate::kms_service::KmsService::rename_folder(&app, &old_path, &new_name)
        .await
        .map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_FOLDER_RENAME",
                "KMS_RENAME_FOLDER_FAIL",
                "Failed to rename folder",
                Some(e.to_string()),
            )
        })
}

pub(crate) async fn kms_delete_folder(host: ApiImpl, path: String) -> Result<(), String> {
    let request_id = kms_request_id("delete_folder");
    let abs_path = PathBuf::from(&path);
    if !abs_path.exists() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_FOLDER_NOT_FOUND",
            "KMS_DELETE_FOLDER_NOT_FOUND",
            "Folder does not exist",
            Some(path),
        ));
    }
    if !abs_path.is_dir() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_FOLDER_INVALID",
            "KMS_DELETE_FOLDER_INVALID_PATH",
            "Path is not a folder",
            Some(path),
        ));
    }

    let rel_path = host.get_relative_path(&abs_path).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_PATH_OUTSIDE_VAULT",
            "KMS_DELETE_FOLDER_REL_PATH_FAIL",
            "Failed to resolve folder path",
            Some(e),
        )
    })?;

    kms_repository::delete_folder_recursive(&rel_path).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_FOLDER_DELETE",
            "KMS_DELETE_FOLDER_REPO_FAIL",
            "Failed to delete folder records",
            Some(e.to_string()),
        )
    })?;
    let _ = kms_repository::prune_kms_ui_path_entries(&rel_path, true);
    std::fs::remove_dir_all(&abs_path).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_FOLDER_DELETE",
            "KMS_DELETE_FOLDER_FS_FAIL",
            "Failed to delete folder from disk",
            Some(e.to_string()),
        )
    })?;

    crate::kms_link_adjacency_cache::invalidate_kms_link_adjacency_cache();

    Ok(())
}

pub(crate) async fn kms_move_item(
    host: ApiImpl,
    path: String,
    new_parent_path: String,
) -> Result<String, String> {
    let request_id = kms_request_id("move_item");
    let app = get_app(&host.app_handle);
    crate::kms_service::KmsService::move_item(&app, &path, &new_parent_path)
        .await
        .map_err(|e| {
            kms_ipc_error(
                &request_id,
                "KMS_ITEM_MOVE",
                "KMS_MOVE_ITEM_FAIL",
                "Failed to move item",
                Some(e.to_string()),
            )
        })
}

pub(crate) async fn kms_create_folder(_host: ApiImpl, path: String) -> Result<(), String> {
    let request_id = kms_request_id("create_folder");
    let path_buf = PathBuf::from(&path);
    if path_buf.exists() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_FOLDER_EXISTS",
            "KMS_CREATE_FOLDER_EXISTS",
            "Folder already exists",
            Some(path),
        ));
    }
    std::fs::create_dir_all(&path_buf).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_FOLDER_CREATE",
            "KMS_CREATE_FOLDER_FS_FAIL",
            "Failed to create folder",
            Some(e.to_string()),
        )
    })?;
    Ok(())
}

fn build_vault_tree(
    dir: &Path,
    root: &Path,
    note_map: &mut HashMap<String, KmsNoteDto>,
) -> Vec<KmsFileSystemItemDto> {
    let mut items = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if name.starts_with('.') {
                continue;
            }

            if path.is_dir() {
                let children = build_vault_tree(&path, root, note_map);
                let rel_path = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                items.push(KmsFileSystemItemDto {
                    name,
                    path: path.to_string_lossy().to_string(),
                    rel_path,
                    item_type: "directory".to_string(),
                    children: Some(children),
                    note: None,
                });
            } else if path
                .extension()
                .map(|e| e == "md" || e == "markdown")
                .unwrap_or(false)
            {
                let rel_path = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let note = note_map.remove(&rel_path);
                items.push(KmsFileSystemItemDto {
                    name,
                    path: path.to_string_lossy().to_string(),
                    rel_path,
                    item_type: "file".to_string(),
                    children: None,
                    note,
                });
            }
        }
    }
    items.sort_by(|a, b| {
        if a.item_type != b.item_type {
            b.item_type.cmp(&a.item_type)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });
    items
}

pub(crate) async fn kms_get_vault_structure(host: ApiImpl) -> Result<KmsFileSystemItemDto, String> {
    let request_id = kms_request_id("vault_structure");
    let vault_root = host.get_vault_path();
    if !vault_root.exists() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_VAULT_NOT_INITIALIZED",
            "KMS_VAULT_STRUCTURE_NOT_INITIALIZED",
            "Vault not initialized",
            None,
        ));
    }

    let db_notes = kms_repository::list_notes().map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_REPO_NOTES",
            "KMS_VAULT_STRUCTURE_NOTES_FAIL",
            "Failed to load notes for vault structure",
            Some(e.to_string()),
        )
    })?;
    let mut note_map: HashMap<String, KmsNoteDto> = db_notes
        .into_iter()
        .map(|r| {
            (
                r.path.replace('\\', "/"),
                map_kms_note_row_to_dto(&host, r),
            )
        })
        .collect();

    let children = build_vault_tree(&vault_root, &vault_root, &mut note_map);

    Ok(KmsFileSystemItemDto {
        name: "Vault".to_string(),
        path: vault_root.to_string_lossy().to_string(),
        rel_path: "".to_string(),
        item_type: "directory".to_string(),
        children: Some(children),
        note: None,
    })
}

const MEDIA_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "pdf", "mp4", "webm", "mp3", "wav", "mov",
];

fn walk_media_files(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk_media_files(&path, root, out);
        } else if let Some(ext) = path.extension() {
            let e = ext.to_string_lossy().to_lowercase();
            if MEDIA_EXTENSIONS.contains(&e.as_str()) {
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(rel);
            }
        }
    }
}

fn walk_markdown_concat(dir: &Path, root: &Path, buf: &mut String) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk_markdown_concat(&path, root, buf)?;
        } else if path
            .extension()
            .map(|e| e == "md" || e == "markdown")
            .unwrap_or(false)
        {
            if let Ok(s) = std::fs::read_to_string(&path) {
                buf.push_str(&s);
                buf.push('\n');
            }
        }
    }
    Ok(())
}

pub(crate) async fn kms_list_vault_media(host: ApiImpl) -> Result<Vec<String>, String> {
    let request_id = kms_request_id("vault_media");
    let vault_root = host.get_vault_path();
    if !vault_root.exists() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_VAULT_NOT_INITIALIZED",
            "KMS_VAULT_MEDIA_NOT_INITIALIZED",
            "Vault not initialized",
            None,
        ));
    }
    let mut out = Vec::new();
    walk_media_files(&vault_root, &vault_root, &mut out);
    out.sort();
    Ok(out)
}

pub(crate) async fn kms_list_unused_vault_media(host: ApiImpl) -> Result<Vec<String>, String> {
    let request_id = kms_request_id("vault_media_unused");
    let vault_root = host.get_vault_path();
    if !vault_root.exists() {
        return Err(kms_ipc_error(
            &request_id,
            "KMS_VAULT_NOT_INITIALIZED",
            "KMS_VAULT_MEDIA_UNUSED_NOT_INITIALIZED",
            "Vault not initialized",
            None,
        ));
    }
    let mut media = Vec::new();
    walk_media_files(&vault_root, &vault_root, &mut media);
    let mut md_text = String::new();
    walk_markdown_concat(&vault_root, &vault_root, &mut md_text).map_err(|e| {
        kms_ipc_error(
            &request_id,
            "KMS_VAULT_READ_MD",
            "KMS_VAULT_MEDIA_UNUSED_READ_FAIL",
            "Failed to read markdown notes for unused scan",
            Some(e.to_string()),
        )
    })?;
    let hay = md_text.to_lowercase();
    let mut unused = Vec::new();
    for rel in media {
        let base = Path::new(&rel)
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let rel_lower = rel.to_lowercase();
        if !hay.contains(rel_lower.as_str()) && !hay.contains(base.as_str()) {
            unused.push(rel);
        }
    }
    unused.sort();
    Ok(unused)
}

