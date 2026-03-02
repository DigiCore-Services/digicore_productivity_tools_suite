//! DigiCore Core - Shared domain, ports, and adapters.
//!
//! Hexagonal architecture: domain has no external I/O; adapters implement ports.

pub mod domain;
pub mod adapters;
