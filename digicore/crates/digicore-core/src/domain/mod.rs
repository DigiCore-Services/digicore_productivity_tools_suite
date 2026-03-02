//! Domain layer - entities, value objects, ports.
//! No external I/O; pure business logic.

pub mod entities;
pub mod value_objects;
pub mod ports;

pub use entities::*;
pub use value_objects::*;
pub use ports::*;
