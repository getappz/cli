//! File system utilities.
//!
//! Use `starbase_utils::fs` for general file operations (read_file, write_file, create_dir_all,
//! etc.). Use `std::fs` only for operations starbase_utils does not expose, such as setting
//! executable bits via `PermissionsExt`.

pub use starbase_utils::fs;

use miette::Result;
use std::path::Path;

/// Make a file executable (Unix only, no-op on Windows)
///
/// Uses `std::fs` intentionally: starbase_utils does not expose `PermissionsExt` for setting
/// executable bits on Unix.
#[cfg(unix)]
pub fn make_executable<P: AsRef<Path>>(path: P) -> Result<()> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let path = path.as_ref();
    let mut perms = fs::metadata(path)
        .map_err(|e| miette::miette!("Failed to get file metadata: {}", e))?
        .permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(path, perms)
        .map_err(|e| miette::miette!("Failed to set executable permissions: {}", e))?;
    Ok(())
}

/// Make a file executable (no-op on Windows)
#[cfg(windows)]
pub fn make_executable<P: AsRef<Path>>(_path: P) -> Result<()> {
    // Windows doesn't have executable permissions in the same way
    Ok(())
}

/// Make a file executable (async version, Unix only)
///
/// Uses `tokio::fs` intentionally: starbase_utils does not expose async set_permissions with
/// `PermissionsExt` for executable bits.
#[cfg(unix)]
pub async fn make_executable_async<P: AsRef<Path>>(path: P) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let path = path.as_ref();
    let mut perms = tokio::fs::metadata(path)
        .await
        .map_err(|e| miette::miette!("Failed to get file metadata: {}", e))?
        .permissions();
    perms.set_mode(perms.mode() | 0o111);
    tokio::fs::set_permissions(path, perms)
        .await
        .map_err(|e| miette::miette!("Failed to set executable permissions: {}", e))?;
    Ok(())
}

/// Make a file executable (async version, no-op on Windows)
#[cfg(windows)]
pub async fn make_executable_async<P: AsRef<Path>>(_path: P) -> Result<()> {
    // Windows doesn't have executable permissions in the same way
    Ok(())
}
