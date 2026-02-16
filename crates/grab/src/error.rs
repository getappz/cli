use thiserror::Error;

#[derive(Error, Debug)]
pub enum GrabError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("HTTP error: {0}")]
    HttpStatus(u16),

    #[error("Server does not support range requests (resume or parallel not available)")]
    NoRangeSupport,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("{0}")]
    Other(String),
}

pub type GrabResult<T> = Result<T, GrabError>;
