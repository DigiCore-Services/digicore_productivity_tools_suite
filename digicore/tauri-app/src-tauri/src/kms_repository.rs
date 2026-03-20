use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use chrono;
use crate::clipboard_repository;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::ports::{storage_keys, StoragePort};

pub fn get_vault_path() -> Result<PathBuf, String> {
    let storage = JsonFileStorageAdapter::load();
    let path_str = storage.get(storage_keys::KMS_VAULT_PATH)
        .ok_or_else(|| "KMS Vault Path not configured".to_string())?;
    Ok(PathBuf::from(path_str))
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct KmsNoteRow {
    pub id: i32,
    pub path: String,
    pub title: String,
    pub content_preview: Option<String>,
    pub last_modified: Option<String>,
    pub is_favorite: bool,
    pub sync_status: String,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct KmsIndexStatusRow {
    pub entity_type: String,
    pub entity_id: String,
    pub status: String,
    pub error: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct KmsLog {
    pub id: i32,
    pub level: String,
    pub message: String,
    pub details: Option<String>,
    pub timestamp: String,
}

static DB_CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub entity_type: String,
    pub entity_id: String,
    pub distance: f32,
    pub modality: String,
    pub metadata: Option<String>,
}

pub fn init(db_path: PathBuf) -> Result<(), String> {
    if DB_CONN.get().is_some() {
        return Ok(());
    }
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ())));
    }
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    // Tables are created via migrations in lib.rs, 
    // but we ensure journaling and sync modes here for this connection.
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        
        CREATE TABLE IF NOT EXISTS kms_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            level TEXT NOT NULL,
            message TEXT NOT NULL,
            details TEXT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        "#
    ).map_err(|e| e.to_string())?;
    
    if DB_CONN.set(Mutex::new(conn)).is_err() {
        // Already initialized by another thread, that's fine.
    }
    Ok(())
}

pub fn init_database() -> Result<(), String> {
    init(clipboard_repository::default_db_path())
}

fn conn_guard() -> Result<std::sync::MutexGuard<'static, Connection>, String> {
    let conn = DB_CONN
        .get()
        .ok_or_else(|| "KMS repository not initialized".to_string())?;
    conn.lock().map_err(|e| e.to_string())
}

pub fn upsert_note(path: &str, title: &str, preview: &str, sync_status: &str, error: Option<&str>) -> Result<(), String> {
    let conn = conn_guard()?;
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO kms_notes (path, title, content_preview, last_modified, sync_status, last_error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(path) DO UPDATE SET
            title = excluded.title,
            content_preview = excluded.content_preview,
            last_modified = excluded.last_modified,
            sync_status = excluded.sync_status,
            last_error = excluded.last_error",
        params![path, title, preview, now, sync_status, error],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_notes() -> Result<Vec<KmsNoteRow>, String> {
    let conn = conn_guard()?;
    let mut stmt = conn
        .prepare("SELECT id, path, title, content_preview, last_modified, is_favorite, sync_status, last_error FROM kms_notes ORDER BY last_modified DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(KmsNoteRow {
                id: row.get(0)?,
                path: row.get(1)?,
                title: row.get(2)?,
                content_preview: row.get(3)?,
                last_modified: row.get(4)?,
                is_favorite: row.get::<_, i32>(5)? != 0,
                sync_status: row.get(6)?,
                last_error: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut notes = Vec::new();
    for note in rows {
        notes.push(note.map_err(|e| e.to_string())?);
    }
    Ok(notes)
}

pub fn update_index_status(entity_type: &str, entity_id: &str, status: &str, error: Option<&str>) -> Result<(), String> {
    let conn = conn_guard()?;
    conn.execute(
        "INSERT INTO kms_index_status (entity_type, entity_id, status, error, updated_at)
         VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
            status = excluded.status,
            error = excluded.error,
            updated_at = CURRENT_TIMESTAMP",
        params![entity_type, entity_id, status, error],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_detailed_status(category: &str) -> Result<Vec<KmsIndexStatusRow>, String> {
    let conn = conn_guard()?;
    
    if category == "notes" {
        let mut stmt = conn.prepare("SELECT path, 'notes', sync_status, last_error, last_modified FROM kms_notes WHERE sync_status = 'failed'")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
            Ok(KmsIndexStatusRow {
                entity_id: row.get(0)?,
                entity_type: row.get(1)?,
                status: row.get(2)?,
                error: row.get(3)?,
                updated_at: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        }).map_err(|e| e.to_string())?;
        
        let mut results = Vec::new();
        for r in rows {
            results.push(r.map_err(|e| e.to_string())?);
        }
        return Ok(results);
    }

    let mut stmt = conn.prepare("SELECT entity_id, entity_type, status, error, updated_at FROM kms_index_status WHERE entity_type = ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![category], |row| {
        Ok(KmsIndexStatusRow {
            entity_id: row.get(0)?,
            entity_type: row.get(1)?,
            status: row.get(2)?,
            error: row.get(3)?,
            updated_at: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for r in rows {
        results.push(r.map_err(|e| e.to_string())?);
    }
    Ok(results)
}

pub fn get_category_counts(category: &str) -> Result<(u32, u32, u32), String> {
    let conn = conn_guard()?;
    
    if category == "notes" {
        let total: u32 = conn.query_row("SELECT COUNT(*) FROM kms_notes", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let indexed: u32 = conn.query_row("SELECT COUNT(*) FROM kms_notes WHERE sync_status = 'indexed'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let failed: u32 = conn.query_row("SELECT COUNT(*) FROM kms_notes WHERE sync_status = 'failed'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        return Ok((indexed, failed, total));
    }
    
    // For snippets/clipboard, we need to know the total baseline.
    // For now, let's use the counts from the source tables.
    let total: u32 = if category == "snippets" {
        conn.query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get(0)).map_err(|e| e.to_string())?
    } else if category == "clipboard" {
        conn.query_row("SELECT COUNT(*) FROM clipboard_history", [], |r| r.get(0)).map_err(|e| e.to_string())?
    } else {
        0
    };
    
    let indexed: u32 = conn.query_row("SELECT COUNT(*) FROM kms_index_status WHERE entity_type = ?1 AND status = 'indexed'", params![category], |r| r.get(0)).map_err(|e| e.to_string())?;
    let failed: u32 = conn.query_row("SELECT COUNT(*) FROM kms_index_status WHERE entity_type = ?1 AND status = 'failed'", params![category], |r| r.get(0)).map_err(|e| e.to_string())?;
    
    Ok((indexed, failed, total))
}

pub fn insert_log(level: &str, message: &str, details: Option<&str>) -> Result<(), String> {
    let conn = conn_guard()?;
    conn.execute(
        "INSERT INTO kms_logs (level, message, details) VALUES (?1, ?2, ?3)",
        params![level, message, details],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_logs(limit: u32) -> Result<Vec<KmsLog>, String> {
    let conn = conn_guard()?;
    let mut stmt = conn.prepare("SELECT id, level, message, details, timestamp FROM kms_logs ORDER BY id DESC LIMIT ?1")
        .map_err(|e| e.to_string())?;
    
    let rows = stmt.query_map(params![limit], |row| {
        Ok(KmsLog {
            id: row.get(0)?,
            level: row.get(1)?,
            message: row.get(2)?,
            details: row.get(3)?,
            timestamp: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;
    
    let mut logs = Vec::new();
    for r in rows {
        logs.push(r.map_err(|e| e.to_string())?);
    }
    Ok(logs)
}

pub fn clear_logs() -> Result<(), String> {
    let conn = conn_guard()?;
    conn.execute("DELETE FROM kms_logs", []).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_note(path: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    
    // 1. Find vector IDs to delete
    let mut stmt = conn.prepare("SELECT vec_id FROM kms_vector_map WHERE entity_type = 'note' AND entity_id = ?1")
        .map_err(|e| e.to_string())?;
    let vec_ids: Vec<i64> = stmt.query_map(params![path], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // 2. Delete from vector tables
    for vid in vec_ids {
        let _ = conn.execute("DELETE FROM kms_embeddings_text WHERE rowid = ?1", params![vid]);
        let _ = conn.execute("DELETE FROM kms_embeddings_image WHERE rowid = ?1", params![vid]);
    }
    
    // 3. Delete from support tables (mapping, links)
    let _ = conn.execute("DELETE FROM kms_vector_map WHERE entity_type = 'note' AND entity_id = ?1", params![path]);
    let _ = conn.execute("DELETE FROM kms_links WHERE source_path = ?1 OR target_path = ?1", params![path]);
    
    // 4. Delete from main table (triggers triggers for FTS, and CASCADE for tags)
    conn.execute("DELETE FROM kms_notes WHERE path = ?1", params![path])
        .map_err(|e| e.to_string())?;
        
    Ok(())
}

pub fn repair_database() -> Result<(), String> {
    let conn = conn_guard()?;
    
    log::warn!("[KMS] Executing surgical database repair (KMS Reset)...");

    // Drop virtual tables that are prone to corruption (sqlite-vec / fts5)
    let _ = conn.execute("DROP TABLE IF EXISTS kms_notes_fts", []);
    let _ = conn.execute("DROP TABLE IF EXISTS kms_embeddings_text", []);
    let _ = conn.execute("DROP TABLE IF EXISTS kms_embeddings_image", []);
    
    // Drop standard KMS application tables
    let _ = conn.execute("DROP TABLE IF EXISTS kms_notes", []);
    let _ = conn.execute("DROP TABLE IF EXISTS kms_links", []);
    let _ = conn.execute("DROP TABLE IF EXISTS kms_bookmarks", []);
    let _ = conn.execute("DROP TABLE IF EXISTS kms_tags", []);
    let _ = conn.execute("DROP TABLE IF EXISTS kms_note_tags", []);
    let _ = conn.execute("DROP TABLE IF EXISTS kms_vector_map", []);

    // RESET Migration history for KMS versions (v4-v7)
    // Based on user screenshots, the tracking table is `_sqlx_migrations` (from tauri-plugin-sql / sqlx)
    let _ = conn.execute("DELETE FROM _sqlx_migrations WHERE version IN (4, 5, 6, 7)", []);
    
    // Attempt VACUUM to ensure b-tree integrity after dropping corrupted virtual tables
    let _ = conn.execute("VACUUM", []);

    // Log success
    log::info!("[KMS] Surgical repair complete. KMS Tables dropped and migrations reset.");
    
    Ok(())
}

pub fn rename_note(old_path: &str, new_path: &str, new_title: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    
    // 1. Update the note metadata
    conn.execute(
        "UPDATE kms_notes SET path = ?1, title = ?2 WHERE path = ?3",
        params![new_path, new_title, old_path],
    )
    .map_err(|e| e.to_string())?;

    // 2. Update the vector mapping (path is the entity_id for notes)
    let _ = conn.execute(
        "UPDATE kms_vector_map SET entity_id = ?1 WHERE entity_type = 'note' AND entity_id = ?2",
        params![new_path, old_path]
    );

    Ok(())
}

pub fn rename_folder(old_path: &str, new_path: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    
    // 1. Update all notes within this folder by replacing the path prefix
    // SQL: UPDATE kms_notes SET path = ?1 || SUBSTR(path, LENGTH(?2) + 1) WHERE path LIKE ?2 || '%'
    // This correctly handles subfolders too.
    conn.execute(
        "UPDATE kms_notes SET path = ?1 || SUBSTR(path, LENGTH(?2) + 1) WHERE path LIKE ?2 || '%'",
        params![new_path, old_path],
    ).map_err(|e| e.to_string())?;

    // 2. Update vector mapping entity IDs
    conn.execute(
        "UPDATE kms_vector_map SET entity_id = ?1 || SUBSTR(entity_id, LENGTH(?2) + 1) 
         WHERE entity_type = 'note' AND entity_id LIKE ?2 || '%'",
        params![new_path, old_path]
    ).map_err(|e| e.to_string())?;

    // 3. Update links (both source and target)
    conn.execute(
        "UPDATE kms_links SET source_path = ?1 || SUBSTR(source_path, LENGTH(?2) + 1) 
         WHERE source_path LIKE ?2 || '%'",
        params![new_path, old_path]
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE kms_links SET target_path = ?1 || SUBSTR(target_path, LENGTH(?2) + 1) 
         WHERE target_path LIKE ?2 || '%'",
        params![new_path, old_path]
    ).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn delete_folder_recursive(path: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    
    // 1. Find all notes within this folder
    let mut stmt = conn.prepare("SELECT path FROM kms_notes WHERE path LIKE ?1 || '%'")
        .map_err(|e| e.to_string())?;
    
    let note_paths: Vec<String> = stmt.query_map(params![path], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // 2. Delete each note individually (reusing the existing logic to clean up embeddings/links/etc)
    for note_path in note_paths {
        delete_note(&note_path)?;
    }
    
    Ok(())
}

pub fn upsert_link(source_path: &str, target_path: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    conn.execute(
        "INSERT INTO kms_links (source_path, target_path) VALUES (?1, ?2) ON CONFLICT(source_path, target_path) DO NOTHING",
        params![source_path, target_path],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_links_for_source(source_path: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    conn.execute("DELETE FROM kms_links WHERE source_path = ?1", params![source_path])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_links_for_note(path: &str) -> Result<(Vec<KmsNoteRow>, Vec<KmsNoteRow>), String> {
    let conn = conn_guard()?;
    
    // Outgoing
    let mut stmt = conn.prepare(
        "SELECT n.id, n.path, n.title, n.content_preview, n.last_modified, n.is_favorite, n.sync_status, n.last_error 
         FROM kms_notes n
         JOIN kms_links l ON n.path = l.target_path
         WHERE l.source_path = ?1"
    ).map_err(|e| e.to_string())?;
    
    let outgoing = stmt.query_map(params![path], |row| {
        Ok(KmsNoteRow {
            id: row.get(0)?,
            path: row.get(1)?,
            title: row.get(2)?,
            content_preview: row.get(3)?,
            last_modified: row.get(4)?,
            is_favorite: row.get::<_, i32>(5)? != 0,
            sync_status: row.get(6)?,
            last_error: row.get(7)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    // Incoming (Backlinks)
    let mut stmt = conn.prepare(
        "SELECT n.id, n.path, n.title, n.content_preview, n.last_modified, n.is_favorite, n.sync_status, n.last_error 
         FROM kms_notes n
         JOIN kms_links l ON n.path = l.source_path
         WHERE l.target_path = ?1"
    ).map_err(|e| e.to_string())?;
    
    let incoming = stmt.query_map(params![path], |row| {
        Ok(KmsNoteRow {
            id: row.get(0)?,
            path: row.get(1)?,
            title: row.get(2)?,
            content_preview: row.get(3)?,
            last_modified: row.get(4)?,
            is_favorite: row.get::<_, i32>(5)? != 0,
            sync_status: row.get(6)?,
            last_error: row.get(7)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok((outgoing, incoming))
}

pub fn update_links_on_path_change(old_path: &str, new_path: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    // Update target paths (incoming links to the renamed note)
    conn.execute("UPDATE kms_links SET target_path = ?1 WHERE target_path = ?2", params![new_path, old_path])
        .map_err(|e| e.to_string())?;
    // Update source paths (outgoing links from the renamed note)
    conn.execute("UPDATE kms_links SET source_path = ?1 WHERE source_path = ?2", params![new_path, old_path])
        .map_err(|e| e.to_string())?;
    Ok(())
}



#[allow(dead_code)]
pub fn get_note_by_path(path: &str) -> Result<Option<KmsNoteRow>, String> {
    let conn = conn_guard()?;
    let mut stmt = conn
        .prepare("SELECT id, path, title, content_preview, last_modified, is_favorite, sync_status, last_error FROM kms_notes WHERE path = ?1")
        .map_err(|e| e.to_string())?;
    stmt.query_row(params![path], |row| {
        Ok(KmsNoteRow {
            id: row.get(0)?,
            path: row.get(1)?,
            title: row.get(2)?,
            content_preview: row.get(3)?,
            last_modified: row.get(4)?,
            is_favorite: row.get::<_, i32>(5)? != 0,
            sync_status: row.get(6)?,
            last_error: row.get(7)?,
        })
    })
    .optional()
    .map_err(|e| e.to_string())
}

pub fn upsert_embedding(
    modality: &str,
    entity_type: &str,
    entity_id: &str,
    embedding: Vec<f32>,
    metadata: Option<String>,
) -> Result<(), String> {
    let conn = conn_guard()?;
    
    // Convert embedding to bytes (float32 little-endian) for sqlite-vec
    let mut bytes = Vec::with_capacity(embedding.len() * 4);
    for f in embedding {
        bytes.extend_from_slice(&f.to_le_bytes());
    }

    let table = if modality == "text" { "kms_embeddings_text" } else { "kms_embeddings_image" };
    
    // Check for existing mapping to perform update vs insert
    let existing: Option<(i64, i64)> = conn.query_row(
        "SELECT id, vec_id FROM kms_vector_map WHERE entity_type = ?1 AND entity_id = ?2 AND modality = ?3",
        params![entity_type, entity_id, modality],
        |row| Ok((row.get(0)?, row.get(1)?))
    ).optional().map_err(|e| e.to_string())?;

    if let Some((map_id, vec_id)) = existing {
        // Update existing vector in the virtual table (rowid-based)
        conn.execute(
            &format!("UPDATE {} SET embedding = ?1 WHERE rowid = ?2", table),
            params![bytes, vec_id]
        ).map_err(|e| e.to_string())?;
        
        // Update metadata and timestamp in the mapping table
        conn.execute(
            "UPDATE kms_vector_map SET metadata = ?1, created_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![metadata, map_id]
        ).map_err(|e| e.to_string())?;
    } else {
        // Insert new vector into the virtual table
        conn.execute(
            &format!("INSERT INTO {} (embedding) VALUES (?1)", table),
            params![bytes]
        ).map_err(|e| e.to_string())?;
        
        let new_vec_id = conn.last_insert_rowid();

        // Insert new entry into the mapping table
        conn.execute(
            "INSERT INTO kms_vector_map (vec_id, modality, entity_type, entity_id, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new_vec_id, modality, entity_type, entity_id, metadata]
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[allow(dead_code)]
pub fn delete_embedding(modality: &str, entity_type: &str, entity_id: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    
    // 1. Find vector ID
    let vec_id: Option<i64> = conn.query_row(
        "SELECT vec_id FROM kms_vector_map WHERE modality = ?1 AND entity_type = ?2 AND entity_id = ?3",
        params![modality, entity_type, entity_id],
        |row| row.get(0)
    ).optional().map_err(|e| e.to_string())?;

    if let Some(vid) = vec_id {
        let table = if modality == "text" { "kms_embeddings_text" } else { "kms_embeddings_image" };
        // 2. Delete from virtual table
        let _ = conn.execute(&format!("DELETE FROM {} WHERE rowid = ?1", table), params![vid]);
        // 3. Delete from mapping table
        let _ = conn.execute(
            "DELETE FROM kms_vector_map WHERE vec_id = ?1 AND modality = ?2",
            params![vid, modality]
        );
    }
    
    Ok(())
}

pub fn upsert_unified_fts(
    entity_type: &str,
    entity_id: &str,
    title: &str,
    content: &str,
) -> Result<(), String> {
    let conn = conn_guard()?;
    
    // FTS5 doesn't have native ON CONFLICT, so we DELETE then INSERT
    let _ = conn.execute(
        "DELETE FROM kms_unified_fts WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type, entity_id],
    );
    
    conn.execute(
        "INSERT INTO kms_unified_fts (entity_type, entity_id, title, content)
         VALUES (?1, ?2, ?3, ?4)",
        params![entity_type, entity_id, title, content],
    ).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[allow(dead_code)]
pub fn delete_unified_fts(entity_type: &str, entity_id: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    conn.execute(
        "DELETE FROM kms_unified_fts WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type, entity_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Deletes all embeddings for all modalities (text, image, etc.) for a specific entity.
pub fn delete_embeddings_for_entity(entity_type: &str, entity_id: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    
    // 1. Get all modalities for this entity
    let mut stmt = conn.prepare(
        "SELECT vec_id, modality FROM kms_vector_map WHERE entity_type = ?1 AND entity_id = ?2"
    ).map_err(|e| e.to_string())?;
    
    let rows = stmt.query_map(params![entity_type, entity_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    }).map_err(|e| e.to_string())?;

    for row_res in rows {
        if let Ok((vid, modality)) = row_res {
            let table = if modality == "text" { "kms_embeddings_text" } else { "kms_embeddings_image" };
            // 2. Delete from virtual table
            let _ = conn.execute(&format!("DELETE FROM {} WHERE rowid = ?1", table), params![vid]);
        }
    }

    // 3. Delete from mapping table
    let _ = conn.execute(
        "DELETE FROM kms_vector_map WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type, entity_id]
    ).map_err(|e| e.to_string())?;

    // 4. Also clean up index status
    let _ = conn.execute(
        "DELETE FROM kms_index_status WHERE entity_type = ?1 AND entity_id = ?2",
        params![entity_type, entity_id]
    );
    
    Ok(())
}

/// Bulk deletes all embeddings associated with a specific entity type (e.g., "clipboard").
pub fn delete_all_embeddings_for_type(entity_type: &str) -> Result<(), String> {
    let conn = conn_guard()?;
    
    // Delete from virtual tables using subqueries
    let _ = conn.execute(
        "DELETE FROM kms_embeddings_text WHERE rowid IN (SELECT vec_id FROM kms_vector_map WHERE entity_type = ?1 AND modality = 'text')",
        params![entity_type]
    );
    let _ = conn.execute(
        "DELETE FROM kms_embeddings_image WHERE rowid IN (SELECT vec_id FROM kms_vector_map WHERE entity_type = ?1 AND modality = 'image')",
        params![entity_type]
    );
    
    // Delete from mapping table
    let _ = conn.execute(
        "DELETE FROM kms_vector_map WHERE entity_type = ?1",
        params![entity_type]
    ).map_err(|e| e.to_string())?;

    // Also clear index status for this type
    let _ = conn.execute(
        "DELETE FROM kms_index_status WHERE entity_type = ?1",
        params![entity_type]
    ).map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Bulk deletes embeddings for a list of entity IDs.
#[allow(dead_code)]
pub fn delete_embeddings_for_ids(entity_type: &str, entity_ids: &[String]) -> Result<(), String> {
    if entity_ids.is_empty() {
        return Ok(());
    }
    
    // For safety and simplicity, we reuse the per-entity cleanup
    for id in entity_ids {
        let _ = delete_embeddings_for_entity(entity_type, id);
    }
    
    Ok(())
}

/// Performs a multi-modal search using k-NN, FTS5, or both (Hybrid Search).
pub fn search_hybrid(
    query: &str,
    modality: &str,
    query_vector: Vec<f32>,
    search_mode: &str, // "Hybrid", "Semantic", "Keyword"
    limit: u32,
) -> Result<Vec<SearchResult>, String> {
    let conn = conn_guard()?;
    
    let mut query_bytes = Vec::with_capacity(query_vector.len() * 4);
    for f in query_vector {
        query_bytes.extend_from_slice(&f.to_le_bytes());
    }

    let vector_table = if modality == "text" { "kms_embeddings_text" } else { "kms_embeddings_image" };
    
    #[derive(Debug, Clone)]
    struct Hit {
        entity_type: String,
        entity_id: String,
        modality: String,
        metadata: Option<String>,
        vec_rank: Option<usize>,
        fts_rank: Option<usize>,
        combined_score: f32,
        dist: f32,
    }
    
    let mut hits: std::collections::HashMap<(String, String), Hit> = std::collections::HashMap::new();

    // 1. Vector Search (if Hybrid or Semantic)
    if search_mode == "Hybrid" || search_mode == "Semantic" {
        let mut stmt = conn.prepare(&format!(
            "SELECT m.entity_type, m.entity_id, v.distance, m.metadata
             FROM {} v
             JOIN kms_vector_map m ON v.rowid = m.vec_id
             WHERE v.embedding MATCH ?1 AND k = ?2
             ORDER BY distance",
            vector_table
        )).map_err(|e| e.to_string())?;

        let vec_rows = stmt.query_map(params![query_bytes, limit * 2], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)? as f32,
                row.get::<_, Option<String>>(3)?,
            ))
        }).map_err(|e| e.to_string())?;

        for (rank_idx, row) in vec_rows.enumerate() {
            let (entity_type, entity_id, distance, metadata) = row.map_err(|e| e.to_string())?;
            let key = (entity_type.clone(), entity_id.clone());
            hits.insert(key.clone(), Hit {
                entity_type,
                entity_id,
                modality: modality.to_string(),
                metadata,
                vec_rank: Some(rank_idx + 1),
                fts_rank: None,
                combined_score: 0.0,
                dist: distance,
            });
        }
    }

    // 2. FTS5 Search (if Hybrid or Keyword, and text modality)
    if modality == "text" && !query.trim().is_empty() && (search_mode == "Hybrid" || search_mode == "Keyword") {
        // Soften the query for FTS5 so multiple words act as an OR search for higher recall
        let words: Vec<_> = query
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '/')
            .collect::<String>()
            .split_whitespace()
            .map(|w| w.to_string())
            .filter(|w| !w.is_empty())
            .collect();
            
        if !words.is_empty() {
            // Boost titles substantially; give triggers/paths higher weight implicitly in titles
            let fts_query = if words.len() == 1 {
                format!("(title:{}* ^2) OR (content:{}*)", words[0], words[0])
            } else {
                let joined = words.iter().map(|w| format!("{}*", w)).collect::<Vec<_>>().join(" OR ");
                format!("(title:({}) ^2) OR (content:({}))", joined, joined)
            };

            let mut fts_stmt = conn.prepare(
                "SELECT f.entity_type, f.entity_id, bm25(kms_unified_fts) as score, m.metadata, m.modality
                 FROM kms_unified_fts f
                 LEFT JOIN kms_vector_map m ON f.entity_type = m.entity_type AND f.entity_id = m.entity_id
                 WHERE kms_unified_fts MATCH ?1
                 ORDER BY score
                 LIMIT ?2"
            ).map_err(|e| e.to_string())?;
            
            let fts_rows = match fts_stmt.query_map(params![fts_query, limit * 2], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)? as f32,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            }) {
                Ok(mapped) => mapped.collect::<Result<Vec<_>, _>>().unwrap_or_default(),
                Err(_) => Vec::new(),
            };

            for (rank_idx, (e_type, e_id, _score, metadata, hit_modality)) in fts_rows.into_iter().enumerate() {
                let key = (e_type.clone(), e_id.clone());
                let final_modality = hit_modality.unwrap_or_else(|| "text".to_string());
                
                let entry = hits.entry(key).or_insert_with(|| Hit {
                    entity_type: e_type,
                    entity_id: e_id,
                    modality: final_modality,
                    metadata: metadata.clone(),
                    vec_rank: None,
                    fts_rank: Some(rank_idx + 1),
                    combined_score: 0.0,
                    dist: 1.0,
                });
                
                // If it was already found by vector search, fts_rank will be updated
                if entry.fts_rank.is_none() {
                    entry.fts_rank = Some(rank_idx + 1);
                }
                // Ensure metadata is populated even if vector search didn't have it (though it should)
                if entry.metadata.is_none() {
                    entry.metadata = metadata;
                }
            }
        }
    }

    // 3. Reciprocal Rank Fusion (RRF) or Single Mode Scoring
    let k = 60.0;
    for (_, hit) in hits.iter_mut() {
        if search_mode == "Hybrid" {
            let mut rrf = 0.0;
            if let Some(vr) = hit.vec_rank {
                rrf += 1.0 / (k + vr as f32);
            }
            if let Some(fr) = hit.fts_rank {
                rrf += 1.0 / (k + fr as f32);
            }
            hit.combined_score = rrf;
            // Scale RRF back to 0..1 range (inverted distance)
            hit.dist = 1.0 - (hit.combined_score * 30.0).clamp(0.0, 1.0); 
        } else if search_mode == "Keyword" {
            if let Some(fr) = hit.fts_rank {
                // For keyword-only, we just use rank as score
                hit.combined_score = 1.0 / (k + fr as f32);
                hit.dist = 1.0 - (hit.combined_score * 60.0).clamp(0.0, 1.0);
            }
        } else {
            // Semantic mode uses distance directly from vector search
            hit.combined_score = 1.0 - hit.dist.clamp(0.0, 1.0);
        }
    }

    // 4. Sort and Limit
    let mut results: Vec<Hit> = hits.into_values()
        .filter(|h| h.combined_score > 0.0 || search_mode == "Semantic")
        .collect();
    
    results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap_or(std::cmp::Ordering::Equal));

    let final_results = results.into_iter().take(limit as usize).map(|h| SearchResult {
        entity_type: h.entity_type,
        entity_id: h.entity_id,
        distance: h.dist,
        modality: h.modality,
        metadata: h.metadata,
    }).collect();

    Ok(final_results)
}
