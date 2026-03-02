//! IClipboardPort - read, write, and optionally monitor clipboard.

use anyhow::Result;

/// Port for clipboard access.
///
/// Implementations: ArboardClipboardAdapter, MockClipboardAdapter (tests).
pub trait ClipboardPort: Send + Sync {
    /// Read current clipboard text.
    fn get_text(&self) -> Result<String>;

    /// Write text to clipboard.
    fn set_text(&self, text: &str) -> Result<()>;

    /// Check if clipboard contains text.
    fn has_text(&self) -> Result<bool> {
        Ok(!self.get_text()?.is_empty())
    }
}
