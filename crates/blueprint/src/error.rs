use std::path::PathBuf;

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

    #[error("DDEV command failed: {command}\n{message}")]
    DdevFailed { command: String, message: String },

    #[error("DDEV is not available. Install it: https://docs.ddev.com/en/stable/users/install/ddev-installation/")]
    DdevNotAvailable,

    #[error("Not a WordPress DDEV project. blueprint apply requires a WordPress project with DDEV configured.")]
    NotWordPressProject,

    #[error("Unsupported resource type: {0}")]
    UnsupportedResource(String),
}
