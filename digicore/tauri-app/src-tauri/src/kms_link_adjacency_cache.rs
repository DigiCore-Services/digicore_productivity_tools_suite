//! In-memory cache of all wiki link rows for graph build and shortest-path BFS.
//! Invalidated when links change: `kms_sync_orchestration::sync_note_index_internal`, vault sync cleanup, note/folder delete
//! or rename/move via `KmsService`, folder delete in API, `kms_repair_database`, etc.

use std::sync::{Mutex, OnceLock};

use crate::kms_error::KmsError;
use crate::kms_repository;

static LINK_ROWS_CACHE: OnceLock<Mutex<Option<Vec<(String, String)>>>> = OnceLock::new();

fn cell() -> &'static Mutex<Option<Vec<(String, String)>>> {
    LINK_ROWS_CACHE.get_or_init(|| Mutex::new(None))
}

/// Drop cached link rows so the next read reloads from SQLite.
pub fn invalidate_kms_link_adjacency_cache() {
    if let Ok(mut g) = cell().lock() {
        *g = None;
    }
    if let Err(e) = kms_repository::clear_wiki_pagerank_fingerprint() {
        log::warn!(
            "[KMS][Graph] failed to clear materialized wiki PageRank fingerprint: {}",
            e
        );
    }
    log::debug!("[KMS][Graph] link row cache invalidated");
}

/// All `(source_path, target_path)` wiki rows, using the process-wide cache when warm.
pub fn get_all_links_cached() -> crate::kms_error::KmsResult<Vec<(String, String)>> {
    let mut guard = cell()
        .lock()
        .map_err(|_| KmsError::General("link cache lock poisoned".into()))?;
    if let Some(ref rows) = *guard {
        log::debug!(
            "[KMS][Graph] link cache hit ({} rows)",
            rows.len()
        );
        return Ok(rows.clone());
    }
    let rows = kms_repository::get_all_links()?;
    log::info!(
        "[KMS][Graph] link cache miss; loaded {} rows from DB",
        rows.len()
    );
    *guard = Some(rows.clone());
    Ok(rows)
}
