//! WASM [`Vfs`] implementation using PDK host functions.
//!
//! Delegates all filesystem and git operations to the appz CLI host
//! through the Extism host function ABI.

use appz_pdk::prelude::*;
use extism_pdk::*;
use miette::miette;
use ssg_migrator::vfs::{FsEntry, Vfs};

/// Vfs implementation backed by PDK host function calls.
pub struct WasmFs;

// Helper: call a host function and convert its Extism error to miette.
macro_rules! host_call {
    ($fn:ident, $input:expr) => {
        unsafe { super::$fn($input) }.map(|j| j.0).map_err(|e| miette!("{}", e))
    };
}

// Helper: call a host function that returns a bool-ish output, defaulting on error.
macro_rules! host_call_bool {
    ($fn:ident, $input:expr, $field:ident) => {
        unsafe { super::$fn($input) }
            .map(|j| j.0.$field)
            .unwrap_or(false)
    };
}

impl Vfs for WasmFs {
    fn read_to_string(&self, path: &str) -> miette::Result<String> {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        let out = host_call!(appz_pfs_read_file, input)?;
        if out.success {
            Ok(out.content.unwrap_or_default())
        } else {
            Err(miette!(
                "Failed to read {}: {}",
                path,
                out.error.unwrap_or_default()
            ))
        }
    }

    fn write_string(&self, path: &str, content: &str) -> miette::Result<()> {
        let input = Json(PluginFsWriteInput {
            path: path.to_string(),
            content: content.to_string(),
        });
        let out = host_call!(appz_pfs_write_file, input)?;
        if out.success {
            Ok(())
        } else {
            Err(miette!(
                "Failed to write {}: {}",
                path,
                out.error.unwrap_or_default()
            ))
        }
    }

    fn exists(&self, path: &str) -> bool {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        host_call_bool!(appz_pfs_exists, input, exists)
    }

    fn is_file(&self, path: &str) -> bool {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        host_call_bool!(appz_pfs_is_file, input, exists)
    }

    fn is_dir(&self, path: &str) -> bool {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        host_call_bool!(appz_pfs_is_dir, input, exists)
    }

    fn create_dir_all(&self, path: &str) -> miette::Result<()> {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        let out = host_call!(appz_pfs_mkdir, input)?;
        if out.success {
            Ok(())
        } else {
            Err(miette!(
                "Failed to mkdir {}: {}",
                path,
                out.error.unwrap_or_default()
            ))
        }
    }

    fn remove_file(&self, path: &str) -> miette::Result<()> {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        let out = host_call!(appz_pfs_remove, input)?;
        if out.success {
            Ok(())
        } else {
            Err(miette!(
                "Failed to remove {}: {}",
                path,
                out.error.unwrap_or_default()
            ))
        }
    }

    fn remove_dir_all(&self, path: &str) -> miette::Result<()> {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        let out = host_call!(appz_pfs_remove, input)?;
        if out.success {
            Ok(())
        } else {
            Err(miette!(
                "Failed to remove dir {}: {}",
                path,
                out.error.unwrap_or_default()
            ))
        }
    }

    fn copy_file(&self, src: &str, dst: &str) -> miette::Result<()> {
        let input = Json(PluginFsCopyInput {
            source: src.to_string(),
            destination: dst.to_string(),
        });
        let out = host_call!(appz_pfs_copy, input)?;
        if out.success {
            Ok(())
        } else {
            Err(miette!(
                "Failed to copy {} -> {}: {}",
                src,
                dst,
                out.error.unwrap_or_default()
            ))
        }
    }

    fn copy_dir(&self, src: &str, dst: &str) -> miette::Result<()> {
        self.create_dir_all(dst)?;
        let entries = self.walk_dir(src)?;
        for entry in &entries {
            if let Some(rel) = entry.path.strip_prefix(src) {
                let rel = rel.trim_start_matches('/');
                if rel.is_empty() {
                    continue;
                }
                let target = format!("{}/{}", dst, rel);
                if entry.is_dir {
                    self.create_dir_all(&target)?;
                } else if entry.is_file {
                    self.copy_file(&entry.path, &target)?;
                }
            }
        }
        Ok(())
    }

    fn walk_dir(&self, path: &str) -> miette::Result<Vec<FsEntry>> {
        let input = Json(PluginFsWalkInput {
            path: path.to_string(),
            glob: None,
        });
        let out = host_call!(appz_pfs_walk_dir, input)?;
        if !out.success {
            return Err(miette!(
                "Failed to walk {}: {}",
                path,
                out.error.unwrap_or_default()
            ));
        }
        Ok(out
            .paths
            .into_iter()
            .map(|p| {
                let is_dir = self.is_dir(&p);
                FsEntry {
                    path: p,
                    is_file: !is_dir,
                    is_dir,
                }
            })
            .collect())
    }

    fn list_dir(&self, path: &str) -> miette::Result<Vec<FsEntry>> {
        // The list_dir host function returns PluginFsWriteOutput without paths,
        // so we implement list_dir by walking and filtering to immediate children.
        let all = self.walk_dir(path)?;
        let prefix = if path.ends_with('/') {
            path.to_string()
        } else {
            format!("{}/", path)
        };
        Ok(all
            .into_iter()
            .filter(|e| {
                if let Some(rel) = e.path.strip_prefix(&prefix) {
                    !rel.is_empty() && !rel.contains('/')
                } else {
                    false
                }
            })
            .collect())
    }

    fn git_changed_files(&self, repo_path: &str) -> miette::Result<Vec<String>> {
        let input = Json(PluginFsReadInput {
            path: repo_path.to_string(),
        });
        let out = host_call!(appz_pgit_changed_files, input)?;
        if out.success {
            Ok(out.files)
        } else {
            Err(miette!(
                "Git changed files failed: {}",
                out.error.unwrap_or_default()
            ))
        }
    }

    fn git_staged_files(&self, repo_path: &str) -> miette::Result<Vec<String>> {
        let input = Json(PluginFsReadInput {
            path: repo_path.to_string(),
        });
        let out = host_call!(appz_pgit_staged_files, input)?;
        if out.success {
            Ok(out.files)
        } else {
            Err(miette!(
                "Git staged files failed: {}",
                out.error.unwrap_or_default()
            ))
        }
    }

    fn git_is_repo(&self, path: &str) -> bool {
        let input = Json(PluginFsReadInput {
            path: path.to_string(),
        });
        host_call_bool!(appz_pgit_is_repo, input, is_repo)
    }
}
