//! E2E-style tests for `appz open` command.
//!
//! Tests the open flow: requires linked project, fetches project/team, builds dashboard URL.
//! Run with: cargo test -p cli open_e2e

use std::process::Command;
use tempfile::tempdir;

/// Run `appz open` in the given directory and return (stdout, stderr, exit_code).
fn run_appz_open(cwd: &std::path::Path) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_appz"))
        .arg("open")
        .current_dir(cwd)
        .output()
        .expect("failed to run appz open");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

#[test]
fn test_open_requires_linked_project() {
    // Run in a temp dir with no .appz/project.json
    let temp = tempdir().expect("tempdir");
    let cwd = temp.path();

    let (_stdout, stderr, code) = run_appz_open(cwd);

    assert!(
        code != 0,
        "appz open should fail when not linked, got exit code {}",
        code
    );
    assert!(
        stderr.contains("No linked project") || stderr.contains("link"),
        "stderr should mention linking: {}",
        stderr
    );
}

#[test]
fn test_open_help_succeeds() {
    // Smoke test: --help works
    let output = Command::new(env!("CARGO_BIN_EXE_appz"))
        .args(["open", "--help"])
        .output()
        .expect("failed to run appz open --help");

    assert!(output.status.success(), "help should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("open") || stdout.contains("Open"));
}
