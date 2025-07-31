//! ByteBot Shared Rust Library
//!
//! This library contains shared types, utilities, and constants
//! used across ByteBot Rust services.

pub mod constants;
pub mod error;
pub mod logging;
pub mod types;
pub mod utils;

// Re-export commonly used types
pub use constants::*;
pub use logging::*;
pub use types::*;
pub use utils::*;
