//! Mock adapters for testing (unit, integration, edge, negative).

use crate::domain::ports::{ClipboardPort, InputPort, Key, WindowContext, WindowContextPort};
use anyhow::Result;
use std::sync::Mutex;

/// Mock input adapter - records typed text for assertions.
#[derive(Debug, Default)]
pub struct MockInputAdapter {
    pub typed: Mutex<Vec<String>>,
    pub keys_pressed: Mutex<Vec<Key>>,
}

impl MockInputAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn typed_text(&self) -> Vec<String> {
        self.typed.lock().unwrap().clone()
    }
}

impl InputPort for MockInputAdapter {
    fn type_text(&self, text: &str) -> Result<()> {
        self.typed.lock().unwrap().push(text.to_string());
        Ok(())
    }

    fn key_sequence(&self, keys: &[Key]) -> Result<()> {
        self.keys_pressed.lock().unwrap().extend(keys.to_vec());
        Ok(())
    }

    fn send_ctrl_v(&self) -> Result<()> {
        // We could record this as a special key or just return Ok
        Ok(())
    }
}

/// Mock clipboard adapter - in-memory store for tests.
#[derive(Debug, Default)]
pub struct MockClipboardAdapter {
    content: Mutex<String>,
    html: Mutex<Option<String>>,
    rtf: Mutex<Option<String>>,
}

impl MockClipboardAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_content(s: &str) -> Self {
        Self {
            content: Mutex::new(s.to_string()),
            html: Mutex::new(None),
            rtf: Mutex::new(None),
        }
    }
}

impl ClipboardPort for MockClipboardAdapter {
    fn get_text(&self) -> Result<String> {
        Ok(self.content.lock().unwrap().clone())
    }

    fn set_text(&self, text: &str) -> Result<()> {
        *self.content.lock().unwrap() = text.to_string();
        Ok(())
    }

    fn set_multi(&self, plain: &str, html: Option<&str>, rtf: Option<&str>) -> Result<()> {
        *self.content.lock().unwrap() = plain.to_string();
        *self.html.lock().unwrap() = html.map(|s| s.to_string());
        *self.rtf.lock().unwrap() = rtf.map(|s| s.to_string());
        Ok(())
    }

    fn get_rich_text(&self) -> Result<(String, Option<String>, Option<String>)> {
        Ok((
            self.content.lock().unwrap().clone(),
            self.html.lock().unwrap().clone(),
            self.rtf.lock().unwrap().clone(),
        ))
    }
}

/// Mock window context adapter - returns configurable values.
#[derive(Debug, Default)]
pub struct MockWindowAdapter {
    pub context: Mutex<WindowContext>,
}

impl MockWindowAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_context(process: &str, title: &str) -> Self {
        Self {
            context: Mutex::new(WindowContext {
                process_name: process.to_string(),
                title: title.to_string(),
            }),
        }
    }

    pub fn set_context(&self, process: &str, title: &str) {
        *self.context.lock().unwrap() = WindowContext {
            process_name: process.to_string(),
            title: title.to_string(),
        };
    }
}

impl WindowContextPort for MockWindowAdapter {
    fn get_active(&self) -> Result<WindowContext> {
        Ok(self.context.lock().unwrap().clone())
    }
}
