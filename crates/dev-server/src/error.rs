use thiserror::Error;

/// Result type for dev server operations
pub type Result<T> = std::result::Result<T, DevServerError>;

/// Error types for the dev server
#[derive(Error, Debug)]
pub enum DevServerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] hyper::Error),

    #[error("HTTP error: {0}")]
    HttpError(#[from] hyper::http::Error),

    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] hyper::header::InvalidHeaderValue),

    #[error("File watcher error: {0}")]
    Notify(#[from] notify::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Multipart error: {0}")]
    Multipart(#[from] multer::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Server already running")]
    AlreadyRunning,

    #[error("Server not running")]
    NotRunning,

    #[error("Configuration error: {0}")]
    Config(String),
}
