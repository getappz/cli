//! Path-scoped filesystem operations — the sandbox security boundary.
//!
//! [`ScopedFs`] ensures that **every** filesystem path is resolved relative to
//! a sandbox root directory and validated before any I/O occurs. This prevents
//! path traversal attacks (e.g. `../../etc/passwd`) and accidental writes
//! outside the project.
//!
//! # Security guarantees
//!
//! - **Absolute paths** are rejected immediately (`PathEscape` error).
//! - **`..` traversal** that escapes the root is rejected.
//! - **Symlinks** are resolved via `canonicalize` — if the target falls outside
//!   the root, the operation is rejected.
//! - The root itself is canonicalized at construction time.
//!
//! # Performance
//!
//! Single-file operations (`read_to_string`, `write_file`, etc.) are
//! synchronous and straightforward. Batch operations (`read_files`,
//! `write_files`, `remove_files`, `copy_files`) use [`rayon`] thread-pool
//! parallelism to maximise throughput when dealing with hundreds or thousands
//! of files.
//!
//! Each batch method has a `_with_progress` variant that accepts an
//! `on_each: Option<F>` callback, invoked after each item completes. This
//! hook is used by [`crate::provider::SandboxProviderExt`] to drive progress
//! bar updates.
//!
//! # API surface (quick reference)
//!
//! | Category | Methods |
//! |----------|---------|
//! | Read | `read_to_string`, `read_bytes`, `read_files` |
//! | Write | `write_file`, `write_string`, `append_file`, `write_files` |
//! | Directory | `create_dir`, `create_dir_all`, `list_dir` |
//! | Remove | `remove_file`, `remove_dir`, `remove_dir_all`, `remove_files` |
//! | Copy/Move | `copy`, `copy_files`, `rename` |
//! | Query | `exists`, `is_file`, `is_dir` |
//! | Search | `glob` |
//!
//! All paths are **relative to the sandbox root**. Pass `"."` for the root
//! itself (e.g. `list_dir(".")`).
//!
//! # Example
//!
//! ```rust,no_run
//! use sandbox::ScopedFs;
//!
//! # fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let fs = ScopedFs::new("/tmp/my-project")?;
//!
//! fs.write_string("src/index.html", "<h1>Hello</h1>")?;
//! assert!(fs.is_file("src/index.html"));
//!
//! let matches = fs.glob("src/**/*.html")?;
//! assert_eq!(matches.len(), 1);
//! # Ok(())
//! # }
//! ```

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use starbase_utils::fs as starbase_fs;

use crate::error::{SandboxError, SandboxResult};

/// A filesystem handle scoped to a root directory.
///
/// Every operation resolves paths relative to `root` and validates that the
/// resulting absolute path stays within the sandbox boundary.
///
/// Optionally, [`read_allowed`](Self::read_allowed) paths permit reading from
/// directories outside the root (e.g. `~/.appz/skills`).
#[derive(Debug, Clone)]
pub struct ScopedFs {
    root: PathBuf,
    /// Canonicalized paths the sandbox may read from (in addition to root).
    read_allowed: Vec<PathBuf>,
}

/// A lightweight directory entry returned by [`ScopedFs::list_dir`].
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Path relative to the sandbox root.
    pub rel_path: PathBuf,
    /// Absolute path on disk.
    pub abs_path: PathBuf,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Whether this entry is a file.
    pub is_file: bool,
    /// The file name component.
    pub name: String,
}

impl ScopedFs {
    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    /// Create a new `ScopedFs` rooted at the given directory.
    ///
    /// The root path is canonicalized immediately so that all subsequent
    /// containment checks are reliable.
    pub fn new(root: impl Into<PathBuf>) -> SandboxResult<Self> {
        Self::new_with_allowed(root, Vec::<PathBuf>::new())
    }

    /// Create a new `ScopedFs` with optional read-allowed paths.
    ///
    /// `read_allowed` are directories outside the root that the sandbox may
    /// read from via [`read_allowed`](Self::read_allowed) and
    /// [`list_dir_allowed`](Self::list_dir_allowed).
    pub fn new_with_allowed(
        root: impl Into<PathBuf>,
        read_allowed: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> SandboxResult<Self> {
        let raw: PathBuf = root.into();
        let root = if raw.exists() {
            raw.canonicalize().map_err(SandboxError::Io)?
        } else {
            normalize_path(&raw)
        };
        let allowed: Vec<PathBuf> = read_allowed
            .into_iter()
            .map(Into::into)
            .filter_map(|p| {
                if p.exists() {
                    p.canonicalize().ok()
                } else {
                    Some(normalize_path(&p))
                }
            })
            .collect();
        Ok(Self {
            root,
            read_allowed: allowed,
        })
    }

    /// Return the canonical root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // ------------------------------------------------------------------
    // Path resolution (the security boundary)
    // ------------------------------------------------------------------

    /// Resolve a relative path to an absolute path inside the sandbox.
    ///
    /// If the path already exists on disk the resolution uses `canonicalize`
    /// (which resolves symlinks). For paths that don't exist yet, a logical
    /// normalisation is applied instead.
    ///
    /// Returns `Err(SandboxError::PathEscape)` if the resolved path falls
    /// outside the sandbox root.
    pub fn resolve(&self, rel: impl AsRef<Path>) -> SandboxResult<PathBuf> {
        let rel = rel.as_ref();

        if rel.is_absolute() {
            return Err(SandboxError::PathEscape {
                path: rel.display().to_string(),
            });
        }

        let joined = self.root.join(rel);
        let candidate = if joined.exists() {
            joined.canonicalize().map_err(SandboxError::Io)?
        } else {
            normalize_path(&joined)
        };

        if !candidate.starts_with(&self.root) {
            return Err(SandboxError::PathEscape {
                path: rel.display().to_string(),
            });
        }

        Ok(candidate)
    }

    /// Resolve a path that **must** already exist on disk.
    fn resolve_existing(&self, rel: impl AsRef<Path>) -> SandboxResult<PathBuf> {
        let resolved = self.resolve(&rel)?;
        if !resolved.exists() {
            return Err(SandboxError::FileNotFound {
                path: rel.as_ref().display().to_string(),
            });
        }
        Ok(resolved)
    }

    // ------------------------------------------------------------------
    // Read operations
    // ------------------------------------------------------------------

    /// Read a file's contents as a UTF-8 string.
    pub fn read_to_string(&self, rel_path: impl AsRef<Path>) -> SandboxResult<String> {
        let abs = self.resolve_existing(&rel_path)?;
        Ok(starbase_fs::read_file(&abs)?)
    }

    /// Read a file's contents as raw bytes.
    pub fn read_bytes(&self, rel_path: impl AsRef<Path>) -> SandboxResult<Vec<u8>> {
        let abs = self.resolve_existing(&rel_path)?;
        std::fs::read(&abs).map_err(SandboxError::Io)
    }

    /// Read multiple files in parallel.
    ///
    /// Returns a `Vec` of `(relative_path, Result<content>)` pairs in the
    /// same order as the input. Each file is read on a rayon worker thread.
    pub fn read_files<P: AsRef<Path> + Sync>(
        &self,
        rel_paths: &[P],
    ) -> Vec<(PathBuf, SandboxResult<String>)> {
        self.read_files_with_progress(rel_paths, None::<fn()>)
    }

    /// Read multiple files in parallel with an optional progress callback.
    ///
    /// `on_each` is called after every file is read (regardless of success or
    /// failure). Pass `None` to skip progress tracking.
    pub fn read_files_with_progress<P, F>(
        &self,
        rel_paths: &[P],
        on_each: Option<F>,
    ) -> Vec<(PathBuf, SandboxResult<String>)>
    where
        P: AsRef<Path> + Sync,
        F: Fn() + Send + Sync,
    {
        rel_paths
            .par_iter()
            .map(|rel| {
                let key = rel.as_ref().to_path_buf();
                let result = self.read_to_string(rel);
                if let Some(ref cb) = on_each {
                    cb();
                }
                (key, result)
            })
            .collect()
    }

    // ------------------------------------------------------------------
    // Write operations
    // ------------------------------------------------------------------

    /// Write `content` to a file, creating parent directories as needed.
    /// Overwrites the file if it already exists.
    pub fn write_file(
        &self,
        rel_path: impl AsRef<Path>,
        content: impl AsRef<[u8]>,
    ) -> SandboxResult<()> {
        let abs = self.resolve(&rel_path)?;
        if let Some(parent) = abs.parent() {
            starbase_fs::create_dir_all(parent)?;
        }
        starbase_fs::write_file(&abs, content)?;
        Ok(())
    }

    /// Write a UTF-8 string to a file.
    pub fn write_string(
        &self,
        rel_path: impl AsRef<Path>,
        content: &str,
    ) -> SandboxResult<()> {
        self.write_file(rel_path, content.as_bytes())
    }

    /// Write multiple files in parallel.
    ///
    /// Directories are created as needed. Returns a `Vec` of
    /// `(relative_path, Result<()>)` pairs preserving input order.
    pub fn write_files<P, C>(&self, items: &[(P, C)]) -> Vec<(PathBuf, SandboxResult<()>)>
    where
        P: AsRef<Path> + Sync,
        C: AsRef<[u8]> + Sync,
    {
        self.write_files_with_progress(items, None::<fn()>)
    }

    /// Write multiple files in parallel with an optional progress callback.
    ///
    /// `on_each` is called after every file write completes.
    pub fn write_files_with_progress<P, C, F>(
        &self,
        items: &[(P, C)],
        on_each: Option<F>,
    ) -> Vec<(PathBuf, SandboxResult<()>)>
    where
        P: AsRef<Path> + Sync,
        C: AsRef<[u8]> + Sync,
        F: Fn() + Send + Sync,
    {
        // Pre-create all unique parent directories serially (cheap, avoids
        // racing mkdir calls on the same directory).
        let mut parents = std::collections::HashSet::new();
        for (rel, _) in items {
            if let Ok(abs) = self.resolve(rel) {
                if let Some(parent) = abs.parent() {
                    parents.insert(parent.to_path_buf());
                }
            }
        }
        for parent in &parents {
            let _ = starbase_fs::create_dir_all(parent);
        }

        // Write files in parallel.
        items
            .par_iter()
            .map(|(rel, content)| {
                let key = rel.as_ref().to_path_buf();
                let result = (|| -> SandboxResult<()> {
                    let abs = self.resolve(rel)?;
                    starbase_fs::write_file(&abs, content)?;
                    Ok(())
                })();
                if let Some(ref cb) = on_each {
                    cb();
                }
                (key, result)
            })
            .collect()
    }

    /// Append `content` to an existing file (or create it).
    pub fn append_file(
        &self,
        rel_path: impl AsRef<Path>,
        content: impl AsRef<[u8]>,
    ) -> SandboxResult<()> {
        use std::io::Write;

        let abs = self.resolve(&rel_path)?;
        if let Some(parent) = abs.parent() {
            starbase_fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&abs)
            .map_err(SandboxError::Io)?;
        file.write_all(content.as_ref()).map_err(SandboxError::Io)
    }

    // ------------------------------------------------------------------
    // Directory operations
    // ------------------------------------------------------------------

    /// Create a single directory (parent must exist).
    pub fn create_dir(&self, rel_path: impl AsRef<Path>) -> SandboxResult<()> {
        let abs = self.resolve(&rel_path)?;
        std::fs::create_dir(&abs).map_err(SandboxError::Io)
    }

    /// Create a directory and all of its parents.
    pub fn create_dir_all(&self, rel_path: impl AsRef<Path>) -> SandboxResult<()> {
        let abs = self.resolve(&rel_path)?;
        starbase_fs::create_dir_all(&abs)?;
        Ok(())
    }

    /// List the entries in a directory.
    pub fn list_dir(&self, rel_path: impl AsRef<Path>) -> SandboxResult<Vec<DirEntry>> {
        let abs = self.resolve_existing(&rel_path)?;
        if !abs.is_dir() {
            return Err(SandboxError::DirectoryNotFound {
                path: rel_path.as_ref().display().to_string(),
            });
        }

        let rd = std::fs::read_dir(&abs).map_err(SandboxError::Io)?;
        // Collect raw entries first, then build DirEntry structs.
        let raw: Vec<_> = rd.collect::<Result<Vec<_>, _>>().map_err(SandboxError::Io)?;
        let mut entries = Vec::with_capacity(raw.len());
        for entry in raw {
            let abs_path = entry.path();
            let ft = entry.file_type().map_err(SandboxError::Io)?;
            let rel = abs_path
                .strip_prefix(&self.root)
                .unwrap_or(&abs_path)
                .to_path_buf();
            entries.push(DirEntry {
                rel_path: rel,
                abs_path,
                is_dir: ft.is_dir(),
                is_file: ft.is_file(),
                name: entry.file_name().to_string_lossy().into_owned(),
            });
        }
        Ok(entries)
    }

    // ------------------------------------------------------------------
    // Read-allowed operations (paths outside the sandbox root)
    // ------------------------------------------------------------------

    /// Check if an absolute path is under one of the read-allowed directories.
    pub fn is_path_allowed(&self, path: &Path) -> bool {
        let candidate = if path.exists() {
            path.canonicalize().ok()
        } else {
            Some(normalize_path(path))
        };
        let Some(candidate) = candidate else {
            return false;
        };
        self.read_allowed
            .iter()
            .any(|allowed| candidate.starts_with(allowed))
    }

    /// Read a file from a whitelisted path (outside the sandbox root).
    ///
    /// The path must be absolute and contained under one of the
    /// `read_allowed` directories configured at construction.
    pub fn read_allowed(&self, path: impl AsRef<Path>) -> SandboxResult<String> {
        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(SandboxError::PathEscape {
                path: path.display().to_string(),
            });
        }
        let abs = if path.exists() {
            path.canonicalize().map_err(SandboxError::Io)?
        } else {
            normalize_path(path)
        };
        if !self.is_path_allowed(&abs) {
            return Err(SandboxError::PathEscape {
                path: path.display().to_string(),
            });
        }
        if !abs.exists() {
            return Err(SandboxError::FileNotFound {
                path: path.display().to_string(),
            });
        }
        Ok(starbase_fs::read_file(&abs)?)
    }

    /// List directory entries from a whitelisted path (outside the sandbox root).
    ///
    /// The path must be absolute and contained under one of the
    /// `read_allowed` directories. Returns `DirEntry` with `rel_path` relative
    /// to the allowed directory.
    pub fn list_dir_allowed(&self, path: impl AsRef<Path>) -> SandboxResult<Vec<DirEntry>> {
        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(SandboxError::PathEscape {
                path: path.display().to_string(),
            });
        }
        let abs = if path.exists() {
            path.canonicalize().map_err(SandboxError::Io)?
        } else {
            return Err(SandboxError::DirectoryNotFound {
                path: path.display().to_string(),
            });
        };
        if !self.is_path_allowed(&abs) {
            return Err(SandboxError::PathEscape {
                path: path.display().to_string(),
            });
        }
        if !abs.is_dir() {
            return Err(SandboxError::DirectoryNotFound {
                path: path.display().to_string(),
            });
        }

        let rd = std::fs::read_dir(&abs).map_err(SandboxError::Io)?;
        let raw: Vec<_> = rd.collect::<Result<Vec<_>, _>>().map_err(SandboxError::Io)?;
        let mut entries = Vec::with_capacity(raw.len());
        for entry in raw {
            let abs_path = entry.path();
            let ft = entry.file_type().map_err(SandboxError::Io)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel_path = PathBuf::from(&name);
            entries.push(DirEntry {
                rel_path,
                abs_path,
                is_dir: ft.is_dir(),
                is_file: ft.is_file(),
                name,
            });
        }
        Ok(entries)
    }

    // ------------------------------------------------------------------
    // Remove operations
    // ------------------------------------------------------------------

    /// Remove a single file.
    pub fn remove_file(&self, rel_path: impl AsRef<Path>) -> SandboxResult<()> {
        let abs = self.resolve_existing(&rel_path)?;
        starbase_fs::remove_file(&abs)?;
        Ok(())
    }

    /// Remove multiple files in parallel.
    ///
    /// Returns a `Vec` of `(relative_path, Result<()>)` pairs preserving
    /// input order.
    pub fn remove_files<P: AsRef<Path> + Sync>(
        &self,
        rel_paths: &[P],
    ) -> Vec<(PathBuf, SandboxResult<()>)> {
        self.remove_files_with_progress(rel_paths, None::<fn()>)
    }

    /// Remove multiple files in parallel with an optional progress callback.
    ///
    /// `on_each` is called after every file removal completes.
    pub fn remove_files_with_progress<P, F>(
        &self,
        rel_paths: &[P],
        on_each: Option<F>,
    ) -> Vec<(PathBuf, SandboxResult<()>)>
    where
        P: AsRef<Path> + Sync,
        F: Fn() + Send + Sync,
    {
        rel_paths
            .par_iter()
            .map(|rel| {
                let key = rel.as_ref().to_path_buf();
                let result = self.remove_file(rel);
                if let Some(ref cb) = on_each {
                    cb();
                }
                (key, result)
            })
            .collect()
    }

    /// Remove an empty directory.
    pub fn remove_dir(&self, rel_path: impl AsRef<Path>) -> SandboxResult<()> {
        let abs = self.resolve_existing(&rel_path)?;
        std::fs::remove_dir(&abs).map_err(SandboxError::Io)
    }

    /// Remove a directory and all of its contents recursively.
    pub fn remove_dir_all(&self, rel_path: impl AsRef<Path>) -> SandboxResult<()> {
        let abs = self.resolve_existing(&rel_path)?;
        starbase_fs::remove_dir_all(&abs)?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Copy / Rename
    // ------------------------------------------------------------------

    /// Copy a file or directory tree from `from` to `to` (both relative).
    ///
    /// For directory trees the copy is parallelised with rayon: the directory
    /// structure is created first, then all files are copied concurrently.
    pub fn copy(
        &self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
    ) -> SandboxResult<()> {
        self.copy_with_progress(from, to, None::<fn()>)
    }

    /// Copy with an optional per-file progress callback.
    ///
    /// `on_each` is invoked once for each file copied (not directories).
    pub fn copy_with_progress<F>(
        &self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
        on_each: Option<F>,
    ) -> SandboxResult<()>
    where
        F: Fn() + Send + Sync,
    {
        let src = self.resolve_existing(&from)?;
        let dst = self.resolve(&to)?;
        copy_recursive_parallel(&src, &dst, on_each.as_ref())
    }

    /// Copy multiple (source, dest) pairs in parallel.
    ///
    /// Each pair is a `(from_rel, to_rel)`. Returns results preserving order.
    pub fn copy_files<P, Q>(&self, pairs: &[(P, Q)]) -> Vec<SandboxResult<()>>
    where
        P: AsRef<Path> + Sync,
        Q: AsRef<Path> + Sync,
    {
        self.copy_files_with_progress(pairs, None::<fn()>)
    }

    /// Copy multiple (source, dest) pairs in parallel with a progress callback.
    ///
    /// `on_each` is called after every pair copy completes.
    pub fn copy_files_with_progress<P, Q, F>(
        &self,
        pairs: &[(P, Q)],
        on_each: Option<F>,
    ) -> Vec<SandboxResult<()>>
    where
        P: AsRef<Path> + Sync,
        Q: AsRef<Path> + Sync,
        F: Fn() + Send + Sync,
    {
        pairs
            .par_iter()
            .map(|(from, to)| {
                let result = self.copy(from, to);
                if let Some(ref cb) = on_each {
                    cb();
                }
                result
            })
            .collect()
    }

    /// Rename / move a file or directory within the sandbox.
    pub fn rename(
        &self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
    ) -> SandboxResult<()> {
        let src = self.resolve_existing(&from)?;
        let dst = self.resolve(&to)?;
        if let Some(parent) = dst.parent() {
            starbase_fs::create_dir_all(parent)?;
        }
        std::fs::rename(&src, &dst).map_err(SandboxError::Io)
    }

    /// Copy a file or directory tree from an external absolute path into the sandbox.
    ///
    /// `src_abs` is an absolute path outside the sandbox (e.g. a temp dir).
    /// `dst_rel` is the destination path inside the sandbox (e.g. `"."` for root).
    ///
    /// For directory trees the copy is parallelised with rayon.
    pub fn copy_from_external(
        &self,
        src_abs: impl AsRef<Path>,
        dst_rel: impl AsRef<Path>,
    ) -> SandboxResult<()> {
        let src = src_abs.as_ref();
        let dst_rel = dst_rel.as_ref();

        if src.is_file() {
            let dst = self.resolve(dst_rel)?;
            if let Some(parent) = dst.parent() {
                starbase_fs::create_dir_all(parent)?;
            }
            std::fs::copy(src, dst).map_err(SandboxError::Io)?;
            return Ok(());
        }

        if !src.is_dir() {
            return Ok(());
        }

        // Ensure destination root exists (needed when source is flat, i.e. has no subdirs)
        let dst_root = self.resolve(dst_rel)?;
        starbase_fs::create_dir_all(&dst_root)?;

        let mut dirs = Vec::new();
        let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
        collect_tree_external(self, src, dst_rel, &mut dirs, &mut files)?;

        for dir in &dirs {
            starbase_fs::create_dir_all(dir)?;
        }

        let errors: Vec<SandboxError> = files
            .par_iter()
            .filter_map(|(from, to)| std::fs::copy(from, to).map_err(SandboxError::Io).err())
            .collect();

        if let Some(first_err) = errors.into_iter().next() {
            return Err(first_err);
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// Check whether a path exists inside the sandbox.
    pub fn exists(&self, rel_path: impl AsRef<Path>) -> bool {
        self.resolve(&rel_path)
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    /// Check whether the path points to a regular file.
    pub fn is_file(&self, rel_path: impl AsRef<Path>) -> bool {
        self.resolve(&rel_path)
            .map(|p| p.is_file())
            .unwrap_or(false)
    }

    /// Check whether the path points to a directory.
    pub fn is_dir(&self, rel_path: impl AsRef<Path>) -> bool {
        self.resolve(&rel_path)
            .map(|p| p.is_dir())
            .unwrap_or(false)
    }

    // ------------------------------------------------------------------
    // Glob
    // ------------------------------------------------------------------

    /// Find files matching a glob pattern within the sandbox.
    ///
    /// The pattern is relative to the sandbox root (e.g. `"src/**/*.ts"`).
    /// Returns paths relative to the sandbox root.
    pub fn glob(&self, pattern: &str) -> SandboxResult<Vec<PathBuf>> {
        let full_pattern = self.root.join(pattern).display().to_string();
        let iter = glob::glob(&full_pattern).map_err(|e| SandboxError::GlobError {
            reason: e.to_string(),
        })?;

        let mut results = Vec::with_capacity(128);
        for entry in iter {
            let path = entry.map_err(|e| SandboxError::GlobError {
                reason: e.to_string(),
            })?;
            if path.starts_with(&self.root) {
                let rel = path
                    .strip_prefix(&self.root)
                    .unwrap_or(&path)
                    .to_path_buf();
                results.push(rel);
            }
        }
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parallel recursive copy.
///
/// Strategy:
/// 1. Walk the source tree and collect all (src, dst) file pairs plus all
///    directories that need creating.
/// 2. Create directories sequentially (cheap, avoids races).
/// 3. Copy all files in parallel via rayon.
///
/// The optional `on_each` callback is invoked once per file copied.
fn copy_recursive_parallel<F>(src: &Path, dst: &Path, on_each: Option<&F>) -> SandboxResult<()>
where
    F: Fn() + Send + Sync,
{
    if src.is_file() {
        if let Some(parent) = dst.parent() {
            starbase_fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dst).map_err(SandboxError::Io)?;
        if let Some(cb) = on_each {
            cb();
        }
        return Ok(());
    }

    if !src.is_dir() {
        return Ok(());
    }

    // Phase 1: walk and collect.
    let mut dirs = Vec::new();
    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
    collect_tree(src, dst, &mut dirs, &mut files)?;

    // Phase 2: create directories (sequential — fast, order matters).
    for dir in &dirs {
        starbase_fs::create_dir_all(dir)?;
    }

    // Phase 3: copy files in parallel.
    let errors: Vec<SandboxError> = files
        .par_iter()
        .filter_map(|(from, to)| {
            let result = std::fs::copy(from, to).map_err(SandboxError::Io);
            if let Some(cb) = on_each {
                cb();
            }
            result.err()
        })
        .collect();

    if let Some(first_err) = errors.into_iter().next() {
        return Err(first_err);
    }

    Ok(())
}

/// Walk `src` recursively and collect directories to create under `dst`
/// and `(src_file, dst_file)` pairs to copy.
fn collect_tree(
    src: &Path,
    dst: &Path,
    dirs: &mut Vec<PathBuf>,
    files: &mut Vec<(PathBuf, PathBuf)>,
) -> SandboxResult<()> {
    dirs.push(dst.to_path_buf());
    for entry in std::fs::read_dir(src).map_err(SandboxError::Io)? {
        let entry = entry.map_err(SandboxError::Io)?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ft = entry.file_type().map_err(SandboxError::Io)?;
        if ft.is_dir() {
            collect_tree(&from, &to, dirs, files)?;
        } else {
            files.push((from, to));
        }
    }
    Ok(())
}

/// Walk an external `src` directory and collect directories/files to create
/// under the sandbox at `dst_rel`. Each destination path is resolved via
/// `scoped.resolve()` to enforce the sandbox boundary.
fn collect_tree_external(
    scoped: &ScopedFs,
    src: &Path,
    dst_rel: &Path,
    dirs: &mut Vec<PathBuf>,
    files: &mut Vec<(PathBuf, PathBuf)>,
) -> SandboxResult<()> {
    for entry in std::fs::read_dir(src).map_err(SandboxError::Io)? {
        let entry = entry.map_err(SandboxError::Io)?;
        let from = entry.path();
        let rel_suffix = from.strip_prefix(src).map_err(|_| {
            SandboxError::Other("template path outside source directory".to_string())
        })?;
        let dst_rel_full = if dst_rel == Path::new(".") {
            rel_suffix.to_path_buf()
        } else {
            dst_rel.join(rel_suffix)
        };
        let to = scoped.resolve(&dst_rel_full)?;
        let ft = entry.file_type().map_err(SandboxError::Io)?;
        if ft.is_dir() {
            dirs.push(to.clone());
            collect_tree_external(scoped, &from, &dst_rel_full, dirs, files)?;
        } else {
            files.push((from, to));
        }
    }
    Ok(())
}

/// Normalise a path without touching the filesystem (for non-existent paths).
///
/// Collapses `.` and `..` components logically.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                if components
                    .last()
                    .map(|c| matches!(c, std::path::Component::Normal(_)))
                    .unwrap_or(false)
                {
                    components.pop();
                } else {
                    components.push(comp);
                }
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rejects_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        let result = scoped.resolve("/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_rejects_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        let result = scoped.resolve("../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn write_and_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        scoped.write_string("hello.txt", "world").unwrap();
        let content = scoped.read_to_string("hello.txt").unwrap();
        assert_eq!(content, "world");
    }

    #[test]
    fn write_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        scoped
            .write_string("a/b/c/deep.txt", "deep content")
            .unwrap();
        assert!(scoped.is_file("a/b/c/deep.txt"));
    }

    #[test]
    fn list_dir_works() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        scoped.write_string("file1.txt", "one").unwrap();
        scoped.write_string("file2.txt", "two").unwrap();
        scoped.create_dir_all("subdir").unwrap();
        let entries = scoped.list_dir(".").unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn copy_file() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        scoped.write_string("src.txt", "data").unwrap();
        scoped.copy("src.txt", "dst.txt").unwrap();
        assert_eq!(scoped.read_to_string("dst.txt").unwrap(), "data");
    }

    #[test]
    fn copy_directory_parallel() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        // Create a tree with enough files to exercise parallelism.
        for i in 0..50 {
            scoped
                .write_string(&format!("src/sub{}/file{}.txt", i % 5, i), &format!("content-{}", i))
                .unwrap();
        }
        scoped.copy("src", "dst").unwrap();
        for i in 0..50 {
            let content = scoped
                .read_to_string(&format!("dst/sub{}/file{}.txt", i % 5, i))
                .unwrap();
            assert_eq!(content, format!("content-{}", i));
        }
    }

    #[test]
    fn remove_file_works() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        scoped.write_string("doomed.txt", "bye").unwrap();
        assert!(scoped.exists("doomed.txt"));
        scoped.remove_file("doomed.txt").unwrap();
        assert!(!scoped.exists("doomed.txt"));
    }

    #[test]
    fn glob_matches_files() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        scoped.write_string("a.txt", "").unwrap();
        scoped.write_string("b.txt", "").unwrap();
        scoped.write_string("c.rs", "").unwrap();
        let matches = scoped.glob("*.txt").unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn rename_moves_file() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        scoped.write_string("old.txt", "content").unwrap();
        scoped.rename("old.txt", "new.txt").unwrap();
        assert!(!scoped.exists("old.txt"));
        assert_eq!(scoped.read_to_string("new.txt").unwrap(), "content");
    }

    #[test]
    fn append_file_works() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();
        scoped.write_string("log.txt", "line1\n").unwrap();
        scoped.append_file("log.txt", b"line2\n").unwrap();
        let content = scoped.read_to_string("log.txt").unwrap();
        assert_eq!(content, "line1\nline2\n");
    }

    #[test]
    fn batch_write_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();

        let items: Vec<(String, String)> = (0..100)
            .map(|i| (format!("batch/file_{}.txt", i), format!("data-{}", i)))
            .collect();
        let write_results = scoped.write_files(&items);
        for (_, r) in &write_results {
            assert!(r.is_ok(), "batch write failed");
        }

        let paths: Vec<String> = items.iter().map(|(p, _)| p.clone()).collect();
        let read_results = scoped.read_files(&paths);
        for (i, (_, r)) in read_results.iter().enumerate() {
            assert_eq!(r.as_ref().unwrap(), &format!("data-{}", i));
        }
    }

    #[test]
    fn batch_remove() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();

        let paths: Vec<String> = (0..50)
            .map(|i| format!("rm/file_{}.txt", i))
            .collect();
        for p in &paths {
            scoped.write_string(p, "x").unwrap();
        }

        let results = scoped.remove_files(&paths);
        for (_, r) in &results {
            assert!(r.is_ok());
        }
        for p in &paths {
            assert!(!scoped.exists(p));
        }
    }

    #[test]
    fn batch_copy_files() {
        let dir = tempfile::tempdir().unwrap();
        let scoped = ScopedFs::new(dir.path()).unwrap();

        for i in 0..20 {
            scoped.write_string(&format!("orig/{}.txt", i), &format!("v{}", i)).unwrap();
        }
        let pairs: Vec<(String, String)> = (0..20)
            .map(|i| (format!("orig/{}.txt", i), format!("dup/{}.txt", i)))
            .collect();
        let results = scoped.copy_files(&pairs);
        for r in &results {
            assert!(r.is_ok());
        }
        for i in 0..20 {
            let c = scoped.read_to_string(&format!("dup/{}.txt", i)).unwrap();
            assert_eq!(c, format!("v{}", i));
        }
    }
}
