//! WASM VFS implementation using PDK host functions.
//!
//! Delegates filesystem and HTTP operations to appz_pfs_* and appz_phttp_download.

use appz_pdk::prelude::*;
use extism_pdk::*;

/// VFS abstraction for crawl plugin — read, write, mkdir, download.
pub trait CrawlVfs: Send + Sync {
    fn read_file(&self, path: &str) -> Result<String, String>;
    fn write_file(&self, path: &str, content: &str) -> Result<(), String>;
    #[allow(dead_code)] // reserved for cache checks
    fn exists(&self, path: &str) -> bool;
    fn mkdir(&self, path: &str) -> Result<(), String>;
    fn download(&self, url: &str, dest: &str, strict_ssl: bool) -> Result<(), String>;
}

/// VFS backed by host function calls.
pub struct WasmVfs;

macro_rules! host_call {
    ($fn:ident, $input:expr) => {{
        let j = unsafe { super::$fn($input) };
        #[allow(clippy::redundant_closure_call)]
        j.map(|x| x.0).map_err(|e: extism_pdk::Error| format!("host call failed: {}", e))
    }};
}

macro_rules! host_call_bool {
    ($fn:ident, $input:expr, $field:ident) => {{
        unsafe { super::$fn($input) }
            .map(|j| j.0.$field)
            .unwrap_or(false)
    }};
}

impl CrawlVfs for WasmVfs {
    fn read_file(&self, path: &str) -> Result<String, String> {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        let out = host_call!(appz_pfs_read_file, input)?;
        if out.success {
            Ok(out.content.unwrap_or_default())
        } else {
            Err(out.error.unwrap_or_else(|| "Read failed".to_string()))
        }
    }

    fn write_file(&self, path: &str, content: &str) -> Result<(), String> {
        let input = Json(PluginFsWriteInput {
            path: path.to_string(),
            content: content.to_string(),
        });
        let out = host_call!(appz_pfs_write_file, input)?;
        if out.success {
            Ok(())
        } else {
            Err(out.error.unwrap_or_else(|| "Write failed".to_string()))
        }
    }

    fn exists(&self, path: &str) -> bool {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        host_call_bool!(appz_pfs_exists, input, exists)
    }

    fn mkdir(&self, path: &str) -> Result<(), String> {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        let out = host_call!(appz_pfs_mkdir, input)?;
        if out.success {
            Ok(())
        } else {
            Err(out.error.unwrap_or_else(|| "Mkdir failed".to_string()))
        }
    }

    fn download(&self, url: &str, dest: &str, strict_ssl: bool) -> Result<(), String> {
        let input = Json(PluginHttpDownloadInput {
            url: url.to_string(),
            dest_path: dest.to_string(),
            strict_ssl: Some(strict_ssl),
        });
        let out = host_call!(appz_phttp_download, input)?;
        if out.success {
            Ok(())
        } else {
            Err(out.error.unwrap_or_else(|| "Download failed".to_string()))
        }
    }
}
