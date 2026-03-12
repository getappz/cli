//! Init error types with rich diagnostic help messages.

#![allow(unused_assignments)]

use miette::Diagnostic;
use thiserror::Error;

/// Result type alias for init operations.
pub type InitResult<T> = Result<T, InitError>;

/// Errors that can occur during project initialization.
#[derive(Error, Debug, Diagnostic)]
pub enum InitError {
    #[diagnostic(
        code(init::source_not_found),
        help("Use a framework slug (e.g. astro, nextjs), a git URL, npm:package, or local path.")
    )]
    #[error("Unknown init source: {0}")]
    SourceNotFound(String),

    #[diagnostic(
        code(init::download_failed),
        help("Check your network connection and that the URL is accessible.")
    )]
    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[diagnostic(
        code(init::extraction_failed),
        help("Verify the archive file is not corrupted and is a supported format.")
    )]
    #[error("Extraction failed: {0}")]
    ExtractionFailed(String),

    #[diagnostic(
        code(init::not_found),
        help("Verify the repository, package, or path exists and is accessible.")
    )]
    #[error("Not found: {0}")]
    NotFound(String),

    #[diagnostic(
        code(init::invalid_format),
        help("Check the source format. Use user/repo for GitHub, npm:package for npm, or a full URL.")
    )]
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[diagnostic(
        code(init::command_failed),
        help("Check the command output above for details.")
    )]
    #[error("Command failed: {0}\n{1}")]
    CommandFailed(String, String),

    #[diagnostic(
        code(init::directory_exists),
        help("Use --force to overwrite the existing directory.")
    )]
    #[error("Directory already exists: {0}")]
    DirectoryExists(String),

    #[diagnostic(
        code(init::archive_error),
        help("Ensure the file is a valid archive (zip, tar.gz, tar.xz, tar.zstd).")
    )]
    #[error("Archive error: {0}")]
    Archive(String),

    #[diagnostic(
        code(init::fs_error),
        help("Check file permissions and available disk space.")
    )]
    #[error("Filesystem error: {0}")]
    FsError(String),

    #[diagnostic(code(init::other))]
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for InitError {
    fn from(err: std::io::Error) -> Self {
        InitError::FsError(err.to_string())
    }
}

impl From<sandbox::SandboxError> for InitError {
    fn from(err: sandbox::SandboxError) -> Self {
        InitError::CommandFailed("sandbox operation".into(), err.to_string())
    }
}
