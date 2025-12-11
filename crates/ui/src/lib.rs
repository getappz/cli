//! UI/UX components crate for the CLI application.
//!
//! This crate provides reusable components for displaying data, user interactions,
//! and status feedback across all CLI commands.

pub mod banner;
pub mod empty;
pub mod error;
pub mod format;
pub mod layout;
pub mod list;
pub mod pagination;
pub mod progress;
pub mod prompt;
pub mod status;
pub mod table;

// Re-export commonly used types
pub use prompt::{
    checkbox, choose, choose_multiple, confirm, password, prompt, select_with_value,
    text_with_transformer, text_with_validation,
};
