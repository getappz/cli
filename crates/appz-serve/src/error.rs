use thiserror::Error;

pub type Result<T> = std::result::Result<T, ServeError>;

#[derive(Error, Debug)]
pub enum ServeError {
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
    WebSocket(#[from] Box<tokio_tungstenite::tungstenite::Error>),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Configuration error: {0}")]
    Config(String),
}
