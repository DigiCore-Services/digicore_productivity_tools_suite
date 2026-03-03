//! Window adapters - implement WindowPort for specific frameworks.

#[cfg(feature = "gui-egui")]
pub mod egui_window;

#[cfg(feature = "gui-tauri")]
pub mod tauri_window;

#[cfg(feature = "gui-egui")]
pub use egui_window::EguiWindowAdapter;

#[cfg(feature = "gui-tauri")]
pub use tauri_window::{TauriViewportState, TauriWindowAdapter};
