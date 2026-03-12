//! E2E-style tests for environments (pull, env pull).
//!
//! Run with: cargo test -p cli environments_e2e

use std::process::Command;
use tempfile::tempdir;

fn run_appz(args: &[&str], cwd: &std::path::Path) -> (String, String, i32) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_appz"));
    cmd.args(args).current_dir(cwd);
    let output = cmd.output().expect("failed to run appz");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

#[test]
fn test_pull_requires_linked_project() {
    let temp = tempdir().expect("tempdir");
    let (_out, stderr, code) = run_appz(&["pull", "-y"], temp.path());
    assert!(code != 0, "pull should fail when not linked");
    assert!(
        stderr.contains("link") || stderr.contains("Linked") || stderr.contains("linked"),
        "stderr should mention linking: {}",
        stderr
    );
}

#[test]
fn test_pull_help_shows_environment() {
    let output = Command::new(env!("CARGO_BIN_EXE_appz"))
        .args(["pull", "--help"])
        .output()
        .expect("failed to run appz pull --help");
    assert!(output.status.success(), "pull --help should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("environment") || stdout.contains("--environment"),
        "help should show environment: {}",
        stdout
    );
}

#[test]
fn test_pull_rejects_invalid_environment() {
    let temp = tempdir().expect("tempdir");
    let (_out, stderr, code) = run_appz(&["pull", "--environment=staging", "-y"], temp.path());
    assert!(code != 0, "pull should reject invalid environment");
    assert!(
        stderr.contains("development") || stderr.contains("preview") || stderr.contains("production"),
        "stderr should list valid environments: {}",
        stderr
    );
}

#[test]
fn test_env_pull_help_shows_target() {
    let output = Command::new(env!("CARGO_BIN_EXE_appz"))
        .args(["env", "pull", "--help"])
        .output()
        .expect("failed to run appz env pull --help");
    assert!(output.status.success(), "env pull --help should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("target") || stdout.contains("environment"),
        "help should show target/environment: {}",
        stdout
    );
}
