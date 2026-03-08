//! UI components - SRP: one responsibility per tab/component.
//!
//! Each module renders a single tab or modal. Orchestration remains in App.
//! ui/egui/ = egui (gui-egui); ui/tauri/ = Tauri (gui-tauri, web frontend in tauri-app/).

#[cfg(feature = "gui-egui")]
pub mod egui;

#[cfg(feature = "gui-egui")]
pub use egui::{clipboard_history_tab, configuration_tab, library_tab, modals, script_library_tab};

#[cfg(feature = "gui-tauri")]
pub mod tauri;
