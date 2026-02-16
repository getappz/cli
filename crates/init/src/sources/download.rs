//! Download with progress bar (delegates to grab crate).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use grab::{download_to_path, DownloadOptions, GrabError};

use crate::error::{InitError, InitResult};

/// Download a URL to a file with progress bar.
pub async fn download_with_progress(
    url: &str,
    dest: &Path,
    label: &str,
    quiet: bool,
) -> InitResult<()> {
    let progress = if quiet {
        None
    } else {
        Some(Arc::new(ui::progress::progress_bar(0, label)) as Arc<dyn grab::Progress>)
    };

    let options = DownloadOptions {
        timeout: Duration::from_secs(300),
        user_agent: "appz-cli".to_string(),
        parallel_threshold_bytes: 5 * 1024 * 1024, // 5 MiB
        max_concurrent_chunks: 4,
        chunk_size: 1024 * 1024,
        resume: false,
        headers: None,
    };

    download_to_path(url, dest, options, progress).await.map_err(grab_error_to_init)
}

fn grab_error_to_init(e: GrabError) -> InitError {
    match e {
        GrabError::Request(err) => InitError::DownloadFailed(format!("HTTP request failed: {}", err)),
        GrabError::HttpStatus(code) => {
            InitError::DownloadFailed(format!("HTTP error: {} - check URL and network", code))
        }
        GrabError::NoRangeSupport => {
            InitError::DownloadFailed("Server does not support range requests".to_string())
        }
        GrabError::Io(err) => InitError::FsError(format!("IO error: {}", err)),
        GrabError::Timeout(msg) => InitError::DownloadFailed(msg),
        GrabError::Other(msg) => InitError::DownloadFailed(msg),
    }
}
