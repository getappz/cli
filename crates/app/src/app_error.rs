#![allow(dead_code)]

use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum AppError {
    #[diagnostic(code(app::workspace::invalid_root_env))]
    #[error(
        "Unable to determine workspace root. Failed to parse {} into a valid path.",
        "APPZ_WORKSPACE_ROOT"
    )]
    InvalidWorkspaceRootEnvVar,

    #[diagnostic(code(app::missing_working_dir))]
    #[error("Unable to determine your current working directory.")]
    MissingWorkingDir,

    #[diagnostic(code(app::blueprint::invalid_file))]
    #[error(
        "Unable to parse blueprint file: {}",
        .0
    )]
    InvalidRecipeFile(String),

    #[diagnostic(code(app::blueprint::file_not_found))]
    #[error(
        "Blueprint file not found: {}",
        .0
    )]
    RecipeFileNotFound(String),

    #[diagnostic(code(app::plugin::load_failed))]
    #[error(
        "Failed to load WASM plugin: {}",
        .0
    )]
    PluginLoadFailed(String),

    #[diagnostic(code(app::plugin::invalid_id))]
    #[error(
        "Invalid plugin ID: {}",
        .0
    )]
    InvalidPluginId(String),

    #[diagnostic(code(app::task::not_found))]
    #[error(
        "Task not found: {}",
        .0
    )]
    TaskNotFound(String),

    #[diagnostic(code(app::task::registry::build_failed))]
    #[error("Failed to build task registry")]
    TaskRegistryBuildFailed,

    #[diagnostic(code(app::importer::import_failed))]
    #[error(
        "Failed to import recipe: {}",
        .0
    )]
    ImportFailed(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error(transparent)]
    Command(#[from] command::CommandError),

    #[error(transparent)]
    Inquire(#[from] inquire::InquireError),
}

/// User-initiated cancellation (e.g. Esc, No, Ctrl+C). Not an error — exit gracefully.
#[derive(Error, Debug, Diagnostic)]
#[error("{message}")]
pub struct UserCancellation {
    pub message: String,
}

impl UserCancellation {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }

    pub fn project_setup() -> Self {
        Self::new("Project setup cancelled.")
    }

    pub fn selection() -> Self {
        Self::new("Selection cancelled.")
    }
}
