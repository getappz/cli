//! Native [`Vfs`] implementation using `std::fs`, `walkdir`, and `git2`.
//!
//! Available only when the `native` feature is enabled.

use crate::vfs::{FsEntry, Vfs};
use miette::{miette, Result};
use std::path::Path;

/// Native filesystem implementation.
pub struct NativeFs;

impl NativeFs {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NativeFs {
    fn default() -> Self {
        Self::new()
    }
}

impl Vfs for NativeFs {
    fn read_to_string(&self, path: &str) -> Result<String> {
        std::fs::read_to_string(path).map_err(|e| miette!("Failed to read {}: {}", path, e))
    }

    fn write_string(&self, path: &str, content: &str) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| miette!("Failed to create parent dir for {}: {}", path, e))?;
            }
        }
        std::fs::write(path, content).map_err(|e| miette!("Failed to write {}: {}", path, e))
    }

    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn is_file(&self, path: &str) -> bool {
        Path::new(path).is_file()
    }

    fn is_dir(&self, path: &str) -> bool {
        Path::new(path).is_dir()
    }

    fn create_dir_all(&self, path: &str) -> Result<()> {
        std::fs::create_dir_all(path)
            .map_err(|e| miette!("Failed to create directory {}: {}", path, e))
    }

    fn remove_file(&self, path: &str) -> Result<()> {
        std::fs::remove_file(path).map_err(|e| miette!("Failed to remove {}: {}", path, e))
    }

    fn remove_dir_all(&self, path: &str) -> Result<()> {
        std::fs::remove_dir_all(path)
            .map_err(|e| miette!("Failed to remove directory {}: {}", path, e))
    }

    fn copy_file(&self, src: &str, dst: &str) -> Result<()> {
        if let Some(parent) = Path::new(dst).parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| miette!("Failed to create parent dir for {}: {}", dst, e))?;
            }
        }
        std::fs::copy(src, dst)
            .map(|_| ())
            .map_err(|e| miette!("Failed to copy {} -> {}: {}", src, dst, e))
    }

    fn copy_dir(&self, src: &str, dst: &str) -> Result<()> {
        self.create_dir_all(dst)?;
        for entry in walkdir::WalkDir::new(src) {
            let entry = entry.map_err(|e| miette!("Walk error in {}: {}", src, e))?;
            let rel = entry
                .path()
                .strip_prefix(src)
                .map_err(|_| miette!("Path prefix error"))?;
            let target = Path::new(dst).join(rel);
            let target_str = target.to_string_lossy();
            if entry.path().is_dir() {
                self.create_dir_all(&target_str)?;
            } else {
                self.copy_file(
                    &entry.path().to_string_lossy(),
                    &target_str,
                )?;
            }
        }
        Ok(())
    }

    fn walk_dir(&self, path: &str) -> Result<Vec<FsEntry>> {
        let mut entries = Vec::new();
        for entry in walkdir::WalkDir::new(path) {
            let entry = entry.map_err(|e| miette!("Walk error in {}: {}", path, e))?;
            entries.push(FsEntry {
                path: entry.path().to_string_lossy().replace('\\', "/"),
                is_file: entry.path().is_file(),
                is_dir: entry.path().is_dir(),
            });
        }
        Ok(entries)
    }

    fn list_dir(&self, path: &str) -> Result<Vec<FsEntry>> {
        let mut entries = Vec::new();
        for entry in
            std::fs::read_dir(path).map_err(|e| miette!("Failed to read dir {}: {}", path, e))?
        {
            let entry = entry.map_err(|e| miette!("Failed to read entry: {}", e))?;
            let p = entry.path();
            entries.push(FsEntry {
                path: p.to_string_lossy().replace('\\', "/"),
                is_file: p.is_file(),
                is_dir: p.is_dir(),
            });
        }
        Ok(entries)
    }

    fn git_changed_files(&self, repo_path: &str) -> Result<Vec<String>> {
        use git2::{Repository, StatusOptions};
        use std::collections::HashSet;

        let path = Path::new(repo_path);
        let repo = Repository::discover(path)
            .or_else(|_| Repository::open(path))
            .map_err(|e| miette!("Not a git repository: {}", e))?;

        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.include_ignored(false);

        let statuses = repo
            .statuses(Some(&mut opts))
            .map_err(|e| miette!("Failed to get git status: {}", e))?;

        let mut paths = HashSet::new();
        for entry in statuses.iter() {
            if let Some(p) = entry.path() {
                paths.insert(p.to_string());
            }
        }

        let mut result: Vec<String> = paths.into_iter().collect();
        result.sort();
        Ok(result)
    }

    fn git_staged_files(&self, repo_path: &str) -> Result<Vec<String>> {
        use git2::{Repository, Status, StatusOptions};

        let path = Path::new(repo_path);
        let repo = Repository::discover(path)
            .or_else(|_| Repository::open(path))
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
                if let Some(p) = entry.path() {
                    paths.push(p.to_string());
                }
            }
        }

        paths.sort();
        Ok(paths)
    }

    fn git_is_repo(&self, path: &str) -> bool {
        use git2::Repository;
        let p = Path::new(path);
        Repository::open(p).is_ok() || Repository::discover(p).is_ok()
    }
}
