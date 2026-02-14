//! Remote archive source: download and extract from any .zip/.tar.gz URL.

use std::path::PathBuf;

use starbase_archive::Archiver;
use starbase_utils::fs;

use super::download::download_with_progress;
use crate::error::{InitError, InitResult};

/// Download and extract a remote archive URL.
pub async fn download_remote_archive(url: &str, quiet: bool) -> InitResult<PathBuf> {
    let temp_dir = std::env::temp_dir().join(format!(
        "appz-init-archive-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir)
        .map_err(|e| InitError::FsError(format!("Temp dir: {}", e)))?;

    let path_part = url.split('?').next().unwrap_or(url);
    let filename = path_part.rsplit('/').next().unwrap_or("archive");
    let archive_file = temp_dir.join(filename);

    download_with_progress(url, &archive_file, "Downloading archive...", quiet).await?;

    let extracted_dir = temp_dir.join("extracted");
    fs::create_dir_all(&extracted_dir)
        .map_err(|e| InitError::FsError(format!("Extracted dir: {}", e)))?;

    Archiver::new(&extracted_dir, &archive_file)
        .unpack_from_ext()
        .map_err(|e| InitError::Archive(format!("Extract failed: {:?}", e)))?;

    let _ = std::fs::remove_file(&archive_file);

    let extracted_repo_dir = fs::read_dir(&extracted_dir)
        .map_err(|e| InitError::FsError(format!("Read dir: {}", e)))?
        .into_iter()
        .filter_map(|e| {
            let p = e.path();
            if p.is_dir() {
                Some(p)
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| InitError::ExtractionFailed("No directory in archive".to_string()))?;

    Ok(extracted_repo_dir)
}
