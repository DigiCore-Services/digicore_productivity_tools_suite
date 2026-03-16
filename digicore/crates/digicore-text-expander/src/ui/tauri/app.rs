//! TauriApp - integration points for Tauri GUI.
//!
//! The Tauri app lives in digicore/tauri-app/ with:
//! - Frontend: src/index.html, src/*.js (or framework of choice)
//! - Backend: src-tauri/src/lib.rs (Tauri commands that call digicore-text-expander)
//!
//! Integration points:
//! - AppState: framework-agnostic application state
//! - TauriStorageAdapter: JsonFileStorageAdapter for persistence
//! - TauriWindowAdapter: WebviewWindow for Ghost Follower, Ghost Suggestor, Variable Input
//! - TauriTimerAdapter: Channel-based repaint scheduling
//! - RfdFileDialogAdapter: File picker (or Tauri's native dialog)
//!
//! Run: cd digicore/tauri-app && npm run tauri dev
