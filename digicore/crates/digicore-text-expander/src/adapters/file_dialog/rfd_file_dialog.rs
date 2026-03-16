//! RfdFileDialogAdapter - FileDialogPort using rfd.
//!
//! Framework-agnostic; works with egui, Tauri, etc.

use crate::ports::FileDialogPort;
use std::path::PathBuf;

/// Adapter that uses rfd for native file dialogs.
#[derive(Default)]
pub struct RfdFileDialogAdapter;

impl RfdFileDialogAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl FileDialogPort for RfdFileDialogAdapter {
    fn pick_file(&self, filters: &[(&str, &[&str])]) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new();
        for (name, exts) in filters {
            if !exts.is_empty() && exts[0] != "*" {
                dialog = dialog.add_filter(*name, exts);
            }
        }
        dialog.pick_file()
    }

    fn save_file(&self, filters: &[(&str, &[&str])], default_name: &str) -> Option<PathBuf> {
        let mut dialog = rfd::FileDialog::new().set_file_name(default_name);
        for (name, exts) in filters {
            if !exts.is_empty() && exts[0] != "*" {
                dialog = dialog.add_filter(*name, exts);
            }
        }
        dialog.save_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfd_adapter_constructs() {
        let _ = RfdFileDialogAdapter::new();
    }

    #[test]
    fn test_pick_file_empty_filters_returns_none_without_user() {
        // Cannot test actual dialog; just ensure it doesn't panic.
        let adapter = RfdFileDialogAdapter::new();
        let _ = adapter.pick_file(&[]);
    }

    #[test]
    fn test_save_file_empty_filters_returns_none_without_user() {
        let adapter = RfdFileDialogAdapter::new();
        let _ = adapter.save_file(&[], "test.json");
    }
}
