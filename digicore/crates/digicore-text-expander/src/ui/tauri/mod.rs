//! Tauri UI - integration points for Tauri web frontend.
//!
//! Tauri uses a web frontend (HTML/CSS/JS) in tauri-app/src/.
//! The Rust backend lives in tauri-app/src-tauri/ and depends on digicore-text-expander.
//! This module documents integration points: AppState, TauriStorageAdapter,
//! TauriWindowAdapter, TauriTimerAdapter, RfdFileDialogAdapter.

pub mod app;
