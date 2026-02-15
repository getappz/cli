//! Download with progress bar.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use tokio::time::timeout;

use crate::error::{InitError, InitResult};

/// Download a URL to a file with progress bar.
pub async fn download_with_progress(
    url: &str,
    dest: &Path,
    label: &str,
    quiet: bool,
) -> InitResult<()> {
    use ui::progress;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent("appz-cli")
        .build()
        .map_err(|e| InitError::DownloadFailed(format!("Failed to create HTTP client: {}", e)))?;

    let mut response = client.get(url).send().await.map_err(|e| {
        InitError::DownloadFailed(format!(
            "HTTP request failed for {}: {}",
            url, e
        ))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(InitError::DownloadFailed(format!(
            "HTTP error: {} - {}",
            status,
            response.url()
        )));
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = if quiet {
        None
    } else {
        Some(progress::progress_bar(0, label))
    };

    let mut file = std::fs::File::create(dest)
        .map_err(|e| InitError::FsError(format!("Failed to create file: {}", e)))?;

    let mut downloaded: u64 = 0;
    while let Some(chunk) = timeout(
        Duration::from_secs(60),
        response.chunk(),
    )
    .await
    .map_err(|_| InitError::DownloadFailed("Download timeout".to_string()))?
    .map_err(|e| InitError::DownloadFailed(format!("Failed to read chunk: {}", e)))?
    {
        use std::io::Write;
        file.write_all(&chunk)
            .map_err(|e| InitError::FsError(format!("Failed to write: {}", e)))?;
        downloaded += chunk.len() as u64;
        if let Some(ref p) = pb {
            if total_size > 0 {
                p.set_length(total_size);
                p.set_position(downloaded);
            } else {
                p.inc_by(chunk.len() as u64);
            }
        }
    }

    drop(pb);

    file.flush()
        .map_err(|e| InitError::FsError(format!("Failed to flush: {}", e)))?;

    Ok(())
}
