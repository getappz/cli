//! Version command and platform detection utilities.

use std::sync::LazyLock as Lazy;

/// Operating system name (mise-compatible format)
pub static OS: Lazy<String> = Lazy::new(|| std::env::consts::OS.into());

/// Architecture name (mise-compatible format)
pub static ARCH: Lazy<String> = Lazy::new(|| {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => std::env::consts::ARCH,
    }
    .to_string()
});
