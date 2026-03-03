//! FileDialogPort - framework-agnostic file picker.
//!
//! Part of Phase 2 UI decoupling. Abstracts Load/Save/Browse dialogs.
//!
//! Implementations: RfdFileDialogAdapter (rfd), TauriFileDialogAdapter (future).

use std::path::PathBuf;

/// Port for file picker dialogs (open, save).
///
/// Filters: `(display_name, extensions)` e.g. `("JSON", ["json"])`.
/// Empty filters or `("*", ["*"])` = All Files.
pub trait FileDialogPort: Send + Sync {
    /// Pick a file to open. Returns None if cancelled.
    fn pick_file(&self, filters: &[(&str, &[&str])]) -> Option<PathBuf>;

    /// Pick a path to save. Returns None if cancelled.
    fn save_file(&self, filters: &[(&str, &[&str])], default_name: &str) -> Option<PathBuf>;
}
