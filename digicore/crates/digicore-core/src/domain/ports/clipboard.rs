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

    /// Write multiple formats to clipboard simultaneously (e.g. Plain + HTML + RTF).
    /// This ensures that the expansion preserves formatting in apps that support it.
    fn set_multi(&self, plain: &str, html: Option<&str>, rtf: Option<&str>) -> Result<()>;

    /// Read multiple formats from clipboard simultaneously (Plain, HTML, RTF).
    fn get_rich_text(&self) -> Result<(String, Option<String>, Option<String>)>;

    /// Check if clipboard contains text.
    fn has_text(&self) -> Result<bool> {
        Ok(!self.get_text()?.is_empty())
    }
}
