//! TransformerService - handles global clipboard transformations.
//!
//! Example: Ctrl+Shift+V -> Paste as Plain Text.

use std::sync::Arc;
use digicore_core::domain::ports::{ClipboardPort, InputPort};

pub struct TransformerService {
    clipboard: Arc<dyn ClipboardPort>,
    input: Arc<dyn InputPort>,
}

impl TransformerService {
    pub fn new(clipboard: Arc<dyn ClipboardPort>, input: Arc<dyn InputPort>) -> Self {
        Self { clipboard, input }
    }

    /// Paste current clipboard content as plain text.
    pub fn paste_plain_text(&self) -> anyhow::Result<()> {
        if let Ok(text) = self.clipboard.get_text() {
            // Setting text only (without HTML/RTF) effectively strips formatting for the next paste.
            self.clipboard.set_text(&text)?;
            self.input.send_ctrl_v()?;
        }
        Ok(())
    }
}
