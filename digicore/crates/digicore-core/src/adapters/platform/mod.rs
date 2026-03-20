//! Platform adapters - input, clipboard, window context.

#[cfg(feature = "platform-windows")]
pub mod input;
#[cfg(feature = "platform-windows")]
pub mod clipboard;
#[cfg(feature = "platform-windows")]
pub mod clipboard_windows;
#[cfg(feature = "platform-windows")]
pub mod window;

/// Mock adapters for testing (always available).
pub mod mock;
