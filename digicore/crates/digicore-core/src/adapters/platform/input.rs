//! EnigoInputAdapter - implements InputPort using enigo (Windows).

use crate::domain::ports::{InputPort, Key};
use anyhow::Result;
use enigo::{Direction, Enigo, Key as EnigoKey, Keyboard, Settings};
use std::sync::Mutex;

/// Windows input adapter via enigo.
pub struct EnigoInputAdapter {
    enigo: Mutex<Enigo>,
}

impl EnigoInputAdapter {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("Enigo init: {}", e))?;
        Ok(Self {
            enigo: Mutex::new(enigo),
        })
    }
}

impl Default for EnigoInputAdapter {
    fn default() -> Self {
        Self::new().expect("EnigoInputAdapter init")
    }
}

impl EnigoInputAdapter {
    /// Send Ctrl+V (paste). More reliable than type_text for editors like Sublime.
    pub fn send_ctrl_v(&self) -> Result<()> {
        use enigo::Key as EnigoKey;
        let mut enigo = self.enigo.lock().unwrap();
        enigo.key(EnigoKey::Control, Direction::Press).map_err(|e| anyhow::anyhow!("Enigo ctrl: {}", e))?;
        enigo.key(EnigoKey::Unicode('v'), Direction::Press).map_err(|e| anyhow::anyhow!("Enigo v: {}", e))?;
        enigo.key(EnigoKey::Unicode('v'), Direction::Release).map_err(|e| anyhow::anyhow!("Enigo v: {}", e))?;
        enigo.key(EnigoKey::Control, Direction::Release).map_err(|e| anyhow::anyhow!("Enigo ctrl: {}", e))?;
        Ok(())
    }
}

impl InputPort for EnigoInputAdapter {
    fn type_text(&self, text: &str) -> Result<()> {
        self.enigo
            .lock()
            .unwrap()
            .text(text)
            .map_err(|e| anyhow::anyhow!("Enigo text: {}", e))
    }

    fn key_sequence(&self, keys: &[Key]) -> Result<()> {
        let mut enigo = self.enigo.lock().unwrap();
        for key in keys {
            match key {
                Key::Char(c) => enigo.text(&c.to_string()).map_err(|e| anyhow::anyhow!("Enigo text: {}", e))?,
                Key::Tab => enigo.key(EnigoKey::Tab, Direction::Press).map_err(|e| anyhow::anyhow!("Enigo key: {}", e))?,
                Key::Enter => enigo.key(EnigoKey::Return, Direction::Press).map_err(|e| anyhow::anyhow!("Enigo key: {}", e))?,
                Key::Escape => enigo.key(EnigoKey::Escape, Direction::Press).map_err(|e| anyhow::anyhow!("Enigo key: {}", e))?,
                Key::Backspace => enigo.key(EnigoKey::Backspace, Direction::Press).map_err(|e| anyhow::anyhow!("Enigo key: {}", e))?,
                _ => {}
            }
        }
        Ok(())
    }
}
