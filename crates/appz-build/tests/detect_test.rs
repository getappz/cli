//! Integration tests for framework detection.

use appz_build::detect_framework;
use std::path::Path;
use tempfile::TempDir;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[tokio::test]
async fn test_detect_none_for_empty_dir() {
    let tmp = TempDir::new().unwrap();
    let result = detect_framework(tmp.path()).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_detect_nextjs_from_package_json() {
    let fixtures = fixture("nextjs");
    if !fixtures.exists() {
        eprintln!("Skipping: fixture not found at {}", fixtures.display());
        return;
    }
    let result = detect_framework(&fixtures).await.unwrap();
    let detected = result.expect("Next.js should be detected");
    assert_eq!(detected.name, "Next.js");
    assert_eq!(detected.slug.as_deref(), Some("nextjs"));
    assert!(!detected.build_command.is_empty());
    assert!(!detected.install_command.is_empty());
}
