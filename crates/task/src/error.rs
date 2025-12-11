use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum TaskError {
    #[diagnostic(code(task::not_found))]
    #[error(
        "Task not found: {}",
        .0
    )]
    TaskNotFound(String),

    #[diagnostic(code(task::circular_dependency))]
    #[error(
        "Circular dependency detected in task: {}",
        .0
    )]
    CircularDependency(String),

    #[diagnostic(code(task::registry::build_failed))]
    #[error("Failed to build task registry")]
    RegistryBuildFailed,

    #[diagnostic(code(task::execution::failed))]
    #[error(
        "Task execution failed: {}",
        .0
    )]
    ExecutionFailed(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type TaskResult<T = ()> = miette::Result<T>;
