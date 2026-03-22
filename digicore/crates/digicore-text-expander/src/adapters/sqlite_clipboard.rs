//! SqliteClipboardRepository - Persistence adapter for clipboard history.

use digicore_core::domain::ports::ClipboardRepository;
use digicore_core::domain::entities::clipboard_entry::ClipEntry;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;
use chrono::{DateTime, Local};

pub struct SqliteClipboardRepository {
    conn: Mutex<Connection>,
}

impl SqliteClipboardRepository {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path).context("Failed to open clipboard database")?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS clipboard_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                html_content TEXT,
                rtf_content TEXT,
                process_name TEXT NOT NULL,
                window_title TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                entry_type TEXT NOT NULL DEFAULT 'text',
                mime_type TEXT,
                image_path TEXT,
                thumb_path TEXT,
                image_width INTEGER,
                image_height INTEGER,
                image_bytes INTEGER,
                parent_id INTEGER,
                metadata TEXT,
                file_list TEXT,
                length INTEGER NOT NULL DEFAULT 0,
                word_count INTEGER NOT NULL DEFAULT 0
            )",
            [],
        ).context("Failed to create clipboard_history table")?;

        // Migration: Add new columns if they don't exist (Basic attempt)
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN entry_type TEXT NOT NULL DEFAULT 'text'", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN mime_type TEXT", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN image_path TEXT", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN thumb_path TEXT", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN image_width INTEGER", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN image_height INTEGER", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN image_bytes INTEGER", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN parent_id INTEGER", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN metadata TEXT", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN file_list TEXT", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN length INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE clipboard_history ADD COLUMN word_count INTEGER NOT NULL DEFAULT 0", []);

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_clipboard_timestamp ON clipboard_history(timestamp)",
            [],
        ).context("Failed to create index on timestamp")?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl ClipboardRepository for SqliteClipboardRepository {
    fn save(&self, entry: &ClipEntry) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO clipboard_history (
                content, html_content, rtf_content, process_name, window_title, timestamp,
                entry_type, mime_type, image_path, thumb_path, image_width, image_height,
                image_bytes, parent_id, metadata, file_list, length, word_count
            )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                entry.content,
                entry.html_content,
                entry.rtf_content,
                entry.process_name,
                entry.window_title,
                entry.timestamp.to_rfc3339(),
                entry.entry_type,
                entry.mime_type,
                entry.image_path,
                entry.thumb_path,
                entry.image_width,
                entry.image_height,
                entry.image_bytes,
                entry.parent_id,
                entry.metadata.clone(),
                entry.file_list.as_ref().and_then(|fl| serde_json::to_string(fl).ok()),
                entry.length as i64,
                entry.word_count as i64,
            ],
        ).context("Failed to save clipboard entry")?;
        Ok(())
    }

    fn load_last_n(&self, n: usize) -> Result<Vec<ClipEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT 
                content, html_content, rtf_content, process_name, window_title, timestamp,
                entry_type, mime_type, image_path, thumb_path, image_width, image_height,
                image_bytes, parent_id, metadata, file_list, length, word_count, id
             FROM clipboard_history
             ORDER BY timestamp DESC
             LIMIT ?1",
        ).context("Failed to prepare load statement")?;

        let entries = stmt.query_map(params![n as i64], |row| {
            let timestamp_str: String = row.get(5)?;
            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or_else(|_| Local::now());

            Ok(ClipEntry {
                content: row.get(0)?,
                html_content: row.get(1)?,
                rtf_content: row.get(2)?,
                process_name: row.get(3)?,
                window_title: row.get(4)?,
                timestamp,
                entry_type: row.get(6)?,
                mime_type: row.get(7)?,
                image_path: row.get(8)?,
                thumb_path: row.get(9)?,
                image_width: row.get(10)?,
                image_height: row.get(11)?,
                image_bytes: row.get(12)?,
                parent_id: row.get(13)?,
                metadata: row.get(14)?,
                file_list: row.get::<_, Option<String>>(15)?.and_then(|s| serde_json::from_str(&s).ok()),
                length: row.get::<_, i64>(16)? as usize,
                word_count: row.get::<_, i64>(17)? as usize,
                id: row.get(18)?,
            })
        }).context("Failed to query clipboard history")?;

        let mut result = Vec::new();
        for entry in entries {
            result.push(entry?);
        }
        Ok(result)
    }

    fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_history", []).context("Failed to clear clipboard history")?;
        Ok(())
    }

    fn delete_at(&self, timestamp: DateTime<Local>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM clipboard_history WHERE timestamp = ?1",
            params![timestamp.to_rfc3339()],
        ).context("Failed to delete clipboard entry")?;
        Ok(())
    }
}
