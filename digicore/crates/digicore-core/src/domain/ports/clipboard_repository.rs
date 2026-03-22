//! ClipboardRepository Port - Persistence for clipboard history.

use crate::domain::entities::clipboard_entry::ClipEntry;
use anyhow::Result;

/// Port for clipboard persistence (SQLite).
pub trait ClipboardRepository: Send + Sync {
    /// Save a new entry to the database.
    fn save(&self, entry: &ClipEntry) -> Result<()>;

    /// Load the last N entries from the database, most recent first.
    fn load_last_n(&self, n: usize) -> Result<Vec<ClipEntry>>;

    /// Clear all entries from the history.
    fn clear_all(&self) -> Result<()>;

    /// Delete a single entry (optional, but good for completeness).
    fn delete_at(&self, timestamp: chrono::DateTime<chrono::Local>) -> Result<()>;
}
