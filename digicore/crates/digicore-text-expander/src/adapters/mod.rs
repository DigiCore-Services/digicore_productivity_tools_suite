//! Adapters - implement ports for specific frameworks.
//!
//! Phase 0/1: storage (EframeStorageAdapter), window (EguiWindowAdapter).

pub mod storage;
pub mod window;
pub mod file_dialog;
pub mod timer;
pub mod extraction;
pub mod corpus;
pub mod export;


#[cfg(feature = "gui-egui")]
pub use storage::EframeStorageAdapter;
pub use storage::JsonFileStorageAdapter;
pub use file_dialog::RfdFileDialogAdapter;
#[cfg(feature = "gui-egui")]
pub use timer::EguiTimerAdapter;
#[cfg(feature = "gui-tauri")]
pub use timer::{TauriTimerAdapter, TauriTimerContext};
#[cfg(feature = "gui-tauri")]
pub use storage::TauriStorageAdapter;
#[cfg(feature = "gui-egui")]
pub use window::EguiWindowAdapter;
#[cfg(feature = "gui-tauri")]
pub use window::{TauriViewportState, TauriWindowAdapter};
