//! Git integration for scoped file checking.
//!
//! Provides file lists for `--changed` and `--staged` modes, enabling
//! incremental checking of only modified files.

use crate::error::{CheckResult, CheckerError};

/// Get the list of files changed since the last commit (unstaged + staged).
///
/// Uses `git diff --name-only HEAD` to find modified files.
pub async fn changed_files(
    sandbox: &dyn sandbox::SandboxProvider,
) -> CheckResult<Vec<String>> {
    let output = sandbox.exec("git diff --name-only HEAD").await.map_err(|e| {
        CheckerError::GitError {
            reason: format!("Failed to run git diff: {}", e),
        }
    })?;

    if !output.success() {
        // If HEAD doesn't exist (new repo), try without HEAD.
        let output = sandbox
            .exec("git diff --name-only")
            .await
            .map_err(|e| CheckerError::GitError {
                reason: format!("Failed to run git diff: {}", e),
            })?;

        return Ok(parse_file_list(&output.stdout()));
    }

    // Also include untracked files.
    let untracked = sandbox
        .exec("git ls-files --others --exclude-standard")
        .await
        .map_err(|e| CheckerError::GitError {
            reason: format!("Failed to list untracked files: {}", e),
        })?;

    let mut files = parse_file_list(&output.stdout());
    files.extend(parse_file_list(&untracked.stdout()));
    files.sort();
    files.dedup();

    Ok(files)
}

/// Get the list of staged files (git index).
///
/// Uses `git diff --cached --name-only` to find staged files.
pub async fn staged_files(
    sandbox: &dyn sandbox::SandboxProvider,
) -> CheckResult<Vec<String>> {
    let output = sandbox
        .exec("git diff --cached --name-only")
        .await
        .map_err(|e| CheckerError::GitError {
            reason: format!("Failed to run git diff --cached: {}", e),
        })?;

    if !output.success() {
        return Err(CheckerError::GitError {
            reason: format!("git diff --cached failed: {}", output.stderr()),
        });
    }

    Ok(parse_file_list(&output.stdout()))
}

/// Check if the project directory is a git repository.
pub async fn is_git_repo(sandbox: &dyn sandbox::SandboxProvider) -> bool {
    sandbox
        .exec("git rev-parse --is-inside-work-tree")
        .await
        .map(|o| o.success())
        .unwrap_or(false)
}

/// Parse a newline-separated file list from git output.
fn parse_file_list(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}
