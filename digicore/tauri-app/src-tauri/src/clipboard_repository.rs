use image::{imageops, DynamicImage, ImageFormat, RgbaImage};
use regex::RegexBuilder;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct ClipboardRow {
    pub id: u32,
    pub content: String,
    pub process_name: String,
    pub window_title: String,
    pub char_count: u32,
    pub word_count: u32,
    pub created_at_unix_ms: u64,
    pub entry_type: String,
    pub mime_type: Option<String>,
    pub image_path: Option<String>,
    pub thumb_path: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub image_bytes: Option<u32>,
    pub parent_id: Option<u32>,
}

static DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static DB_CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn normalize_content_for_hash(content: &str) -> String {
    content.replace("\r\n", "\n").trim().to_string()
}

fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalize_content_for_hash(content).as_bytes());
    format!("{:x}", hasher.finalize())
}

fn image_hash(rgba_bytes: &[u8], width: u32, height: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(width.to_le_bytes());
    hasher.update(height.to_le_bytes());
    hasher.update(rgba_bytes);
    format!("{:x}", hasher.finalize())
}

fn word_count(content: &str) -> u32 {
    content.split_whitespace().count() as u32
}

fn assets_root_dir() -> PathBuf {
    digicore_text_expander::ports::data_path_resolver::DataPathResolver::clipboard_images_dir()
}

fn ensure_column(conn: &Connection, name: &str, def: &str) -> Result<(), String> {
    let exists: Option<String> = conn
        .query_row(
            "SELECT name FROM pragma_table_info('clipboard_history') WHERE name = ?1 LIMIT 1",
            params![name],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        conn.execute(
            &format!("ALTER TABLE clipboard_history ADD COLUMN {} {}", name, def),
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn default_db_path() -> PathBuf {
    digicore_text_expander::ports::data_path_resolver::DataPathResolver::db_path()
}

pub fn init(db_path: PathBuf) -> Result<(), String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let _ = DB_PATH.set(db_path.clone());
    log::info!("[ClipboardRepository] Opening database at: {}", db_path.display());
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        CREATE TABLE IF NOT EXISTS clipboard_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            process_name TEXT NOT NULL DEFAULT '',
            window_title TEXT NOT NULL DEFAULT '',
            char_count INTEGER NOT NULL DEFAULT 0,
            word_count INTEGER NOT NULL DEFAULT 0,
            content_hash TEXT NOT NULL DEFAULT '',
            created_at_unix_ms INTEGER NOT NULL,
            entry_type TEXT NOT NULL DEFAULT 'text',
            mime_type TEXT,
            image_path TEXT,
            thumb_path TEXT,
            image_width INTEGER,
            image_height INTEGER,
            image_bytes INTEGER,
            parent_id INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_clipboard_history_created_at
            ON clipboard_history(created_at_unix_ms DESC);
        CREATE INDEX IF NOT EXISTS idx_clipboard_history_content_hash
            ON clipboard_history(content_hash);
        "#,
    )
    .map_err(|e| e.to_string())?;
    ensure_column(&conn, "entry_type", "TEXT NOT NULL DEFAULT 'text'")?;
    ensure_column(&conn, "mime_type", "TEXT")?;
    ensure_column(&conn, "image_path", "TEXT")?;
    ensure_column(&conn, "thumb_path", "TEXT")?;
    ensure_column(&conn, "image_width", "INTEGER")?;
    ensure_column(&conn, "image_height", "INTEGER")?;
    ensure_column(&conn, "image_bytes", "INTEGER")?;
    ensure_column(&conn, "parent_id", "INTEGER")?;
    let _ = DB_CONN.set(Mutex::new(conn));
    Ok(())
}

fn conn_guard() -> Result<std::sync::MutexGuard<'static, Connection>, String> {
    let conn = DB_CONN
        .get()
        .ok_or_else(|| "clipboard repository not initialized".to_string())?;
    conn.lock().map_err(|e| e.to_string())
}

fn latest_content_hash(conn: &Connection) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT content_hash FROM clipboard_history WHERE parent_id IS NULL ORDER BY id DESC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn insert_entry(content: &str, process_name: &str, window_title: &str) -> Result<bool, String> {
    let normalized = normalize_content_for_hash(content);
    if normalized.is_empty() {
        return Ok(false);
    }
    let hash = content_hash(content);
    let conn = conn_guard()?;
    if latest_content_hash(&conn)?.as_deref() == Some(hash.as_str()) {
        return Ok(false);
    }
    let chars = content.chars().count() as u32;
    let words = word_count(content);
    let now_ms = now_unix_ms() as i64;
    log::info!("[ClipboardRepository] Inserting text entry, hash: {}", hash);
    conn.execute(
        "INSERT INTO clipboard_history (
            content, process_name, window_title, char_count, word_count, content_hash, created_at_unix_ms, entry_type, parent_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'text', ?8)",
        params![content, process_name, window_title, chars, words, hash, now_ms, None::<u32>],
    )
    .map_err(|e| e.to_string())?;
    log::info!("[ClipboardRepository] Text insertion successful, row_id: {}", conn.last_insert_rowid());
    Ok(true)
}

fn save_image_assets(
    hash: &str,
    width: u32,
    height: u32,
    rgba_bytes: &[u8],
    image_storage_dir: &str,
) -> Result<(String, String), String> {
    let root = if image_storage_dir.trim().is_empty() {
        assets_root_dir()
    } else {
        PathBuf::from(image_storage_dir.trim())
    };
    let full_dir = root.join("full");
    let thumb_dir = root.join("thumbs");
    std::fs::create_dir_all(&full_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&thumb_dir).map_err(|e| e.to_string())?;

    let full_path = full_dir.join(format!("{hash}.png"));
    let thumb_path = thumb_dir.join(format!("{hash}_thumb.png"));

    if !full_path.exists() || !thumb_path.exists() {
        let img = RgbaImage::from_raw(width, height, rgba_bytes.to_vec())
            .ok_or_else(|| "Failed to construct image from clipboard bytes.".to_string())?;
        let dyn_img = DynamicImage::ImageRgba8(img);
        dyn_img
            .save_with_format(&full_path, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let thumb = imageops::thumbnail(&dyn_img, 320, 200);
        DynamicImage::ImageRgba8(thumb)
            .save_with_format(&thumb_path, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
    }

    Ok((
        full_path.to_string_lossy().to_string(),
        thumb_path.to_string_lossy().to_string(),
    ))
}

fn derived_thumb_path_from_image(image_path: &Path) -> Option<PathBuf> {
    let file_stem = image_path.file_stem()?.to_string_lossy().to_string();
    let root = image_path.parent()?.parent()?;
    Some(root.join("thumbs").join(format!("{file_stem}_thumb.png")))
}

fn ensure_row_thumbnail(row: &mut ClipboardRow) {
    if row.entry_type != "image" {
        return;
    }
    let Some(image_path_str) = row.image_path.clone() else {
        return;
    };
    let image_path = PathBuf::from(&image_path_str);
    if !image_path.exists() {
        return;
    }
    let thumb_target = row
        .thumb_path
        .clone()
        .map(PathBuf::from)
        .or_else(|| derived_thumb_path_from_image(&image_path));
    let Some(thumb_path) = thumb_target else {
        return;
    };
    if !thumb_path.exists() {
        if let Some(parent) = thumb_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(img) = image::open(&image_path) {
            let thumb = imageops::thumbnail(&img, 320, 200);
            if DynamicImage::ImageRgba8(thumb)
                .save_with_format(&thumb_path, ImageFormat::Png)
                .is_ok()
            {
                row.thumb_path = Some(thumb_path.to_string_lossy().to_string());
                return;
            }
        }
    } else {
        row.thumb_path = Some(thumb_path.to_string_lossy().to_string());
    }
}


pub fn insert_image_entry_returning_id(
    rgba_bytes: &[u8],
    width: u32,
    height: u32,
    process_name: &str,
    window_title: &str,
    mime_type: Option<&str>,
    image_storage_dir: &str,
) -> Result<u32, String> {
    if rgba_bytes.is_empty() || width == 0 || height == 0 {
        return Ok(0);
    }
    let hash = image_hash(rgba_bytes, width, height);
    let conn = conn_guard()?;
    if latest_content_hash(&conn)?.as_deref() == Some(hash.as_str()) {
        return Ok(0);
    }
    let (image_path, thumb_path) =
        save_image_assets(&hash, width, height, rgba_bytes, image_storage_dir)?;
    let content = format!("[Image] {}x{} {}", width, height, mime_type.unwrap_or("image/png"));
    let now_ms = now_unix_ms() as i64;
    log::info!("[ClipboardRepository] Inserting image entry (id-return), hash: {}", hash);
    conn.execute(
        "INSERT INTO clipboard_history (
            content, process_name, window_title, char_count, word_count, content_hash, created_at_unix_ms, entry_type,
            mime_type, image_path, thumb_path, image_width, image_height, image_bytes, parent_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'image', ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            content,
            process_name,
            window_title,
            0i64,
            0i64,
            hash,
            now_ms,
            mime_type.unwrap_or("image/png"),
            image_path,
            thumb_path,
            width as i64,
            height as i64,
            rgba_bytes.len() as i64,
            None::<u32>
        ],
    )
    .map_err(|e| e.to_string())?;
    
    Ok(conn.last_insert_rowid() as u32)
}

pub fn migrate_image_assets_root(old_root: &str, new_root: &str) -> Result<u32, String> {
    let old_root_trimmed = old_root.trim();
    let new_root_trimmed = new_root.trim();
    if old_root_trimmed.is_empty() || new_root_trimmed.is_empty() {
        return Ok(0);
    }
    let old_root_path = PathBuf::from(old_root_trimmed);
    let new_root_path = PathBuf::from(new_root_trimmed);
    if old_root_path == new_root_path {
        return Ok(0);
    }
    let new_full = new_root_path.join("full");
    let new_thumbs = new_root_path.join("thumbs");
    std::fs::create_dir_all(&new_full).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&new_thumbs).map_err(|e| e.to_string())?;

    let conn = conn_guard()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, image_path, thumb_path
             FROM clipboard_history
             WHERE entry_type = 'image'
               AND image_path IS NOT NULL
               AND thumb_path IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)? as u32,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut updates = Vec::<(u32, String, String, String, String)>::new();
    for item in rows {
        let (id, image_path, thumb_path) = item.map_err(|e| e.to_string())?;
        let image_src = PathBuf::from(&image_path);
        let thumb_src = PathBuf::from(&thumb_path);
        let image_name = image_src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| format!("Invalid image path for row id={id}"))?;
        let thumb_name = thumb_src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| format!("Invalid thumbnail path for row id={id}"))?;
        let image_dest = new_full.join(image_name);
        let thumb_dest = new_thumbs.join(thumb_name);
        if image_src.exists() {
            std::fs::copy(&image_src, &image_dest).map_err(|e| e.to_string())?;
        }
        if thumb_src.exists() {
            std::fs::copy(&thumb_src, &thumb_dest).map_err(|e| e.to_string())?;
        }
        updates.push((
            id,
            image_dest.to_string_lossy().to_string(),
            thumb_dest.to_string_lossy().to_string(),
            image_path,
            thumb_path,
        ));
    }

    let mut migrated = 0u32;
    for (id, image_dest, thumb_dest, image_old, thumb_old) in updates {
        conn.execute(
            "UPDATE clipboard_history
             SET image_path = ?1, thumb_path = ?2
             WHERE id = ?3",
            params![image_dest, thumb_dest, id],
        )
        .map_err(|e| e.to_string())?;
        migrated += 1;
        let _ = std::fs::remove_file(image_old);
        let _ = std::fs::remove_file(thumb_old);
    }
    Ok(migrated)
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardRow> {
    Ok(ClipboardRow {
        id: row.get::<_, i64>(0)? as u32,
        content: row.get(1)?,
        process_name: row.get(2)?,
        window_title: row.get(3)?,
        char_count: row.get::<_, i64>(4)? as u32,
        word_count: row.get::<_, i64>(5)? as u32,
        created_at_unix_ms: row.get::<_, i64>(6)? as u64,
        entry_type: row.get::<_, String>(7)?,
        mime_type: row.get(8)?,
        image_path: row.get(9)?,
        thumb_path: row.get(10)?,
        image_width: row.get::<_, Option<i64>>(11)?.map(|v| v as u32),
        image_height: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
        image_bytes: row.get::<_, Option<i64>>(13)?.map(|v| v as u32),
        parent_id: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
    })
}

pub fn list_entries(search: Option<&str>, limit: u32) -> Result<Vec<ClipboardRow>, String> {
    let conn = conn_guard()?;
    let cap = limit.clamp(1, 10_000);
    let mut rows = Vec::new();
    let select_sql = "SELECT
            id, content, process_name, window_title, char_count, word_count, created_at_unix_ms,
            entry_type, mime_type, image_path, thumb_path, image_width, image_height, image_bytes, parent_id
        FROM clipboard_history";
    if let Some(search_raw) = search {
        let search_trim = search_raw.trim();
        if !search_trim.is_empty() {
            let like = format!("%{}%", search_trim);
            let mut stmt = conn
                .prepare(&format!(
                    "{select_sql}
                     WHERE content LIKE ?1 OR process_name LIKE ?1 OR window_title LIKE ?1 OR mime_type LIKE ?1
                     ORDER BY id DESC
                     LIMIT ?2"
                ))
                .map_err(|e| e.to_string())?;
            let mapped = stmt
                .query_map(params![like, cap], map_row)
                .map_err(|e| e.to_string())?;
            for item in mapped {
                let mut row = item.map_err(|e| e.to_string())?;
                ensure_row_thumbnail(&mut row);
                rows.push(row);
            }
            return Ok(rows);
        }
    }
    let mut stmt = conn
        .prepare(&format!(
            "{select_sql}
             ORDER BY id DESC
             LIMIT ?1"
        ))
        .map_err(|e| e.to_string())?;
    let mapped = stmt
        .query_map(params![cap], map_row)
        .map_err(|e| e.to_string())?;
    for item in mapped {
        let mut row = item.map_err(|e| e.to_string())?;
        ensure_row_thumbnail(&mut row);
        rows.push(row);
    }
    Ok(rows)
}

fn row_blob(row: &ClipboardRow) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        row.content.to_ascii_lowercase(),
        row.process_name.to_ascii_lowercase(),
        row.window_title.to_ascii_lowercase(),
        row.entry_type.to_ascii_lowercase(),
        row.mime_type
            .clone()
            .unwrap_or_default()
            .to_ascii_lowercase()
    )
}

pub fn search_entries(
    search: &str,
    operator: Option<&str>,
    limit: u32,
) -> Result<Vec<ClipboardRow>, String> {
    let cap = limit.clamp(1, 10_000);
    let query = search.trim();
    if query.is_empty() {
        return list_entries(None, cap);
    }
    let operator_normalized = operator.unwrap_or("or").trim().to_ascii_lowercase();
    let candidates = list_entries(None, cap.saturating_mul(5).min(10_000))?;
    if operator_normalized == "regex" {
        let mut pattern = query.to_string();
        if !pattern.contains('|') {
            pattern = regex::Regex::new("(?i)\\s+OR\\s+")
                .map_err(|e| e.to_string())?
                .replace_all(&pattern, "|")
                .into_owned();
        }
        if pattern.contains('*') {
            pattern = pattern.replace('*', ".*");
        }
        let regex = RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .build()
            .map_err(|e| format!("Invalid regex pattern: {e}"))?;
        let mut rows = Vec::new();
        for row in candidates {
            let haystack = format!(
                "{}\n{}\n{}",
                row.content.as_str(),
                row.process_name.as_str(),
                row.window_title.as_str()
            );
            if regex.is_match(&haystack) {
                rows.push(row);
                if rows.len() >= cap as usize {
                    break;
                }
            }
        }
        return Ok(rows);
    }

    let terms: Vec<String> = query
        .split_whitespace()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let use_and = operator_normalized == "and";
    let mut rows = Vec::new();
    for row in candidates {
        let blob = row_blob(&row);
        let matched = if use_and {
            terms.iter().all(|t| blob.contains(t))
        } else {
            terms.iter().any(|t| blob.contains(t))
        };
        if matched {
            rows.push(row);
            if rows.len() >= cap as usize {
                break;
            }
        }
    }
    Ok(rows)
}

pub fn get_entry_by_id(id: u32) -> Result<Option<ClipboardRow>, String> {
    let conn = conn_guard()?;
    let mut stmt = conn
        .prepare(
            "SELECT
                id, content, process_name, window_title, char_count, word_count, created_at_unix_ms,
                entry_type, mime_type, image_path, thumb_path, image_width, image_height, image_bytes, parent_id
             FROM clipboard_history
             WHERE id = ?1
             LIMIT 1",
        )
        .map_err(|e| e.to_string())?;
    stmt.query_row(params![id], map_row)
        .optional()
        .map_err(|e| e.to_string())
}

pub fn delete_entry_by_id(id: u32) -> Result<(), String> {
    let row = get_entry_by_id(id)?;
    let conn = conn_guard()?;
    conn.execute("DELETE FROM clipboard_history WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    if let Some(r) = row {
        if let Some(path) = r.image_path {
            let _ = std::fs::remove_file(path);
        }
        if let Some(path) = r.thumb_path {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

pub fn clear_all() -> Result<(), String> {
    let all = list_entries(None, 100_000)?;
    for row in all {
        if let Some(path) = row.image_path {
            let _ = std::fs::remove_file(path);
        }
        if let Some(path) = row.thumb_path {
            let _ = std::fs::remove_file(path);
        }
    }
    let conn = conn_guard()?;
    conn.execute("DELETE FROM clipboard_history", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn trim_to_depth(max_depth: u32) -> Result<u32, String> {
    if max_depth == 0 {
        return Ok(0);
    }
    let conn = conn_guard()?;
    let depth = max_depth as i64;
    let mut stale_assets = Vec::<(Option<String>, Option<String>)>::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT image_path, thumb_path
                 FROM clipboard_history
                 WHERE id NOT IN (
                     SELECT id FROM clipboard_history ORDER BY id DESC LIMIT ?1
                 )",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![depth], |row| {
                Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .map_err(|e| e.to_string())?;
        for item in rows {
            stale_assets.push(item.map_err(|e| e.to_string())?);
        }
    }
    let affected = conn
        .execute(
            "DELETE FROM clipboard_history
             WHERE id NOT IN (
                 SELECT id FROM clipboard_history ORDER BY id DESC LIMIT ?1
             )",
            params![depth],
        )
        .map_err(|e| e.to_string())?;
    for (image_path, thumb_path) in stale_assets {
        if let Some(path) = image_path {
            let _ = std::fs::remove_file(path);
        }
        if let Some(path) = thumb_path {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(affected as u32)
}

pub fn count() -> Result<u32, String> {
    let conn = conn_guard()?;
    let total = conn
        .query_row("SELECT COUNT(1) FROM clipboard_history", [], |row| row.get::<_, i64>(0))
        .map_err(|e| e.to_string())?;
    Ok(total.max(0) as u32)
}

pub fn insert_extracted_text_entry(
    content: &str,
    process_name: &str,
    window_title: &str,
    parent_id: u32,
    _metadata: &serde_json::Value,
) -> Result<u32, String> {
    let now_ms = now_unix_ms() as i64;
    let conn = conn_guard()?;
    
    log::info!("[ClipboardRepository] Inserting extracted text entry for parent_id: {}", parent_id);
    
    conn.execute(
        "INSERT INTO clipboard_history (
            content, process_name, window_title, char_count, word_count, created_at_unix_ms, entry_type, parent_id
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'extracted_text', ?7)",
        params![
            content,
            process_name,
            window_title,
            content.chars().count() as i64,
            word_count(content) as i64,
            now_ms,
            parent_id
        ],
    )
    .map_err(|e| e.to_string())?;
    
    Ok(conn.last_insert_rowid() as u32)
}
