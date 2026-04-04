use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use crate::kms_error::{KmsError, KmsResult};
use chrono;
use crate::clipboard_repository;
use crate::utils::crypto;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::ports::{storage_keys, StoragePort, skill::SkillRepository};
use digicore_core::domain::entities::skill::{Skill, SkillMetadata, SkillScope};
use async_trait::async_trait;

/// Dimensions of `kms_note_embeddings.vec0` (see migrations; must match query vectors for text hybrid search).
pub const KMS_TEXT_EMBEDDING_VEC0_DIMENSIONS: usize = 384;

pub fn get_vault_path() -> KmsResult<PathBuf> {
    let storage = JsonFileStorageAdapter::load();
    let path_str = storage.get(storage_keys::KMS_VAULT_PATH)
        .ok_or_else(|| KmsError::Config("KMS Vault Path not configured".to_string()))?;
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
    /// Text embedding model id last written for this note (`NULL` = legacy / unknown).
    pub embedding_model_id: Option<String>,
    /// Fingerprint of model + chunk policy + vector dim when the note was last embedded (`NULL` = unknown / pre-migration).
    pub embedding_policy_sig: Option<String>,
    /// JSON array of tag strings from frontmatter (`[]` when missing).
    pub tags_json: String,
}

pub type KmsNoteMinimal = digicore_kms_ports::GraphNoteMinimal;

pub(crate) fn note_row_from_row(row: &rusqlite::Row) -> rusqlite::Result<KmsNoteRow> {
    Ok(KmsNoteRow {
        id: row.get(0)?,
        path: row.get(1)?,
        title: row.get(2)?,
        content_preview: row.get::<_, Option<String>>(3)?.and_then(|s| crypto::decrypt_local(&s)),
        last_modified: row.get(4)?,
        is_favorite: row.get::<_, i32>(5)? != 0,
        sync_status: row.get(6)?,
        last_error: row.get(7)?,
        embedding_model_id: row.get(8)?,
        embedding_policy_sig: row.get(9)?,
        tags_json: row
            .get::<_, Option<String>>(10)?
            .unwrap_or_else(|| "[]".to_string()),
    })
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct KmsDiagSummary {
    pub note_count: u32,
    pub snippet_count: u32,
    pub clip_count: u32,
    pub vector_count: u32,
    pub error_log_count: u32,
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

pub fn init(db_path: PathBuf) -> KmsResult<()> {
    if DB_CONN.get().is_some() {
        return Ok(());
    }
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ())));
    }
    let conn = Connection::open(&db_path)?;

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
        
        CREATE TABLE IF NOT EXISTS kms_skills (
            name TEXT PRIMARY KEY,
            description TEXT NOT NULL,
            version TEXT,
            author TEXT,
            tags TEXT,
            path TEXT NOT NULL,
            instructions TEXT,
            last_modified DATETIME,
            license TEXT,
            compatibility TEXT,
            extra_metadata TEXT,
            disable_model_invocation INTEGER DEFAULT 0,
            scope TEXT DEFAULT 'Global',
            sync_targets TEXT DEFAULT '[]'
        );
        "#
    ).map_err(|e| e.to_string())?;
    
    // Migration: Add missing columns if they don't exist
    let _ = conn.execute("ALTER TABLE kms_skills ADD COLUMN version TEXT", []);
    let _ = conn.execute("ALTER TABLE kms_skills ADD COLUMN author TEXT", []);
    let _ = conn.execute("ALTER TABLE kms_skills ADD COLUMN tags TEXT", []);
    let _ = conn.execute("ALTER TABLE kms_skills ADD COLUMN license TEXT", []);
    let _ = conn.execute("ALTER TABLE kms_skills ADD COLUMN compatibility TEXT", []);
    let _ = conn.execute("ALTER TABLE kms_skills ADD COLUMN extra_metadata TEXT", []);
    let _ = conn.execute("ALTER TABLE kms_skills ADD COLUMN disable_model_invocation INTEGER DEFAULT 0", []);
    let _ = conn.execute("ALTER TABLE kms_skills ADD COLUMN scope TEXT DEFAULT 'Global'", []);
    let _ = conn.execute("ALTER TABLE kms_skills ADD COLUMN sync_targets TEXT DEFAULT '[]'", []);

    // kms_notes: sync_status / last_error (migration v8); repair if an older DB skipped that migration.
    let _ = conn.execute(
        "ALTER TABLE kms_notes ADD COLUMN sync_status TEXT DEFAULT 'indexed'",
        [],
    );
    let _ = conn.execute("ALTER TABLE kms_notes ADD COLUMN last_error TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE kms_notes ADD COLUMN wiki_pagerank REAL",
        [],
    );
    // embedding_model_id / embedding_policy_sig: see tauri sql migration v17 (digicore.db).
    let _ = conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS kms_graph_meta (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );
        "#,
    );

    match DB_CONN.set(Mutex::new(conn)) {
        Ok(()) => {
            ensure_kms_notes_links_schema()?;
        }
        Err(_) => {
            // Another thread initialized first; schema already ensured there.
        }
    }
    Ok(())
}

/// Ensures `kms_notes` and `kms_links` exist (idempotent).
/// Called from [`init`] so KMS works even when the Tauri SQL migrations did not create these
/// tables (empty DB, migration skip, or rusqlite opened before plugin migrations ran).
pub fn ensure_kms_notes_links_schema() -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS kms_notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            content_preview TEXT,
            last_modified TEXT,
            is_favorite INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS kms_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_path TEXT NOT NULL,
            target_path TEXT NOT NULL,
            link_type TEXT DEFAULT 'internal',
            context TEXT,
            UNIQUE(source_path, target_path)
        );
        "#,
    )
    .map_err(|e| KmsError::General(e.to_string()))?;
    let _ = conn.execute(
        "ALTER TABLE kms_notes ADD COLUMN sync_status TEXT DEFAULT 'indexed'",
        [],
    );
    let _ = conn.execute("ALTER TABLE kms_notes ADD COLUMN last_error TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE kms_notes ADD COLUMN wiki_pagerank REAL",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE kms_notes ADD COLUMN embedding_model_id TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE kms_notes ADD COLUMN embedding_policy_sig TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE kms_notes ADD COLUMN tags_json TEXT DEFAULT '[]'",
        [],
    );
    let _ = conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS kms_graph_meta (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS kms_ui_state (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        );
        "#,
    );
    Ok(())
}

/// JSON array of vault-relative note paths, most recent first ([`KMS_UI_RECENT_PATHS_CAP`] entries max).
pub const KMS_UI_STATE_KEY_RECENT: &str = "recent_note_paths";
/// JSON array of vault-relative paths for manual favorites ordering.
pub const KMS_UI_STATE_KEY_FAVORITE_ORDER: &str = "favorite_path_order";
pub const KMS_UI_RECENT_PATHS_CAP: usize = 25;
pub const KMS_UI_FAVORITE_ORDER_CAP: usize = 500;

pub fn get_kms_ui_string_list(key: &str) -> KmsResult<Vec<String>> {
    let conn = conn_guard()?;
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM kms_ui_state WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| KmsError::General(e.to_string()))?;
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let parsed: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(parsed
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('\\', "/"))
        .collect())
}

pub fn set_kms_ui_string_list(key: &str, paths: &[String]) -> KmsResult<()> {
    let conn = conn_guard()?;
    let trimmed: Vec<String> = paths
        .iter()
        .map(|s| s.replace('\\', "/"))
        .filter(|s| !s.is_empty())
        .collect();
    let json = serde_json::to_string(&trimmed).map_err(|e| KmsError::General(e.to_string()))?;
    conn.execute(
        "INSERT INTO kms_ui_state (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, json],
    )
    .map_err(|e| KmsError::General(e.to_string()))?;
    Ok(())
}

fn rewrite_one_ui_path(p: &str, old_rel: &str, new_rel: &str) -> String {
    let p = p.replace('\\', "/");
    let old_rel = old_rel.replace('\\', "/");
    let new_rel = new_rel.replace('\\', "/");
    if p == old_rel {
        return new_rel;
    }
    let prefix = format!("{}/", old_rel);
    if p.starts_with(&prefix) {
        format!("{}/{}", new_rel, &p[prefix.len()..])
    } else {
        p
    }
}

/// When a note or folder is renamed/moved, keep sidebar lists in sync (vault-relative paths).
pub fn rewrite_kms_ui_stored_paths(old_rel: &str, new_rel: &str) -> KmsResult<()> {
    for key in [KMS_UI_STATE_KEY_RECENT, KMS_UI_STATE_KEY_FAVORITE_ORDER] {
        let paths = get_kms_ui_string_list(key)?;
        let mut changed = false;
        let next: Vec<String> = paths
            .into_iter()
            .map(|p| {
                let n = rewrite_one_ui_path(&p, old_rel, new_rel);
                if n != p {
                    changed = true;
                }
                n
            })
            .collect();
        if changed {
            set_kms_ui_string_list(key, &next)?;
        }
    }
    Ok(())
}

/// Remove a deleted note path, or a folder and all paths under it, from sidebar lists.
pub fn prune_kms_ui_path_entries(pivot_rel: &str, include_children: bool) -> KmsResult<()> {
    let norm = pivot_rel.replace('\\', "/");
    for key in [KMS_UI_STATE_KEY_RECENT, KMS_UI_STATE_KEY_FAVORITE_ORDER] {
        let paths = get_kms_ui_string_list(key)?;
        let before_len = paths.len();
        let prefix = format!("{}/", norm);
        let filtered: Vec<String> = paths
            .into_iter()
            .filter(|p| {
                let pn = p.replace('\\', "/");
                if include_children {
                    !(pn == norm || pn.starts_with(&prefix))
                } else {
                    pn != norm
                }
            })
            .collect();
        if filtered.len() != before_len {
            set_kms_ui_string_list(key, &filtered)?;
        }
    }
    Ok(())
}

/// Stable fingerprint for "how this note's text vector was produced" (normalized model id, chunk policy, output dim).
pub fn note_embedding_policy_sig(
    model_id_norm: &str,
    chunk_enabled: bool,
    chunk_max_chars: u32,
    chunk_overlap_chars: u32,
    vec_dim: usize,
) -> String {
    format!(
        "v1|{}|{}|{}|{}|{}",
        model_id_norm,
        u8::from(chunk_enabled),
        chunk_max_chars,
        chunk_overlap_chars,
        vec_dim
    )
}

pub fn init_database() -> KmsResult<()> {
    init(clipboard_repository::default_db_path())
}

fn conn_guard() -> KmsResult<std::sync::MutexGuard<'static, Connection>> {
    let conn = DB_CONN
        .get()
        .ok_or_else(|| KmsError::NotInitialized)?;
    conn.lock().map_err(|e| KmsError::General(e.to_string()))
}

/// Executes a closure within a database transaction.
pub fn transactional<F, T>(f: F) -> KmsResult<T>
where
    F: FnOnce(&Transaction) -> KmsResult<T>,
{
    let mut conn = conn_guard()?;
    let tx = conn.transaction()?;
    let result = f(&tx)?;
    tx.commit()?;
    Ok(result)
}

pub fn upsert_note(
    path: &str,
    title: &str,
    preview: &str,
    sync_status: &str,
    error: Option<&str>,
    tags: &[String],
) -> KmsResult<()> {
    let conn = conn_guard()?;
    let now = chrono::Local::now().to_rfc3339();
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO kms_notes (path, title, content_preview, last_modified, sync_status, last_error, tags_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(path) DO UPDATE SET
            title = excluded.title,
            content_preview = excluded.content_preview,
            last_modified = excluded.last_modified,
            sync_status = excluded.sync_status,
            last_error = excluded.last_error,
            tags_json = excluded.tags_json",
        params![
            path,
            title,
            crypto::encrypt_local(preview).unwrap_or_else(|_| preview.to_string()),
            now,
            sync_status,
            error,
            tags_json,
        ],
    )?;
    Ok(())
}

pub fn set_note_embedding_identity(path: &str, model_id: &str, policy_sig: &str) -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute(
        "UPDATE kms_notes SET embedding_model_id = ?1, embedding_policy_sig = ?2 WHERE path = ?3",
        params![model_id, policy_sig, path],
    )?;
    Ok(())
}

pub fn count_indexed_notes() -> KmsResult<u32> {
    let conn = conn_guard()?;
    let n: u32 = conn.query_row(
        "SELECT COUNT(*) FROM kms_notes WHERE sync_status = 'indexed'",
        [],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// All `kms_notes` rows plus breakdown by `sync_status` (indexed / pending / failed).
/// Rows with any other status string count toward `total` only; see `other_note_count` in API layer.
/// Walks the vault like sync does: counts every file (`is_file`) and markdown-eligible files (`.md` / `.markdown`).
/// Non-markdown files (images, PDFs, etc.) appear in `all_files` only, not in `kms_notes`.
pub fn count_vault_files_on_disk(vault_root: &Path) -> KmsResult<(u32, u32)> {
    if !vault_root.exists() {
        return Ok((0, 0));
    }
    fn scan_dir(dir: &Path, all_files: &mut u32, markdown_files: &mut u32) -> KmsResult<()> {
        for entry in std::fs::read_dir(dir).map_err(KmsError::from)?.flatten() {
            let p = entry.path();
            if p.is_dir() {
                scan_dir(&p, all_files, markdown_files)?;
            } else if p.is_file() {
                *all_files = all_files.saturating_add(1);
                if p.extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("md") || s.eq_ignore_ascii_case("markdown"))
                    .unwrap_or(false)
                {
                    *markdown_files = markdown_files.saturating_add(1);
                }
            }
        }
        Ok(())
    }
    let mut all: u32 = 0;
    let mut md: u32 = 0;
    scan_dir(vault_root, &mut all, &mut md)?;
    Ok((all, md))
}

pub fn count_kms_notes_sync_breakdown() -> KmsResult<(u32, u32, u32, u32)> {
    let conn = conn_guard()?;
    let (total, indexed, pending, failed): (i64, i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN sync_status = 'indexed' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN sync_status = 'pending' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN sync_status = 'failed' THEN 1 ELSE 0 END), 0)
         FROM kms_notes",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    Ok((
        total as u32,
        indexed as u32,
        pending as u32,
        failed as u32,
    ))
}

pub fn count_notes_needing_embedding_migration(
    target_model_id: &str,
    target_policy_sig: &str,
) -> KmsResult<u32> {
    let conn = conn_guard()?;
    let n: u32 = conn.query_row(
        "SELECT COUNT(*) FROM kms_notes WHERE sync_status = 'indexed' AND (embedding_model_id IS NULL OR embedding_model_id != ?1 OR embedding_policy_sig IS NULL OR embedding_policy_sig != ?2)",
        params![target_model_id, target_policy_sig],
        |r| r.get(0),
    )?;
    Ok(n)
}

pub fn list_note_paths_for_embedding_migration(
    target_model_id: &str,
    target_policy_sig: &str,
    limit: u32,
) -> KmsResult<Vec<String>> {
    let conn = conn_guard()?;
    let lim = limit.max(1) as i64;
    let mut stmt = conn.prepare(
        "SELECT path FROM kms_notes WHERE sync_status = 'indexed' AND (embedding_model_id IS NULL OR embedding_model_id != ?1 OR embedding_policy_sig IS NULL OR embedding_policy_sig != ?2) ORDER BY path LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![target_model_id, target_policy_sig, lim], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Indexed note paths paged by stable `ORDER BY path` (manual full-vault re-embed must advance `offset` each batch).
pub fn list_indexed_note_paths(limit: u32, offset: u32) -> KmsResult<Vec<String>> {
    let conn = conn_guard()?;
    let lim = limit.max(1) as i64;
    let off = offset as i64;
    let mut stmt = conn.prepare(
        "SELECT path FROM kms_notes WHERE sync_status = 'indexed' ORDER BY path LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt.query_map(params![lim, off], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn list_notes() -> KmsResult<Vec<KmsNoteRow>> {
    let conn = conn_guard()?;
    let mut stmt = conn.prepare(
        "SELECT id, path, title, content_preview, last_modified, is_favorite, sync_status, last_error, embedding_model_id, embedding_policy_sig, COALESCE(tags_json, '[]') FROM kms_notes ORDER BY last_modified DESC",
    )?;
    let rows = stmt.query_map([], note_row_from_row)?;

    let mut notes = Vec::new();
    for note in rows {
        notes.push(note?);
    }
    Ok(notes)
}

pub fn update_index_status(entity_type: &str, entity_id: &str, status: &str, error: Option<&str>) -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute(
        "INSERT INTO kms_index_status (entity_type, entity_id, status, error, updated_at)
         VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET
            status = excluded.status,
            error = excluded.error,
            updated_at = CURRENT_TIMESTAMP",
        params![entity_type, entity_id, status, error],
    )?;
    Ok(())
}

pub fn get_detailed_status(category: &str) -> KmsResult<Vec<KmsIndexStatusRow>> {
    let conn = conn_guard()?;
    
    if category == "notes" {
        let mut stmt = conn.prepare("SELECT path, 'notes', sync_status, last_error, last_modified FROM kms_notes WHERE sync_status = 'failed'")?;
        let rows = stmt.query_map([], |row| {
            Ok(KmsIndexStatusRow {
                entity_id: row.get(0)?,
                entity_type: row.get(1)?,
                status: row.get(2)?,
                error: row.get(3)?,
                updated_at: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        })?;
        
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        return Ok(results);
    }

    let mut stmt = conn.prepare("SELECT entity_id, entity_type, status, error, updated_at FROM kms_index_status WHERE entity_type = ?1")?;
    let rows = stmt.query_map(params![category], |row| {
        Ok(KmsIndexStatusRow {
            entity_id: row.get(0)?,
            entity_type: row.get(1)?,
            status: row.get(2)?,
            error: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;

    let mut results = Vec::new();
    for r in rows {
        results.push(r?);
    }
    Ok(results)
}

pub fn get_category_counts(category: &str) -> KmsResult<(u32, u32, u32)> {
    let conn = conn_guard()?;
    
    if category == "notes" {
        let total: u32 = conn.query_row("SELECT COUNT(*) FROM kms_notes", [], |r| r.get(0))?;
        let indexed: u32 = conn.query_row("SELECT COUNT(*) FROM kms_notes WHERE sync_status = 'indexed'", [], |r| r.get(0))?;
        let failed: u32 = conn.query_row("SELECT COUNT(*) FROM kms_notes WHERE sync_status = 'failed'", [], |r| r.get(0))?;
        return Ok((indexed, failed, total));
    }
    
    // For snippets/clipboard, we need to know the total baseline.
    // For now, let's use the counts from the source tables.
    let total: u32 = if category == "snippets" {
        conn.query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get(0))?
    } else if category == "clipboard" {
        conn.query_row("SELECT COUNT(*) FROM clipboard_history", [], |r| r.get(0))?
    } else {
        0
    };
    
    let indexed: u32 = conn.query_row("SELECT COUNT(*) FROM kms_index_status WHERE entity_type = ?1 AND status = 'indexed'", params![category], |r| r.get(0))?;
    let failed: u32 = conn.query_row("SELECT COUNT(*) FROM kms_index_status WHERE entity_type = ?1 AND status = 'failed'", params![category], |r| r.get(0))?;
    
    Ok((indexed, failed, total))
}

pub fn get_diag_summary() -> KmsResult<KmsDiagSummary> {
    let conn = conn_guard()?;
    
    let note_count: u32 = conn.query_row("SELECT COUNT(*) FROM kms_notes", [], |r| r.get(0)).unwrap_or(0);
    let snippet_count: u32 = conn.query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get(0)).unwrap_or(0);
    let clip_count: u32 = conn.query_row("SELECT COUNT(*) FROM clipboard_history", [], |r| r.get(0)).unwrap_or(0);
    let error_log_count: u32 = conn.query_row("SELECT COUNT(*) FROM kms_logs WHERE level = 'error' OR level = 'warn'", [], |r| r.get(0)).unwrap_or(0);
    
    let vector_count: u32 = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM kms_embeddings_text) + (SELECT COUNT(*) FROM kms_embeddings_image)",
        [],
        |r| r.get(0)
    ).unwrap_or(0);

    Ok(KmsDiagSummary {
        note_count,
        snippet_count,
        clip_count,
        vector_count,
        error_log_count,
    })
}

pub fn insert_log(level: &str, message: &str, details: Option<&str>) -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute(
        "INSERT INTO kms_logs (level, message, details) VALUES (?1, ?2, ?3)",
        params![level, message, details],
    )?;
    Ok(())
}

pub fn list_logs(limit: u32) -> KmsResult<Vec<KmsLog>> {
    let conn = conn_guard()?;
    let mut stmt = conn.prepare("SELECT id, level, message, details, timestamp FROM kms_logs ORDER BY id DESC LIMIT ?1")?;
    
    let rows = stmt.query_map(params![limit], |row| {
        Ok(KmsLog {
            id: row.get(0)?,
            level: row.get(1)?,
            message: row.get(2)?,
            details: row.get(3)?,
            timestamp: row.get(4)?,
        })
    })?;
    
    let mut logs = Vec::new();
    for r in rows {
        logs.push(r?);
    }
    Ok(logs)
}

pub fn clear_logs() -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute("DELETE FROM kms_logs", [])?;
    Ok(())
}

pub fn delete_note(path: &str) -> KmsResult<()> {
    transactional(|tx| {
        // 1. Find vector IDs to delete
        let mut stmt = tx.prepare("SELECT vec_id FROM kms_vector_map WHERE entity_type = 'note' AND entity_id = ?1")?;
        let vec_ids: Vec<i64> = stmt.query_map(params![path], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // 2. Delete from vector tables
        for vid in vec_ids {
            let _ = tx.execute("DELETE FROM kms_embeddings_text WHERE rowid = ?1", params![vid]);
            let _ = tx.execute("DELETE FROM kms_embeddings_image WHERE rowid = ?1", params![vid]);
        }
        
        // 3. Delete from support tables (mapping, links)
        tx.execute("DELETE FROM kms_vector_map WHERE entity_type = 'note' AND entity_id = ?1", params![path])?;
        tx.execute("DELETE FROM kms_links WHERE source_path = ?1 OR target_path = ?1", params![path])?;
        
        // 4. Delete from main table (triggers triggers for FTS, and CASCADE for tags)
        tx.execute("DELETE FROM kms_notes WHERE path = ?1", params![path])?;
            
        Ok(())
    })
}

pub fn repair_database() -> KmsResult<()> {
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
    let _ = conn.execute("DELETE FROM _sqlx_migrations WHERE version IN (4, 5, 6, 7)", []);
    
    // Attempt VACUUM
    let _ = conn.execute("VACUUM", []);

    log::info!("[KMS] Surgical repair complete. KMS Tables dropped and migrations reset.");
    
    Ok(())
}

/// Sets `is_favorite` for an indexed note row (vault-relative `path` as stored in `kms_notes`).
/// Returns the number of rows updated (0 if no matching path).
pub fn set_note_favorite(path: &str, favorite: bool) -> KmsResult<usize> {
    let conn = conn_guard()?;
    let flag: i32 = if favorite { 1 } else { 0 };
    let n = conn.execute(
        "UPDATE kms_notes SET is_favorite = ?1 WHERE path = ?2",
        params![flag, path],
    )?;
    Ok(n)
}

pub fn rename_note(old_path: &str, new_path: &str, new_title: &str) -> KmsResult<()> {
    transactional(|tx| {
        // 1. Update the note metadata
        tx.execute(
            "UPDATE kms_notes SET path = ?1, title = ?2 WHERE path = ?3",
            params![new_path, new_title, old_path],
        )?;

        // 2. Update the vector mapping (path is the entity_id for notes)
        tx.execute(
            "UPDATE kms_vector_map SET entity_id = ?1 WHERE entity_type = 'note' AND entity_id = ?2",
            params![new_path, old_path]
        )?;

        Ok(())
    })
}

pub fn rename_folder(old_path: &str, new_path: &str) -> KmsResult<()> {
    transactional(|tx| {
        // 1. Update all notes within this folder
        tx.execute(
            "UPDATE kms_notes SET path = ?1 || SUBSTR(path, LENGTH(?2) + 1) WHERE path LIKE ?2 || '%'",
            params![new_path, old_path],
        )?;

        // 2. Update vector mapping entity IDs
        tx.execute(
            "UPDATE kms_vector_map SET entity_id = ?1 || SUBSTR(entity_id, LENGTH(?2) + 1) 
             WHERE entity_type = 'note' AND entity_id LIKE ?2 || '%'",
            params![new_path, old_path]
        )?;

        // 3. Update links (both source and target)
        tx.execute(
            "UPDATE kms_links SET source_path = ?1 || SUBSTR(source_path, LENGTH(?2) + 1) 
             WHERE source_path LIKE ?2 || '%'",
            params![new_path, old_path]
        )?;

        tx.execute(
            "UPDATE kms_links SET target_path = ?1 || SUBSTR(target_path, LENGTH(?2) + 1) 
             WHERE target_path LIKE ?2 || '%'",
            params![new_path, old_path]
        )?;

        Ok(())
    })
}

pub fn delete_folder_recursive(path: &str) -> KmsResult<()> {
    transactional(|tx| {
        // 1. Find all notes within this folder
        let mut stmt = tx.prepare("SELECT path FROM kms_notes WHERE path LIKE ?1 || '%'")?;
        
        let note_paths: Vec<String> = stmt.query_map(params![path], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // 2. Delete each note individually
        // Since we are already in a transaction, we should call a variant of delete_note that takes a Transaction
        // But for simplicity, we can just execute the SQL here or refactor delete_note.
        // Let's refactor delete_note to have a transactional internal version.
        
        for note_path in note_paths {
            // Reusing the logic from delete_note but on the current transaction
            let mut stmt_vec = tx.prepare("SELECT vec_id FROM kms_vector_map WHERE entity_type = 'note' AND entity_id = ?1")?;
            let vec_ids: Vec<i64> = stmt_vec.query_map(params![note_path], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            for vid in vec_ids {
                let _ = tx.execute("DELETE FROM kms_embeddings_text WHERE rowid = ?1", params![vid]);
                let _ = tx.execute("DELETE FROM kms_embeddings_image WHERE rowid = ?1", params![vid]);
            }
            
            tx.execute("DELETE FROM kms_vector_map WHERE entity_type = 'note' AND entity_id = ?1", params![note_path])?;
            tx.execute("DELETE FROM kms_links WHERE source_path = ?1 OR target_path = ?1", params![note_path])?;
            tx.execute("DELETE FROM kms_notes WHERE path = ?1", params![note_path])?;
        }
        
        Ok(())
    })
}

pub fn upsert_link(source_path: &str, target_path: &str) -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute(
        "INSERT INTO kms_links (source_path, target_path) VALUES (?1, ?2) ON CONFLICT(source_path, target_path) DO NOTHING",
        params![source_path, target_path],
    )?;
    Ok(())
}

pub fn delete_links_for_source(source_path: &str) -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute("DELETE FROM kms_links WHERE source_path = ?1", params![source_path])?;
    Ok(())
}

pub fn get_links_for_note(path: &str) -> KmsResult<(Vec<KmsNoteRow>, Vec<KmsNoteRow>)> {
    let conn = conn_guard()?;
    
    // Outgoing
    let mut stmt = conn.prepare(
        "SELECT n.id, n.path, n.title, n.content_preview, n.last_modified, n.is_favorite, n.sync_status, n.last_error, n.embedding_model_id, n.embedding_policy_sig, COALESCE(n.tags_json, '[]')
         FROM kms_notes n
         JOIN kms_links l ON n.path = l.target_path
         WHERE l.source_path = ?1"
    )?;
    
    let outgoing = stmt.query_map(params![path], note_row_from_row)?
    .collect::<Result<Vec<_>, _>>()?;

    // Incoming (Backlinks)
    let mut stmt = conn.prepare(
        "SELECT n.id, n.path, n.title, n.content_preview, n.last_modified, n.is_favorite, n.sync_status, n.last_error, n.embedding_model_id, n.embedding_policy_sig, COALESCE(n.tags_json, '[]')
         FROM kms_notes n
         JOIN kms_links l ON n.path = l.source_path
         WHERE l.target_path = ?1"
    )?;
    
    let incoming = stmt.query_map(params![path], note_row_from_row)?
    .collect::<Result<Vec<_>, _>>()?;

    Ok((outgoing, incoming))
}

pub fn update_links_on_path_change(old_path: &str, new_path: &str) -> KmsResult<()> {
    let conn = conn_guard()?;
    // Update target paths (incoming links to the renamed note)
    conn.execute("UPDATE kms_links SET target_path = ?1 WHERE target_path = ?2", params![new_path, old_path])?;
    // Update source paths (outgoing links from the renamed note)
    conn.execute("UPDATE kms_links SET source_path = ?1 WHERE source_path = ?2", params![new_path, old_path])?;
    Ok(())
}

#[allow(dead_code)]
pub fn get_note_by_path(path: &str) -> KmsResult<Option<KmsNoteRow>> {
    let conn = conn_guard()?;
    let mut stmt = conn.prepare(
        "SELECT id, path, title, content_preview, last_modified, is_favorite, sync_status, last_error, embedding_model_id, embedding_policy_sig, COALESCE(tags_json, '[]') FROM kms_notes WHERE path = ?1",
    )?;
    stmt.query_row(params![path], note_row_from_row)
    .optional()
    .map_err(KmsError::from)
}

pub fn upsert_embedding(
    modality: &str,
    entity_type: &str,
    entity_id: &str,
    embedding: &[f32],
    metadata: Option<String>,
) -> KmsResult<()> {
    transactional(|tx| {
        // 1. Check if we already have a vec_id for this entity/modality combination
        let mut stmt = tx.prepare("SELECT vec_id FROM kms_vector_map WHERE entity_type = ?1 AND entity_id = ?2 AND modality = ?3")?;
        let existing_id: Option<i64> = stmt.query_row(params![entity_type, entity_id, modality], |r| r.get(0)).optional()?;

        let vec_id = match existing_id {
            Some(id) => id,
            None => {
                tx.query_row("SELECT COALESCE(MAX(vec_id), 0) + 1 FROM kms_vector_map", [], |r| r.get(0))?
            }
        };

        // 2. Convert f32 slice to bytes for sqlite-vec
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                embedding.as_ptr() as *const u8,
                embedding.len() * std::mem::size_of::<f32>(),
            )
        };

        // 3. Insert into the appropriate vector table (sqlite-vec VTAB: UPSERT / ON CONFLICT is not supported)
        if modality == "text" {
            if existing_id.is_some() {
                tx.execute(
                    "DELETE FROM kms_embeddings_text WHERE rowid = ?1",
                    params![vec_id],
                )?;
            }
            tx.execute(
                "INSERT INTO kms_embeddings_text (rowid, embedding) VALUES (?1, ?2)",
                params![vec_id, bytes],
            )?;
        } else {
            if existing_id.is_some() {
                tx.execute(
                    "DELETE FROM kms_embeddings_image WHERE rowid = ?1",
                    params![vec_id],
                )?;
            }
            tx.execute(
                "INSERT INTO kms_embeddings_image (rowid, embedding) VALUES (?1, ?2)",
                params![vec_id, bytes],
            )?;
        }

        // 4. Mapping row: table has no UNIQUE on vec_id (PK is id), so ON CONFLICT(vec_id) is invalid.
        if existing_id.is_some() {
            tx.execute(
                "UPDATE kms_vector_map SET metadata = ?1 WHERE vec_id = ?2",
                params![metadata, vec_id],
            )?;
        } else {
            tx.execute(
                "INSERT INTO kms_vector_map (vec_id, modality, entity_type, entity_id, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![vec_id, modality, entity_type, entity_id, metadata],
            )?;
        }

        Ok(())
    })
}

#[allow(dead_code)]
pub fn delete_embedding(vec_id: i64, entity_type: &str) -> KmsResult<()> {
    transactional(|tx| {
        if entity_type == "note" || entity_type == "snippet" {
            tx.execute("DELETE FROM kms_embeddings_text WHERE rowid = ?1", params![vec_id])?;
        } else {
            tx.execute("DELETE FROM kms_embeddings_image WHERE rowid = ?1", params![vec_id])?;
        }
        tx.execute("DELETE FROM kms_vector_map WHERE vec_id = ?1", params![vec_id])?;
        Ok(())
    })
}

pub fn upsert_unified_fts(entity_type: &str, entity_id: &str, title: &str, content: &str) -> KmsResult<()> {
    let conn = conn_guard()?;
    // FTS5 doesn't support UPSERT, so we delete and then insert
    conn.execute("DELETE FROM kms_unified_fts WHERE entity_type = ?1 AND entity_id = ?2", params![entity_type, entity_id])?;
    conn.execute(
        "INSERT INTO kms_unified_fts (entity_type, entity_id, title, content) VALUES (?1, ?2, ?3, ?4)",
        params![entity_type, entity_id, title, content],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn delete_unified_fts(entity_type: &str, entity_id: &str) -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute("DELETE FROM kms_unified_fts WHERE entity_type = ?1 AND entity_id = ?2", params![entity_type, entity_id])?;
    Ok(())
}

pub fn delete_embeddings_for_entity(entity_type: &str, entity_id: &str) -> KmsResult<()> {
    transactional(|tx| {
        let mut stmt = tx.prepare("SELECT vec_id FROM kms_vector_map WHERE entity_id = ?1 AND entity_type = ?2")?;
        let vec_ids: Vec<i64> = stmt.query_map(params![entity_id, entity_type], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        
        for vid in vec_ids {
            if entity_type == "note" || entity_type == "snippet" {
                tx.execute("DELETE FROM kms_embeddings_text WHERE rowid = ?1", params![vid])?;
            } else {
                tx.execute("DELETE FROM kms_embeddings_image WHERE rowid = ?1", params![vid])?;
            }
        }
        tx.execute("DELETE FROM kms_vector_map WHERE entity_id = ?1 AND entity_type = ?2", params![entity_id, entity_type])?;
        if entity_type == "note" {
            let _ = tx.execute(
                "UPDATE kms_notes SET embedding_model_id = NULL, embedding_policy_sig = NULL WHERE path = ?1",
                params![entity_id],
            );
        }
        // Also clean up index status
        tx.execute("DELETE FROM kms_index_status WHERE entity_type = ?1 AND entity_id = ?2", params![entity_type, entity_id])?;
        Ok(())
    })
}

pub fn delete_all_embeddings_for_type(entity_type: &str) -> KmsResult<()> {
    transactional(|tx| {
        if entity_type == "note" || entity_type == "snippet" {
            tx.execute("DELETE FROM kms_embeddings_text WHERE rowid IN (SELECT vec_id FROM kms_vector_map WHERE entity_type = ?1)", params![entity_type])?;
        } else {
            tx.execute("DELETE FROM kms_embeddings_image WHERE rowid IN (SELECT vec_id FROM kms_vector_map WHERE entity_type = ?1)", params![entity_type])?;
        }
        tx.execute("DELETE FROM kms_vector_map WHERE entity_type = ?1", params![entity_type])?;
        Ok(())
    })
}


/// Performs a multi-modal search using k-NN, FTS5, or both (Hybrid Search).
///
/// When `min_vector_similarity` > 0, drops hits that have a vector rank and estimated similarity
/// `1 - dist` (with `dist` clamped to 0..1) below the threshold.
pub fn search_hybrid(
    query: &str,
    modality: &str,
    query_vector: Vec<f32>,
    search_mode: &str, // "Hybrid", "Semantic", "Keyword"
    limit: u32,
    min_vector_similarity: f32,
) -> KmsResult<Vec<SearchResult>> {
    if modality == "text" && query_vector.len() != KMS_TEXT_EMBEDDING_VEC0_DIMENSIONS {
        return Err(KmsError::Validation(format!(
            "Text query embedding has {} dimensions; KMS text index expects {}. Re-embed the vault or use a model that outputs {}-dimensional vectors for this schema.",
            query_vector.len(),
            KMS_TEXT_EMBEDDING_VEC0_DIMENSIONS,
            KMS_TEXT_EMBEDDING_VEC0_DIMENSIONS
        )));
    }

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
        ))?;

        let vec_rows = stmt.query_map(params![query_bytes, limit * 2], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)? as f32,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;

        for (rank_idx, row) in vec_rows.enumerate() {
            let (entity_type, entity_id, distance, metadata) = row?;
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
                "SELECT f.entity_type, f.entity_id, bm25(kms_unified_fts) as score, m.metadata, m.entity_type as hit_modality
                 FROM kms_unified_fts f
                 LEFT JOIN kms_vector_map m ON f.entity_type = m.entity_type AND f.entity_id = m.entity_id
                 WHERE kms_unified_fts MATCH ?1
                 ORDER BY score
                 LIMIT ?2"
            )?;
            
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

    if min_vector_similarity > 0.0 {
        hits.retain(|_, h| {
            if h.vec_rank.is_none() {
                return true;
            }
            let sim = 1.0 - h.dist.clamp(0.0, 1.0);
            sim >= min_vector_similarity
        });
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

// --- Skill Management ---

pub struct KmsSkillRepository;

#[async_trait]
impl SkillRepository for KmsSkillRepository {
    async fn list_skills(&self) -> anyhow::Result<Vec<Skill>> {
        let conn = conn_guard()?;
        let mut stmt = conn.prepare(
            "SELECT name, description, version, path, instructions, author, tags, license, compatibility, extra_metadata, disable_model_invocation, scope, sync_targets FROM kms_skills ORDER BY name ASC"
        )?;
        
        let rows = stmt.query_map([], |row| {
            let scope_str: String = row.get(11)?;
            let scope = if scope_str == "Project" { SkillScope::Project } else { SkillScope::Global };
            
            Ok(Skill {
                metadata: SkillMetadata {
                    name: row.get(0)?,
                    description: row.get(1)?,
                    version: row.get(2)?,
                    author: row.get(5)?,
                    tags: serde_json::from_str(&row.get::<_, String>(6).unwrap_or_else(|_| "[]".to_string())).unwrap_or_default(),
                    license: row.get(7)?,
                    compatibility: row.get(8)?,
                    extra_metadata: row.get::<_, Option<String>>(9)?.and_then(|s| serde_json::from_str(&s).ok()),
                    disable_model_invocation: Some(row.get::<_, i32>(10)? != 0),
                    scope,
                    sync_targets: serde_json::from_str(&row.get::<_, String>(12).unwrap_or_else(|_| "[]".to_string())).unwrap_or_default(),
                },
                path: PathBuf::from(row.get::<_, String>(3)?),
                instructions: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                resources: Vec::new(), // Populated elsewhere if needed
            })
        })?;

        let mut skills = Vec::new();
        for skill in rows {
            skills.push(skill?);
        }
        Ok(skills)
    }

    async fn get_skill(&self, name: &str) -> anyhow::Result<Option<Skill>> {
        let conn = conn_guard()?;
        let mut stmt = conn.prepare(
            "SELECT name, description, version, path, instructions, author, tags, license, compatibility, extra_metadata, disable_model_invocation, scope, sync_targets FROM kms_skills WHERE name = ?1"
        )?;
        
        let skill = stmt.query_row(params![name], |row| {
            let scope_str: String = row.get(11)?;
            let scope = if scope_str == "Project" { SkillScope::Project } else { SkillScope::Global };

            Ok(Skill {
                metadata: SkillMetadata {
                    name: row.get(0)?,
                    description: row.get(1)?,
                    version: row.get(2)?,
                    author: row.get(5)?,
                    tags: serde_json::from_str(&row.get::<_, String>(6).unwrap_or_else(|_| "[]".to_string())).unwrap_or_default(),
                    license: row.get(7)?,
                    compatibility: row.get(8)?,
                    extra_metadata: row.get::<_, Option<String>>(9)?.and_then(|s| serde_json::from_str(&s).ok()),
                    disable_model_invocation: Some(row.get::<_, i32>(10)? != 0),
                    scope,
                    sync_targets: serde_json::from_str(&row.get::<_, String>(12).unwrap_or_else(|_| "[]".to_string())).unwrap_or_default(),
                },
                path: PathBuf::from(row.get::<_, String>(3)?),
                instructions: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                resources: Vec::new(),
            })
        }).optional()?;

        Ok(skill)
    }

    async fn save_skill(&self, skill: &Skill) -> anyhow::Result<()> {
        let log_msg = format!("save_skill: name={}, desc_len={}, inst_len={}", 
            skill.metadata.name, 
            skill.metadata.description.len(),
            skill.instructions.len()
        );
        let log_details = format!("desc='{}', inst='{}'", 
            skill.metadata.description.replace("'", "''"), 
            skill.instructions.replace("'", "''")
        );
        
        let conn = conn_guard()?;
        
        let _ = conn.execute(
            "INSERT INTO kms_logs (level, message, details) VALUES (?, ?, ?)",
            params!["INFO", log_msg, log_details]
        );

        let now = chrono::Local::now().to_rfc3339();
        
        // Ensure managed directory exists
        let vault = self.vault_path();
        let skill_dir = vault.join("skills").join(&skill.metadata.name);
        if !skill_dir.exists() {
            std::fs::create_dir_all(&skill_dir).map_err(|e| anyhow::anyhow!("Failed to create skill directory: {}", e))?;
        }
        
        // Write SKILL.md
        let markdown = skill.to_markdown()?;
        std::fs::write(skill_dir.join("SKILL.md"), markdown).map_err(|e| anyhow::anyhow!("Failed to write SKILL.md: {}", e))?;

        let scope_str = match skill.metadata.scope {
            digicore_core::domain::entities::skill::SkillScope::Global => "Global",
            digicore_core::domain::entities::skill::SkillScope::Project => "Project",
        };

        conn.execute(
            "INSERT INTO kms_skills (name, description, version, path, instructions, last_modified, author, tags, license, compatibility, extra_metadata, disable_model_invocation, scope, sync_targets)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(name) DO UPDATE SET
                description = excluded.description,
                version = excluded.version,
                path = excluded.path,
                instructions = excluded.instructions,
                last_modified = excluded.last_modified,
                author = excluded.author,
                tags = excluded.tags,
                license = excluded.license,
                compatibility = excluded.compatibility,
                extra_metadata = excluded.extra_metadata,
                disable_model_invocation = excluded.disable_model_invocation,
                scope = excluded.scope,
                sync_targets = excluded.sync_targets",
            params![
                skill.metadata.name,
                skill.metadata.description,
                skill.metadata.version,
                skill_dir.to_string_lossy(),
                skill.instructions,
                now,
                skill.metadata.author,
                serde_json::to_string(&skill.metadata.tags).unwrap_or_else(|_| "[]".to_string()),
                skill.metadata.license,
                skill.metadata.compatibility,
                skill.metadata.extra_metadata.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default()),
                if skill.metadata.disable_model_invocation.unwrap_or(false) { 1 } else { 0 },
                scope_str,
                serde_json::to_string(&skill.metadata.sync_targets).unwrap_or_else(|_| "[]".to_string())
            ],
        )?;
        Ok(())
    }

    async fn delete_skill(&self, name: &str) -> anyhow::Result<()> {
        let conn = conn_guard()?;
        conn.execute("DELETE FROM kms_skills WHERE name = ?", [name])?;
        
        // Also clean up unified FTS (handled by trigger, but vector map needs manual cleanup if indexed)
        // Cleanup vector map for 'skill' entity
        let _ = conn.execute(
            "DELETE FROM kms_vector_map WHERE entity_type = 'skill' AND entity_id = ?1",
            params![name]
        );
        
        Ok(())
    }

    async fn delete_skill_by_path(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let name = self.find_skill_name_by_path(path).await?;
        if let Some(entry_name) = name {
            self.delete_skill(&entry_name).await?;
        }
        
        Ok(())
    }

    async fn find_skill_name_by_path(&self, path: &std::path::Path) -> anyhow::Result<Option<String>> {
        let conn = conn_guard()?;
        let path_str = path.to_string_lossy();
        
        let name: Option<String> = conn.query_row(
            "SELECT name FROM kms_skills WHERE path = ?1",
            params![path_str],
            |row| row.get(0)
        ).optional()?;
        
        Ok(name)
    }

    async fn refresh(&self) -> anyhow::Result<()> {
        let vault_path = get_vault_path().map_err(|e| anyhow::anyhow!(e))?;
        let skills_dir = vault_path.join("skills");
        
        if !skills_dir.exists() {
            return Ok(());
        }

        let mut discovered_names = Vec::new();

        for entry in std::fs::read_dir(&skills_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                match Skill::from_dir(path.clone()) {
                    Ok(skill) => {
                        discovered_names.push(skill.metadata.name.clone());
                        self.save_skill(&skill).await?;
                    }
                    Err(e) => {
                        log::warn!("[KMS][Skills] Failed to parse skill at {:?}: {}", path, e);
                    }
                }
            }
        }

        // Cleanup: remove skills from DB that were not found on disk
        let conn = conn_guard()?;
        let mut stmt = conn.prepare("SELECT name FROM kms_skills")?;
        let db_names_iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
        
        let mut to_delete = Vec::new();
        for name_res in db_names_iter {
            let name = name_res?;
            if !discovered_names.contains(&name) {
                to_delete.push(name);
            }
        }

        for name in to_delete {
            log::info!("[KMS][Skills] Removing stale skill from DB: {}", name);
            conn.execute("DELETE FROM kms_skills WHERE name = ?1", params![name])?;
            let _ = update_index_status("skills", &name, "deleted", None);
        }

        Ok(())
    }

    fn vault_path(&self) -> PathBuf {
        get_vault_path().unwrap_or_default()
    }
}
pub fn get_all_links() -> KmsResult<Vec<(String, String)>> {
    let conn = conn_guard()?;
    let mut stmt = conn.prepare("SELECT source_path, target_path FROM kms_links")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    
    let mut links = Vec::new();
    for link in rows {
        links.push(link?);
    }
    Ok(links)
}

/// All wiki-link rows incident to any of the given vault-relative note paths (deduped).
/// Used for incremental local-graph BFS instead of loading the full link table.
pub fn get_links_for_notes(paths: &[String]) -> KmsResult<Vec<(String, String)>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let conn = conn_guard()?;
    let mut seen_rel = std::collections::HashSet::<&str>::new();
    let mut deduped: Vec<&String> = Vec::new();
    for p in paths {
        if seen_rel.insert(p.as_str()) {
            deduped.push(p);
        }
    }
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen_pairs = std::collections::HashSet::<(String, String)>::new();
    for rel in deduped {
        let mut stmt = conn.prepare(
            "SELECT source_path, target_path FROM kms_links WHERE source_path = ?1 OR target_path = ?1",
        )?;
        let rows = stmt.query_map(params![rel], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for r in rows {
            let pair = r?;
            if seen_pairs.insert(pair.clone()) {
                out.push(pair);
            }
        }
    }
    Ok(out)
}

pub fn get_all_notes_minimal() -> KmsResult<Vec<KmsNoteMinimal>> {
    let conn = conn_guard()?;
    let mut stmt =
        conn.prepare("SELECT id, path, title, last_modified, wiki_pagerank FROM kms_notes")?;
    let rows = stmt.query_map([], |row| {
        let pr: Option<f64> = row.get(4)?;
        Ok(KmsNoteMinimal {
            id: row.get(0)?,
            path: row.get(1)?,
            title: row.get(2)?,
            last_modified: row.get(3)?,
            wiki_pagerank: pr.map(|x| x as f32),
        })
    })?;
    
    let mut notes = Vec::new();
    for note in rows {
        notes.push(note?);
    }
    Ok(notes)
}

const SQLITE_IN_CHUNK: usize = 400;

/// Minimal note rows for vault-relative paths (batch `IN` clauses). Order not preserved.
pub fn get_notes_minimal_by_paths(paths: &[String]) -> KmsResult<Vec<KmsNoteMinimal>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let conn = conn_guard()?;
    let mut seen = std::collections::HashSet::<String>::new();
    let mut unique: Vec<&String> = Vec::new();
    for p in paths {
        if seen.insert(p.clone()) {
            unique.push(p);
        }
    }
    let mut out: Vec<KmsNoteMinimal> = Vec::new();
    for chunk in unique.chunks(SQLITE_IN_CHUNK) {
        let placeholders = chunk
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, path, title, last_modified, wiki_pagerank FROM kms_notes WHERE path IN ({placeholders})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = chunk
            .iter()
            .map(|s| *s as &dyn rusqlite::ToSql)
            .collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            let pr: Option<f64> = row.get(4)?;
            Ok(KmsNoteMinimal {
                id: row.get(0)?,
                path: row.get(1)?,
                title: row.get(2)?,
                last_modified: row.get(3)?,
                wiki_pagerank: pr.map(|x| x as f32),
            })
        })?;
        for r in rows {
            out.push(r?);
        }
    }
    Ok(out)
}

/// Clears materialized wiki PageRank for all notes (repair / future admin RPC).
#[allow(dead_code)]
pub fn clear_wiki_pagerank_scores() -> KmsResult<u32> {
    let conn = conn_guard()?;
    let n = conn.execute("UPDATE kms_notes SET wiki_pagerank = NULL", [])?;
    Ok(n as u32)
}

/// Meta key: SHA-256 hex fingerprint of the wiki edge set used for materialized PageRank.
pub const KMS_GRAPH_META_WIKI_PR_FP: &str = "wiki_pr_edge_fp_v1";

pub fn kms_graph_meta_get(key: &str) -> KmsResult<Option<String>> {
    let conn = conn_guard()?;
    let v: Option<String> = conn
        .query_row(
            "SELECT value FROM kms_graph_meta WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(v)
}

pub fn kms_graph_meta_upsert(key: &str, value: &str) -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute(
        "INSERT INTO kms_graph_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn kms_graph_meta_delete(key: &str) -> KmsResult<()> {
    let conn = conn_guard()?;
    conn.execute("DELETE FROM kms_graph_meta WHERE key = ?1", params![key])?;
    Ok(())
}

/// Marks materialized wiki PageRank as stale (wiki link graph changed).
pub fn clear_wiki_pagerank_fingerprint() -> KmsResult<()> {
    kms_graph_meta_delete(KMS_GRAPH_META_WIKI_PR_FP)
}

/// Persists full-vault wiki PageRank scores (vault-relative paths).
pub fn bulk_set_wiki_pagerank(rel_paths_and_scores: &[(String, f32)]) -> KmsResult<()> {
    if rel_paths_and_scores.is_empty() {
        return Ok(());
    }
    let mut conn = conn_guard()?;
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("UPDATE kms_notes SET wiki_pagerank = ?1 WHERE path = ?2")?;
        for (path, score) in rel_paths_and_scores {
            stmt.execute(params![*score as f64, path])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn f32_vec_from_embedding_blob(blob: &[u8]) -> Vec<f32> {
    let f32_count = blob.len() / 4;
    let mut embedding = Vec::with_capacity(f32_count);
    for i in 0..f32_count {
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&blob[i * 4..(i + 1) * 4]);
        embedding.push(f32::from_le_bytes(bytes));
    }
    embedding
}

pub fn get_all_note_embeddings() -> KmsResult<Vec<(String, Vec<f32>)>> {
    let conn = conn_guard()?;
    let mut stmt = conn.prepare(
        "SELECT m.entity_id, v.embedding 
         FROM kms_embeddings_text v
         JOIN kms_vector_map m ON v.rowid = m.vec_id
         WHERE m.entity_type = 'note' AND m.modality = 'text'"
    )?;

    let rows = stmt.query_map([], |row| {
        let path: String = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        Ok((path, f32_vec_from_embedding_blob(&blob)))
    })?;

    let mut results = Vec::new();
    for r in rows {
        results.push(r?);
    }
    Ok(results)
}

/// Embeddings for the given vault-relative note paths only (for local graph / scoped loads).
pub fn get_note_embeddings_for_paths(paths: &[String]) -> KmsResult<Vec<(String, Vec<f32>)>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    const CHUNK: usize = 400;
    let conn = conn_guard()?;
    let mut results = Vec::new();
    for chunk in paths.chunks(CHUNK) {
        let ph = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT m.entity_id, v.embedding 
             FROM kms_embeddings_text v
             JOIN kms_vector_map m ON v.rowid = m.vec_id
             WHERE m.entity_type = 'note' AND m.modality = 'text' AND m.entity_id IN ({ph})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(chunk.iter()), |row| {
            let path: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((path, f32_vec_from_embedding_blob(&blob)))
        })?;
        for r in rows {
            results.push(r?);
        }
    }
    Ok(results)
}
