//! Environment variable and platform detection utilities.

use std::env;

/// Get an environment variable or return a default value
pub fn get_env_or_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Get an environment variable or return None
pub fn get_env_or_none(key: &str) -> Option<String> {
    env::var(key).ok()
}

/// Check if an environment variable is set
pub fn has_env(key: &str) -> bool {
    env::var(key).is_ok()
}

/// Check if running in CI environment
pub fn is_ci() -> bool {
    has_env("CI") || has_env("CONTINUOUS_INTEGRATION")
}

/// Check if running in a test environment
pub fn is_test() -> bool {
    has_env("TEST") || has_env("CARGO_PKG_NAME")
}

/// Get the current platform
pub fn platform() -> &'static str {
    #[cfg(target_os = "windows")]
    return "windows";

    #[cfg(target_os = "macos")]
    return "macos";

    #[cfg(target_os = "linux")]
    return "linux";

    #[cfg(target_arch = "wasm32")]
    return "wasm";

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
        target_arch = "wasm32"
    )))]
    return "unknown";
}

/// Check if running on Windows
#[inline]
pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Check if running on Unix-like systems
#[inline]
pub fn is_unix() -> bool {
    cfg!(unix)
}

/// Get the home directory
pub fn home_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir()
}

/// Get the cache directory for the application
pub fn cache_dir() -> Option<std::path::PathBuf> {
    dirs::cache_dir().map(|p| p.join(crate::consts::APP_NAME))
}

/// Get the config directory for the application
pub fn config_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join(crate::consts::CONFIG_DIRNAME))
}

/// Get the data directory for the application
pub fn data_dir() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|p| p.join(crate::consts::APP_NAME))
}
