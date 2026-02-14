//! Init output types.

use std::path::PathBuf;

/// The result of a successful project initialization.
#[derive(Debug, Clone)]
pub struct InitOutput {
    /// Path where the project was created.
    pub project_path: PathBuf,

    /// Framework detected in the project (if any).
    pub framework: Option<String>,

    /// Whether dependencies were installed.
    pub installed: bool,
}
