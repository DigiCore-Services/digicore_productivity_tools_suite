//! Ports - traits (interfaces) for outbound adapters.

pub mod snippet_repository;
pub mod input;
pub mod clipboard;
pub mod window_context;
pub mod crypto;
pub mod sync;

pub use snippet_repository::SnippetRepository;
pub use input::{InputPort, Key};
pub use clipboard::ClipboardPort;
pub use window_context::{WindowContextPort, WindowContext};
pub use crypto::CryptoPort;
pub use sync::SyncPort;
