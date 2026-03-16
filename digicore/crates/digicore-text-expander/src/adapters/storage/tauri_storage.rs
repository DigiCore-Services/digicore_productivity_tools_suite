//! TauriStorageAdapter - StoragePort for Tauri GUI.
//!
//! Uses JsonFileStorageAdapter (same path: %APPDATA%/DigiCore/text_expander_state.json).
//! No Tauri-specific persistence; JSON file is framework-agnostic.
//!
//! Only compiled when feature `gui-tauri` is enabled.

use super::JsonFileStorageAdapter;

/// Storage adapter for Tauri GUI. Delegates to JsonFileStorageAdapter.
pub type TauriStorageAdapter = JsonFileStorageAdapter;
