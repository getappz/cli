//! HTTP download host function for downloadable plugins.
//!
//! Downloads a URL directly to a file within the ScopedFs sandbox.
//! This is a generic host function useful for any plugin that needs
//! to fetch remote binary content (images, archives, etc.).

use extism::{convert::Json, host_fn};

use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

host_fn!(pub appz_phttp_download(
    user_data: PluginHostData;
    args: Json<PluginHttpDownloadInput>
) -> Json<PluginHttpDownloadOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref scoped_fs) = data.scoped_fs else {
        return Ok(Json(PluginHttpDownloadOutput {
            success: false,
            bytes_written: None,
            error: Some("Filesystem not available for this plugin".to_string()),
        }));
    };

    let strict_ssl = input.strict_ssl.unwrap_or(true);

    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(!strict_ssl)
        .build()
        .map_err(|e| extism::Error::msg(format!("Failed to build HTTP client: {}", e)))?;

    let response = match client.get(&input.url).send() {
        Ok(r) => r,
        Err(e) => {
            return Ok(Json(PluginHttpDownloadOutput {
                success: false,
                bytes_written: None,
                error: Some(format!("HTTP request failed for {}: {}", input.url, e)),
            }));
        }
    };

    if !response.status().is_success() {
        return Ok(Json(PluginHttpDownloadOutput {
            success: false,
            bytes_written: None,
            error: Some(format!(
                "HTTP {} for {}",
                response.status(),
                input.url
            )),
        }));
    }

    let bytes = match response.bytes() {
        Ok(b) => b,
        Err(e) => {
            return Ok(Json(PluginHttpDownloadOutput {
                success: false,
                bytes_written: None,
                error: Some(format!("Failed to read response body: {}", e)),
            }));
        }
    };

    let bytes_len = bytes.len() as u64;

    match scoped_fs.write_file(&input.dest_path, &bytes) {
        Ok(()) => Ok(Json(PluginHttpDownloadOutput {
            success: true,
            bytes_written: Some(bytes_len),
            error: None,
        })),
        Err(e) => Ok(Json(PluginHttpDownloadOutput {
            success: false,
            bytes_written: None,
            error: Some(format!("Failed to write {}: {}", input.dest_path, e)),
        })),
    }
});
