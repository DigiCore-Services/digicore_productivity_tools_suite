//! Timer adapters - implement TimerPort.

#[cfg(feature = "gui-egui")]
pub mod egui_timer;

#[cfg(feature = "gui-tauri")]
pub mod tauri_timer;

#[cfg(feature = "gui-egui")]
pub use egui_timer::EguiTimerAdapter;

#[cfg(feature = "gui-tauri")]
pub use tauri_timer::{TauriTimerAdapter, TauriTimerContext};
