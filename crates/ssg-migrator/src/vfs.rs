//! Virtual filesystem trait for the SSG migrator.
//!
//! Abstracts all filesystem and git operations so the same migration logic
//! can run natively (std::fs + walkdir + git2) or inside a WASM plugin
//! (via PDK host functions provided by the appz CLI).

use miette::Result;

/// A single entry returned by [`Vfs::walk_dir`].
#[derive(Debug, Clone)]
pub struct FsEntry {
    /// Full path of the entry (uses `/` separators).
    pub path: String,
    pub is_file: bool,
    pub is_dir: bool,
}

/// Virtual filesystem + git abstraction.
///
/// Implementations:
/// - [`NativeFs`](crate::vfs_native::NativeFs) — wraps `std::fs`, `walkdir`, `git2`.
/// - The WASM plugin provides its own impl backed by PDK host functions.
pub trait Vfs {
    // ── File I/O ────────────────────────────────────────────────────────

    fn read_to_string(&self, path: &str) -> Result<String>;
    fn write_string(&self, path: &str, content: &str) -> Result<()>;

    // ── Queries ─────────────────────────────────────────────────────────

    fn exists(&self, path: &str) -> bool;
    fn is_file(&self, path: &str) -> bool;
    fn is_dir(&self, path: &str) -> bool;

    // ── Directory operations ────────────────────────────────────────────

    fn create_dir_all(&self, path: &str) -> Result<()>;
    fn remove_file(&self, path: &str) -> Result<()>;
    fn remove_dir_all(&self, path: &str) -> Result<()>;

    // ── Copy ────────────────────────────────────────────────────────────

    fn copy_file(&self, src: &str, dst: &str) -> Result<()>;
    fn copy_dir(&self, src: &str, dst: &str) -> Result<()>;

    // ── Traversal ───────────────────────────────────────────────────────

    /// Recursively walk a directory tree.
    fn walk_dir(&self, path: &str) -> Result<Vec<FsEntry>>;

    /// List immediate children of a directory (non-recursive).
    fn list_dir(&self, path: &str) -> Result<Vec<FsEntry>>;

    // ── Git ─────────────────────────────────────────────────────────────

    fn git_changed_files(&self, repo_path: &str) -> Result<Vec<String>>;
    fn git_staged_files(&self, repo_path: &str) -> Result<Vec<String>>;
    fn git_is_repo(&self, path: &str) -> bool;
}
