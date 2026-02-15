//! Git change detection via git2 (in-process, no shelling out).

use git2::{Repository, Status, StatusOptions};
use miette::{miette, Result};
use std::collections::HashSet;
use std::path::Path;

/// Returns paths of files changed since last commit (working tree vs HEAD).
/// Includes unstaged and staged changes.
pub fn changed_files(repo_path: impl AsRef<Path>) -> Result<Vec<String>> {
    let path = repo_path.as_ref();
    let repo = Repository::discover(path).or_else(|_| Repository::open(path))
        .map_err(|e| miette!("Not a git repository: {}", e))?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    opts.include_ignored(false);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| miette!("Failed to get git status: {}", e))?;

    let mut paths = HashSet::new();
    for entry in statuses.iter() {
        if let Some(path) = entry.path() {
            paths.insert(path.to_string());
        }
    }

    let mut result: Vec<String> = paths.into_iter().collect();
    result.sort();
    Ok(result)
}

/// Returns staged file paths (index vs HEAD).
pub fn staged_files(repo_path: impl AsRef<Path>) -> Result<Vec<String>> {
    let path = repo_path.as_ref();
    let repo = Repository::discover(path).or_else(|_| Repository::open(path))
        .map_err(|e| miette!("Not a git repository: {}", e))?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(false);
    opts.include_ignored(false);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| miette!("Failed to get git status: {}", e))?;

    let index_flags = Status::INDEX_NEW
        | Status::INDEX_MODIFIED
        | Status::INDEX_DELETED
        | Status::INDEX_RENAMED
        | Status::INDEX_TYPECHANGE;

    let mut paths = Vec::new();
    for entry in statuses.iter() {
        if entry.status().intersects(index_flags) {
            if let Some(path) = entry.path() {
                paths.push(path.to_string());
            }
        }
    }

    paths.sort();
    Ok(paths)
}

/// Check if path is inside a git repo.
pub fn is_git_repo(path: impl AsRef<Path>) -> bool {
    let p = path.as_ref();
    Repository::open(p).is_ok() || Repository::discover(p).is_ok()
}
