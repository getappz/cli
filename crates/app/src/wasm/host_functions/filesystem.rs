use extism::{convert::Json, host_fn};
use std::fs;
use std::path::Path;

use crate::shell::copy_path_recursive;
use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

// Helper for file transfer results
fn transfer_result(
    success: bool,
    message: Option<String>,
    error: Option<String>,
) -> Json<FileTransferResult> {
    Json(FileTransferResult {
        success,
        message,
        error,
    })
}

fn transfer_error(msg: impl Into<String>) -> Json<FileTransferResult> {
    transfer_result(false, None, Some(msg.into()))
}

fn transfer_success(msg: impl Into<String>) -> Json<FileTransferResult> {
    transfer_result(true, Some(msg.into()), None)
}

// ============================================================================
// Upload Files/Directories
// ============================================================================

host_fn!(pub appz_fs_upload(
    _user_data: PluginHostData;
    args: Json<UploadInput>
) -> Json<FileTransferResult> {
    let input = args.into_inner();

    // For local execution, upload is just a copy operation
    let src = Path::new(&input.source);
    let dst = Path::new(&input.destination);

    if !src.exists() {
        return Ok(transfer_error(format!("Source path '{}' does not exist", input.source)));
    }

    // Ensure destination directory exists
    if let Some(parent) = dst.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return Ok(transfer_error(format!("Failed to create destination directory: {}", e)));
        }
    }

    match copy_path_recursive(src, dst) {
        Ok(_) => Ok(transfer_success(format!("Uploaded '{}' to '{}'", input.source, input.destination))),
        Err(e) => Ok(transfer_error(format!("Upload failed: {}", e))),
    }
});

// ============================================================================
// Download Files/Directories
// ============================================================================

host_fn!(pub appz_fs_download(
    _user_data: PluginHostData;
    args: Json<DownloadInput>
) -> Json<FileTransferResult> {
    let input = args.into_inner();

    // For local execution, download is just a copy operation
    let src = Path::new(&input.source);
    let dst = Path::new(&input.destination);

    if !src.exists() {
        return Ok(transfer_error(format!("Source path '{}' does not exist", input.source)));
    }

    // Ensure destination directory exists
    if let Some(parent) = dst.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return Ok(transfer_error(format!("Failed to create destination directory: {}", e)));
        }
    }

    match copy_path_recursive(src, dst) {
        Ok(_) => Ok(transfer_success(format!("Downloaded '{}' to '{}'", input.source, input.destination))),
        Err(e) => Ok(transfer_error(format!("Download failed: {}", e))),
    }
});
