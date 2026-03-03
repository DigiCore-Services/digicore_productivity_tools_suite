//! Text Expander library - expansion engine and application logic.
//!
//! Hexagonal architecture: domain/ports in digicore-core; adapters here.
//! Phase 0/1: ports (StoragePort) and adapters (EframeStorageAdapter).

pub mod adapters;
pub mod app_config;
pub mod application;
pub mod cli;
pub mod drivers;
pub mod platform;
pub mod ports;
pub mod services;
pub mod utils;

#[cfg(feature = "gui-tauri")]
pub mod tauri_stub;

#[cfg(feature = "gui-tauri")]
pub mod ui;
