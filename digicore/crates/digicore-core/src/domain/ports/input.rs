//! IInputPort - inject text and key events (SendText equivalent).

use anyhow::Result;

/// Port for platform input injection (typing text, key events).
///
/// Implementations: EnigoInputAdapter (Windows/macOS/Linux), MockInputAdapter (tests).
pub trait InputPort: Send + Sync {
    /// Type text as if user typed it (SendText / RobustPaste equivalent).
    fn type_text(&self, text: &str) -> Result<()>;

    /// Send a key press (for hotkeys, Tab, etc.).
    fn key_sequence(&self, keys: &[Key]) -> Result<()>;
}

/// Key representation for cross-platform key events.
#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Char(char),
    Tab,
    Enter,
    Escape,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}
