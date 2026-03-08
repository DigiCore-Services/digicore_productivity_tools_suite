use serde::{Deserialize, Serialize};

/// Configuration for the Corpus Generation Utility sub-application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusConfig {
    /// Whether the corpus generation hotkey is enabled
    pub enabled: bool,
    /// Hotkey modifiers (e.g., bitmask for Ctrl | Alt | Shift)
    pub shortcut_modifiers: u16,
    /// Hotkey virtual key code (e.g., 'S')
    pub shortcut_key: u16,
    /// Directory to save raw captured images
    pub output_dir: String,
    /// Directory to save `insta` baseline `.snap` files
    pub snapshot_dir: String,
}

impl Default for CorpusConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            // 1 (Ctrl) | 2 (Alt) | 4 (Shift)
            shortcut_modifiers: 1 | 2 | 4,
            shortcut_key: 0x43, // 'C'
            output_dir: "docs/sample-ocr-images".to_string(),
            snapshot_dir: "crates/digicore-text-expander/tests/snapshots".to_string(),
        }
    }
}
