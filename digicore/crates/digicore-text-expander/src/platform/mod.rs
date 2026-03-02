//! Platform-specific implementations.

pub mod timezone;
#[cfg(target_os = "windows")]
pub mod windows_caret;
#[cfg(target_os = "windows")]
pub mod windows_monitor;
#[cfg(target_os = "windows")]
pub mod windows_keyboard;
#[cfg(target_os = "windows")]
pub mod windows_window;
#[cfg(target_os = "windows")]
pub mod windows_clipboard_listener;
