//! TauriWindowAdapter - WindowPort for Tauri GUI.
//!
//! Framework-agnostic: holds a shared registry of viewport close/command requests.
//! When Tauri is wired, the app processes these via WebviewWindow API.
//!
//! Only compiled when feature `gui-tauri` is enabled.

use crate::ports::{ViewportCommand, WindowPort};
use std::collections::HashMap;
use std::sync::Mutex;

/// Shared state for viewport commands - processed by the Tauri app when rendering.
#[derive(Default)]
pub struct TauriViewportState {
    /// Pending close requests (viewport ids).
    pub close_requests: Vec<String>,
    /// Pending commands per viewport.
    pub commands: HashMap<String, Vec<ViewportCommand>>,
}

impl TauriViewportState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_close_requests(&mut self) -> Vec<String> {
        std::mem::take(&mut self.close_requests)
    }

    pub fn take_commands(&mut self, id: &str) -> Vec<ViewportCommand> {
        self.commands.remove(id).unwrap_or_default()
    }
}

/// Window adapter for Tauri. Uses shared state for viewport close/command requests.
pub struct TauriWindowAdapter {
    state: std::sync::Arc<Mutex<TauriViewportState>>,
}

impl TauriWindowAdapter {
    pub fn new(state: std::sync::Arc<Mutex<TauriViewportState>>) -> Self {
        Self { state }
    }

    /// Create with a new shared state.
    pub fn with_shared_state() -> (Self, std::sync::Arc<Mutex<TauriViewportState>>) {
        let state = std::sync::Arc::new(Mutex::new(TauriViewportState::new()));
        let adapter = Self::new(state.clone());
        (adapter, state)
    }
}

impl WindowPort for TauriWindowAdapter {
    fn close_viewport(&self, id: &str) {
        if let Ok(mut s) = self.state.lock() {
            s.close_requests.push(id.to_string());
        }
    }

    fn send_viewport_command(&self, id: &str, cmd: ViewportCommand) {
        if let Ok(mut s) = self.state.lock() {
            s.commands
                .entry(id.to_string())
                .or_default()
                .push(cmd);
        }
    }
}
