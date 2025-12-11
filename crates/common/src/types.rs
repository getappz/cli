//! Common type aliases and shared types.

use std::path::PathBuf;

/// Common result type using miette
pub type Result<T> = miette::Result<T>;

/// Path buffer type alias
pub type Path = PathBuf;

/// String type alias for IDs
pub type Id = String;

/// String type alias for names
pub type Name = String;

/// String type alias for descriptions
pub type Description = String;

/// Common error type
pub type Error = miette::Error;
