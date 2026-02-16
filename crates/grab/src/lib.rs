//! Async file download with optional parallel chunk downloads and progress reporting.
//!
//! Uses HEAD to detect size and range support; for large files with range support,
//! downloads chunks in parallel. Otherwise falls back to single-stream download.

mod download;
mod error;
mod progress;

pub use download::{download_to_path, DownloadOptions};
pub use error::{GrabError, GrabResult};
pub use progress::Progress;
pub use reqwest::header::HeaderMap;
pub use std::sync::Arc;
pub use std::time::Duration;
