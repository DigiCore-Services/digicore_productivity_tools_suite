//! Adapters module - implementations of domain ports.

pub mod corpus;
pub mod export;
pub mod extraction;
pub mod file_dialog;
pub mod storage;
pub mod timer;
pub mod window;
pub mod sqlite_clipboard;

pub use corpus::*;
pub use export::*;
pub use extraction::*;
pub use file_dialog::*;
pub use storage::*;
pub use timer::*;
pub use window::*;
pub use sqlite_clipboard::*;

// Convenience re-exports
#[cfg(feature = "gui-egui")]
pub use storage::EframeStorageAdapter;
#[cfg(feature = "gui-egui")]
pub use window::EguiWindowAdapter;
#[cfg(feature = "gui-tauri")]
pub use window::TauriWindowAdapter;
pub use file_dialog::RfdFileDialogAdapter;
#[cfg(feature = "gui-egui")]
pub use timer::EguiTimerAdapter;
#[cfg(feature = "gui-tauri")]
pub use timer::TauriTimerAdapter;
