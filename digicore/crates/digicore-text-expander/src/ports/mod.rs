//! Ports - framework-agnostic interfaces for UI decoupling.
//!
//! Phase 0/1: StoragePort, WindowPort. Phase 2: FileDialogPort, TimerPort.

pub mod data_path_resolver;
pub mod storage;
pub mod window;
pub mod file_dialog;
pub mod timer;

pub use storage::{keys as storage_keys, StoragePort};
pub use window::{ViewportCommand, ViewportDescriptor, WindowLevel, WindowPort};
pub use file_dialog::FileDialogPort;
pub use timer::TimerPort;
