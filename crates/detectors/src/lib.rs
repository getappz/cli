//! Project detection: frameworks, package manager, languages, tools, testing.

pub mod detect;
pub mod filesystem;
pub mod languages;
pub mod tools;

mod testing;

pub use detect::*;
pub use filesystem::{DetectorFilesystem, StdFilesystem};
pub use languages::detect_languages;
pub use tools::detect_tools;

