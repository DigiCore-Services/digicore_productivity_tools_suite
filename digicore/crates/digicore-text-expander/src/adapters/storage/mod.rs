//! Storage adapters - implement StoragePort for specific backends.

#[cfg(feature = "gui-egui")]
pub mod eframe_storage;
pub mod json_file_storage;

#[cfg(feature = "gui-tauri")]
pub mod tauri_storage;

#[cfg(feature = "gui-egui")]
pub use eframe_storage::EframeStorageAdapter;
pub use json_file_storage::JsonFileStorageAdapter;

#[cfg(feature = "gui-tauri")]
pub use tauri_storage::TauriStorageAdapter;
