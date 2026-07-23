use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum CommandError {
    #[diagnostic(code(command::exec::spawn_failed))]
    #[error(
        "Failed to spawn command: {}",
        .0
    )]
    SpawnFailed(String),

    #[diagnostic(code(command::exec::execution_failed))]
    #[error(
        "Command execution failed: {}",
        .0
    )]
    ExecutionFailed(String),

    #[diagnostic(code(command::exec::join_path_failed))]
    #[error(
        "Failed to join PATH: {}",
        .0
    )]
    JoinPathFailed(String),

    #[diagnostic(code(command::exec::write_stdin_failed))]
    #[error(
        "Failed to write to stdin: {}",
        .0
    )]
    WriteStdinFailed(String),

    #[diagnostic(code(command::exec::command_failed))]
    #[error(
        "Command failed: '{}' (exit code {:?})\nstdout: {}\nstderr: {}",
        .command,
        .exit_code,
        .stdout,
        .stderr
    )]
    CommandFailed {
        command: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
