//! ISyncPort - WebDAV push/pull for encrypted library sync.
//!
//! F33: WebDAV push/pull
//! F35: Retry on failure (3x, 2s delay)
//! F36: Merge-by-trigger on pull (handled by SnippetRepository.merge)
//! F37: Startup sync (orchestration layer)

use anyhow::Result;

/// Port for WebDAV sync (push encrypted, pull and decrypt).
///
/// Implementations: WebDAVAdapter.
pub trait SyncPort: Send + Sync {
    /// Push encrypted library to WebDAV URL.
    /// Encrypts with password before upload.
    fn push(&self, library_json: &[u8], url: &str, password: &str) -> Result<()>;

    /// Pull from WebDAV URL, decrypt, return library JSON bytes.
    fn pull(&self, url: &str, password: &str) -> Result<Vec<u8>>;
}
