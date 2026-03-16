use std::path::PathBuf;

use crate::runtime::RuntimeError;

#[derive(Debug, thiserror::Error)]
pub enum BlueprintError {
    #[error("Failed to read blueprint file {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse blueprint JSON: {0}")]
    Parse(String),

    #[error("Step {step_index} ({step_type}) failed: {message}")]
    StepFailed {
        step_index: usize,
        step_type: String,
        message: String,
    },

    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error("Not a WordPress project. blueprint commands require a WordPress project with a runtime configured.")]
    NotWordPressProject,

    #[error("Unsupported resource type: {0}")]
    UnsupportedResource(String),
}
