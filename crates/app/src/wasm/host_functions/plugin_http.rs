//! HTTP download host function for downloadable plugins.
//!
//! Downloads a URL directly to a file within the ScopedFs sandbox.
//! Uses the centralized grab crate for fetch logic.

use extism::{convert::Json, host_fn};
use grab::{fetch_bytes, FetchOptions};
use std::time::Duration;
use tokio::task;

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
    let url = input.url.clone();
    let dest_path = input.dest_path.clone();
    let scoped_fs = scoped_fs.clone();

    let opts = FetchOptions {
        timeout: Duration::from_secs(60),
        danger_accept_invalid_certs: !strict_ssl,
        ..Default::default()
    };

    let result = task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let bytes = fetch_bytes(&url, opts).await.map_err(|e| e.user_message())?;
            scoped_fs
                .write_file(&dest_path, &bytes)
                .map_err(|e| format!("Failed to write {}: {}", dest_path, e))?;
            Ok::<_, String>(bytes.len() as u64)
        })
    });

    match result {
        Ok(bytes_len) => Ok(Json(PluginHttpDownloadOutput {
            success: true,
            bytes_written: Some(bytes_len),
            error: None,
        })),
        Err(e) => Ok(Json(PluginHttpDownloadOutput {
            success: false,
            bytes_written: None,
            error: Some(e),
        })),
    }
});
