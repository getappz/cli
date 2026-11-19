//! Embeds the build commit so `appz --version` reports `<version> (<sha>)`.
use std::process::Command;

fn main() {
    let sha = std::env::var("APPZ_BUILD_SHA")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(9).collect::<String>())
        .or_else(|| {
            let out = Command::new("git")
                .args(["rev-parse", "--short=9", "HEAD"])
                .output()
                .ok()?;
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=APPZ_BUILD_SHA={sha}");
    println!("cargo:rerun-if-env-changed=APPZ_BUILD_SHA");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}
