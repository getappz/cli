//! Common utilities crate for shared functionality across the codebase.
//!
//! This crate provides constants, path utilities, ID helpers, environment utilities,
//! and common types used across multiple crates.

pub mod consts;
pub mod env;
pub mod hardening;
pub mod timing;
pub mod head_tail_buffer;
pub mod id;
pub mod path;
pub mod types;
pub mod user_config;

// Re-export commonly used items
pub use consts::*;
pub use env::*;
pub use head_tail_buffer::HeadTailBuffer;
pub use id::*;
pub use path::*;
pub use types::*;
