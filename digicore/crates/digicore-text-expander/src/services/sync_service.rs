//! Sync service - orchestrates WebDAV push/pull with merge.
//!
//! F33-F37: WebDAV sync, merge-by-trigger, startup sync.
//! F28: Timestamped backup before sync pull.

use digicore_core::adapters::persistence::JsonLibraryAdapter;
use digicore_core::adapters::sync::webdav::WebDAVAdapter;
use digicore_core::domain::ports::{SnippetRepository, SyncPort};
use digicore_core::domain::Snippet;
use std::collections::HashMap;
use std::path::Path;
/// Result of a sync operation.
#[derive(Debug, Clone)]
pub enum SyncResult {
    Idle,
    InProgress,
    Success(String),
    Error(String),
}

/// Perform timestamped backup before sync pull (F28).
fn backup_before_pull(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let stamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let ext = path
        .extension()
        .map(|e| format!("{}.", e.to_string_lossy()))
        .unwrap_or_default();
    let backup_path = path.with_extension(format!("{}backup_{}", ext, stamp));
    std::fs::copy(path, &backup_path)
        .map_err(|e| anyhow::anyhow!("Backup failed: {}", e))?;
    Ok(())
}

/// Push library to WebDAV.
pub fn push_sync(
    path: &Path,
    url: &str,
    password: &str,
) -> anyhow::Result<()> {
    let repo = JsonLibraryAdapter;
    let library = repo.load(path)?;
    let lib_file = serde_json::json!({ "Categories": library });
    let json = serde_json::to_string(&lib_file)?;

    let sync = WebDAVAdapter::new()?;
    sync.push(json.as_bytes(), url, password)?;
    Ok(())
}

/// Pull from WebDAV, merge (F36), save. Creates backup before pull (F28).
pub fn pull_sync(
    path: &Path,
    url: &str,
    password: &str,
) -> anyhow::Result<HashMap<String, Vec<Snippet>>> {
    backup_before_pull(path)?;

    let sync = WebDAVAdapter::new()?;
    let bytes = sync.pull(url, password)?;
    let json = String::from_utf8(bytes)?;
    #[derive(serde::Deserialize)]
    struct LibraryFile {
        #[serde(rename = "Categories")]
        categories: HashMap<String, Vec<Snippet>>,
    }
    let lib: LibraryFile =
        serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("Parse pulled JSON: {}", e))?;
    let incoming = lib.categories;

    let repo = JsonLibraryAdapter;
    let mut existing = if path.exists() {
        repo.load(path).unwrap_or_default()
    } else {
        HashMap::new()
    };

    repo.merge(&mut existing, incoming);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    repo.save(path, &existing)?;
    Ok(existing)
}
