//! NPM package source: download and extract from npm registry.

use std::path::PathBuf;
use std::time::Duration;

use starbase_archive::Archiver;
use starbase_utils::fs;
use tokio::time::timeout;
use tracing::debug;

use super::download::download_with_progress;
use crate::error::{InitError, InitResult};

/// Parse npm package string (package@version or package).
fn parse_npm_package(package: &str) -> InitResult<(String, String)> {
    let package = package.trim_start_matches("npm:");

    if package.starts_with('@') {
        if let Some(slash_idx) = package.find('/') {
            if let Some(at_idx) = package[slash_idx..].find('@') {
                let version_index = slash_idx + at_idx;
                let name = package[..version_index].to_string();
                let version = package[version_index + 1..].to_string();
                return Ok((name, version));
            }
            return Ok((package.to_string(), "latest".to_string()));
        }
        return Ok((package.to_string(), "latest".to_string()));
    }

    if let Some(at_idx) = package.rfind('@') {
        Ok((
            package[..at_idx].to_string(),
            package[at_idx + 1..].to_string(),
        ))
    } else {
        Ok((package.to_string(), "latest".to_string()))
    }
}

/// Resolve latest version from npm registry.
async fn resolve_npm_latest(package: &str) -> InitResult<String> {
    let url = format!("https://registry.npmjs.org/{}", package);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| InitError::DownloadFailed(format!("HTTP client: {}", e)))?;

    let response = timeout(Duration::from_secs(30), client.get(&url).send())
        .await
        .map_err(|_| InitError::DownloadFailed("Request timeout".to_string()))?
        .map_err(|e| InitError::DownloadFailed(format!("HTTP failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(InitError::NotFound(format!("Package not found: {}", package)));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| InitError::DownloadFailed(format!("Parse failed: {}", e)))?;

    let latest = json["dist-tags"]["latest"]
        .as_str()
        .ok_or_else(|| InitError::NotFound(format!("No latest version for: {}", package)))?;

    Ok(latest.to_string())
}

/// Download and extract an npm package.
pub async fn download_npm(package: &str, quiet: bool) -> InitResult<PathBuf> {
    let (package_name, mut version) = parse_npm_package(package)?;

    if version == "latest" {
        version = resolve_npm_latest(&package_name).await?;
    }

    let tarball_url = if let Some(idx) = package_name.find('/') {
        let pkg_name = &package_name[idx + 1..];
        format!(
            "https://registry.npmjs.org/{}/-/{}-{}.tgz",
            package_name, pkg_name, version
        )
    } else {
        format!(
            "https://registry.npmjs.org/{}/-/{}-{}.tgz",
            package_name, package_name, version
        )
    };

    debug!(url = %tarball_url, "Downloading npm package");

    let temp_dir = std::env::temp_dir().join(format!(
        "appz-init-npm-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir)
        .map_err(|e| InitError::FsError(format!("Temp dir: {}", e)))?;

    let tarball_file = temp_dir.join("package.tgz");
    download_with_progress(&tarball_url, &tarball_file, "Downloading npm package...", quiet).await?;

    let extracted_dir = temp_dir.join("extracted");
    fs::create_dir_all(&extracted_dir)
        .map_err(|e| InitError::FsError(format!("Extracted dir: {}", e)))?;

    Archiver::new(&extracted_dir, &tarball_file)
        .set_prefix("package")
        .unpack_from_ext()
        .map_err(|e| InitError::Archive(format!("Extract failed: {:?}", e)))?;

    let _ = std::fs::remove_file(&tarball_file);

    Ok(extracted_dir)
}
