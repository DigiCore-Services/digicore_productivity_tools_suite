//! Outbound adapters - implement ports.

pub mod persistence;
pub mod platform;

#[cfg(feature = "sync")]
pub mod crypto;
#[cfg(feature = "sync")]
pub mod sync;
