//! Native [`Wp2mdVfs`] implementation using `std::fs` and `reqwest`.
//!
//! Only compiled when the `native` feature is enabled.

use crate::vfs::Wp2mdVfs;
use miette::{miette, IntoDiagnostic, Result};
use std::fs;
use std::path::Path;

/// Native filesystem + HTTP implementation.
pub struct NativeFs;

impl Wp2mdVfs for NativeFs {
    fn read_to_string(&self, path: &str) -> Result<String> {
        fs::read_to_string(path)
            .map_err(|e| miette!("Failed to read {}: {}", path, e))
    }

    fn write_string(&self, path: &str, content: &str) -> Result<()> {
        self.write_bytes(path, content.as_bytes())
    }

    fn write_bytes(&self, path: &str, data: &[u8]) -> Result<()> {
        let p = Path::new(path);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| miette!("Failed to create directory {}: {}", parent.display(), e))?;
        }
        fs::write(p, data)
            .map_err(|e| miette!("Failed to write {}: {}", path, e))
    }

    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn create_dir_all(&self, path: &str) -> Result<()> {
        fs::create_dir_all(path)
            .map_err(|e| miette!("Failed to create directory {}: {}", path, e))
    }

    fn download_to_file(&self, url: &str, dest: &str, strict_ssl: bool) -> Result<()> {
        let client = reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(!strict_ssl)
            .build()
            .into_diagnostic()?;

        let encoded_url = encode_url_if_needed(url);

        let response = client
            .get(&encoded_url)
            .send()
            .map_err(|e| miette!("HTTP request failed for {}: {}", url, e))?;

        if !response.status().is_success() {
            return Err(miette!("HTTP {} for {}", response.status(), url));
        }

        let bytes = response
            .bytes()
            .map_err(|e| miette!("Failed to read response body for {}: {}", url, e))?;

        self.write_bytes(dest, &bytes)
    }
}

/// Encode a URL only if it doesn't already contain encoded characters.
fn encode_url_if_needed(url: &str) -> String {
    if url.contains('%') {
        url.to_string()
    } else {
        match url::Url::parse(url) {
            Ok(parsed) => parsed.to_string(),
            Err(_) => url.to_string(),
        }
    }
}
