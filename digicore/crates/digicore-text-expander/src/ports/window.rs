//! WindowPort - framework-agnostic viewport/window management.
//!
//! Part of Phase 0/1 UI decoupling. Abstracts close and command operations.
//! show_viewport is deferred (see DECISION in UI_DECOUPLING_IMPLEMENTATION_PLAN.md).
//!
//! Implementations: EguiWindowAdapter (egui), TauriWindowAdapter (Tauri).

/// Viewport descriptor - framework-agnostic window configuration.
#[derive(Debug, Clone)]
pub struct ViewportDescriptor {
    pub id: String,
    pub title: String,
    pub size: (f32, f32),
    pub position: Option<(f32, f32)>,
    pub always_on_top: bool,
    pub decorations: bool,
    pub taskbar: bool,
}

/// Window level (normal vs always-on-top).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowLevel {
    Normal,
    AlwaysOnTop,
}

/// Viewport command - framework-agnostic intent.
#[derive(Debug, Clone)]
pub enum ViewportCommand {
    Visible(bool),
    Minimized(bool),
    Maximized(bool),
    Focus,
    Close,
    WindowLevel(WindowLevel),
}

/// Port for viewport/window management (close, send command).
///
/// Adapter receives framework context at construction (e.g. EguiWindowAdapter::new(ctx))
/// and is created each frame in update(). show_viewport is deferred - egui's
/// show_viewport_immediate is callback-based; see DECISION in implementation plan.
pub trait WindowPort: Send + Sync {
    /// Close viewport by id.
    fn close_viewport(&self, id: &str);

    /// Send command (visible, minimized, focus, etc.).
    fn send_viewport_command(&self, id: &str, cmd: ViewportCommand);
}
