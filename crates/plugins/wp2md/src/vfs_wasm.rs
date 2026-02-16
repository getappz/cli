//! WASM [`Wp2mdVfs`] implementation using PDK host functions.
//!
//! Delegates filesystem operations to `appz_pfs_*` host functions
//! and HTTP downloads to `appz_phttp_download`.

use appz_pdk::prelude::*;
use extism_pdk::*;
use miette::miette;
use wp2md::vfs::Wp2mdVfs;

/// Vfs implementation backed by PDK host function calls.
pub struct WasmVfs;

macro_rules! host_call {
    ($fn:ident, $input:expr) => {
        unsafe { super::$fn($input) }.map(|j| j.0).map_err(|e| miette!("{}", e))
    };
}

macro_rules! host_call_bool {
    ($fn:ident, $input:expr, $field:ident) => {
        unsafe { super::$fn($input) }
            .map(|j| j.0.$field)
            .unwrap_or(false)
    };
}

impl Wp2mdVfs for WasmVfs {
    fn read_to_string(&self, path: &str) -> miette::Result<String> {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        let out = host_call!(appz_pfs_read_file, input)?;
        if out.success {
            Ok(out.content.unwrap_or_default())
        } else {
            Err(miette!(
                "Failed to read {}: {}",
                path,
                out.error.unwrap_or_default()
            ))
        }
    }

    fn write_string(&self, path: &str, content: &str) -> miette::Result<()> {
        let input = Json(PluginFsWriteInput {
            path: path.to_string(),
            content: content.to_string(),
        });
        let out = host_call!(appz_pfs_write_file, input)?;
        if out.success {
            Ok(())
        } else {
            Err(miette!(
                "Failed to write {}: {}",
                path,
                out.error.unwrap_or_default()
            ))
        }
    }

    fn write_bytes(&self, path: &str, _data: &[u8]) -> miette::Result<()> {
        // The host write_file function only supports string content.
        // For binary writes, use download_to_file which handles binary on
        // the host side. If direct byte writes are needed, the content can
        // be base64-encoded, but for wp2md only markdown (string) writes
        // and image downloads (via download_to_file) are needed.
        Err(miette!(
            "Direct byte writes not supported in WASM; use download_to_file for binary content: {}",
            path
        ))
    }

    fn exists(&self, path: &str) -> bool {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        host_call_bool!(appz_pfs_exists, input, exists)
    }

    fn create_dir_all(&self, path: &str) -> miette::Result<()> {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        let out = host_call!(appz_pfs_mkdir, input)?;
        if out.success {
            Ok(())
        } else {
            Err(miette!(
                "Failed to mkdir {}: {}",
                path,
                out.error.unwrap_or_default()
            ))
        }
    }

    fn download_to_file(&self, url: &str, dest: &str, strict_ssl: bool) -> miette::Result<()> {
        let input = Json(PluginHttpDownloadInput {
            url: url.to_string(),
            dest_path: dest.to_string(),
            strict_ssl: Some(strict_ssl),
        });
        let out = host_call!(appz_phttp_download, input)?;
        if out.success {
            Ok(())
        } else {
            Err(miette!(
                "Failed to download {} -> {}: {}",
                url,
                dest,
                out.error.unwrap_or_default()
            ))
        }
    }
}
