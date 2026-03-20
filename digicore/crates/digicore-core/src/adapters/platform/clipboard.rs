//! ArboardClipboardAdapter - implements ClipboardPort using arboard.

use crate::domain::ports::ClipboardPort;
use anyhow::Result;
use arboard::Clipboard;
use std::sync::Mutex;

/// Clipboard adapter via arboard (cross-platform).
pub struct ArboardClipboardAdapter {
    clipboard: Mutex<Clipboard>,
}

impl ArboardClipboardAdapter {
    pub fn new() -> Result<Self> {
        let clipboard = Clipboard::new().map_err(|e| anyhow::anyhow!("Clipboard init: {}", e))?;
        Ok(Self {
            clipboard: Mutex::new(clipboard),
        })
    }
}

impl Default for ArboardClipboardAdapter {
    fn default() -> Self {
        Self::new().expect("ArboardClipboardAdapter init")
    }
}

impl ClipboardPort for ArboardClipboardAdapter {
    fn get_text(&self) -> Result<String> {
        self.clipboard
            .lock()
            .unwrap()
            .get_text()
            .map_err(|e| anyhow::anyhow!("Clipboard get: {}", e))
    }

    fn set_text(&self, text: &str) -> Result<()> {
        self.clipboard
            .lock()
            .unwrap()
            .set_text(text)
            .map_err(|e| anyhow::anyhow!("Clipboard set: {}", e))
    }

    fn set_multi(&self, plain: &str, _html: Option<&str>, _rtf: Option<&str>) -> Result<()> {
        // Arboard version 3.3 has limited multi-format support.
        // For now, this fallback only sets the plain text.
        // The specialized WindowsRichClipboardAdapter will handle HTML/RTF.
        self.set_text(plain)
    }

    fn get_rich_text(&self) -> Result<(String, Option<String>, Option<String>)> {
        Ok((self.get_text()?, None, None))
    }
}
