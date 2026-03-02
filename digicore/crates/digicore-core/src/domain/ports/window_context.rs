//! IWindowContextPort - active window app and title (WinGetProcessName, WinGetTitle equivalent).

use anyhow::Result;

/// Active window context (app + title).
#[derive(Debug, Clone, Default)]
pub struct WindowContext {
    pub process_name: String,
    pub title: String,
}

/// Port for querying active/focused window.
///
/// Implementations: WindowsWindowAdapter, MockWindowAdapter (tests).
pub trait WindowContextPort: Send + Sync {
    /// Get the currently active window's process name and title.
    fn get_active(&self) -> Result<WindowContext>;
}
