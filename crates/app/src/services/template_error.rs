use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum TemplateError {
    #[error("Invalid template source format: {0}")]
    #[diagnostic(code(app::template::invalid_format))]
    InvalidFormat(String),

    #[error("Template source not found: {0}")]
    #[diagnostic(code(app::template::not_found))]
    NotFound(String),

    #[error("Failed to download template: {0}")]
    #[diagnostic(code(app::template::download_failed))]
    DownloadFailed(String),

    #[error("Failed to extract archive: {0}")]
    #[diagnostic(code(app::template::extraction_failed))]
    ExtractionFailed(String),

    #[error("Subfolder not found in template: {0}")]
    #[diagnostic(code(app::template::subfolder_not_found))]
    SubfolderNotFound(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Filesystem error: {0}")]
    #[diagnostic(code(app::template::fs_error))]
    FsError(String),

    #[error("Archive error: {0}")]
    #[diagnostic(code(app::template::archive_error))]
    Archive(String),
}
