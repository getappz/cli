use starbase_utils::fs as starbase_fs;
use std::{fs, path::PathBuf};

use miette::miette;

use crate::shell::copy_path_recursive;
use common::consts::{K_DEPLOY_PATH, K_RELEASE_PATH, K_SHARED_DIRS, K_SHARED_FILES, SHARED_DIR};
use task::{Task, TaskRegistry};

pub fn register_shared(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "deploy:shared",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let deploy = ctx
                    .get(K_DEPLOY_PATH)
                    .ok_or_else(|| miette!("deploy_path not set"))?;
                let release = ctx
                    .get(K_RELEASE_PATH)
                    .ok_or_else(|| miette!("release_path not set"))?;
                let shared_root = PathBuf::from(deploy).join(SHARED_DIR);
                let release_root = PathBuf::from(&release);
                starbase_utils::fs::create_dir_all(&shared_root)
                    .map_err(|e| miette!("Failed to create shared directory: {}", e))?;
                // shared dirs
                if let Some(dirs) = ctx.get(K_SHARED_DIRS) {
                    for d in dirs.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        let src = shared_root.join(d);
                        let dst = release_root.join(d);
                        if !src.exists() {
                            starbase_fs::create_dir_all(&src).map_err(|e| {
                                miette!("Failed to create shared directory {}: {}", d, e)
                            })?;
                        }
                        if dst.exists() {
                            let _ = starbase_fs::remove_dir_all(&dst);
                        }
                        copy_path_recursive(&src, &dst)
                            .map_err(|e| miette!("Failed to copy shared directory {}: {}", d, e))?;
                    }
                }
                // shared files
                if let Some(files) = ctx.get(K_SHARED_FILES) {
                    for f in files.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        let src = shared_root.join(f);
                        if let Some(parent) = src.parent() {
                            starbase_fs::create_dir_all(parent).map_err(|e| {
                                miette!("Failed to create parent directory for {}: {}", f, e)
                            })?;
                        }
                        if !src.exists() {
                            starbase_fs::write_file(&src, "").map_err(|e| {
                                miette!("Failed to create shared file {}: {}", f, e)
                            })?;
                        }
                        let dst = release_root.join(f);
                        if let Some(parent) = dst.parent() {
                            starbase_fs::create_dir_all(parent).map_err(|e| {
                                miette!("Failed to create parent directory for {}: {}", f, e)
                            })?;
                        }
                        let _ = starbase_fs::remove_file(&dst);
                        fs::copy(&src, &dst)
                            .map_err(|e| miette!("Failed to copy shared file {}: {}", f, e))?;
                    }
                }
                Ok(())
            }),
        )
        .desc("Link shared"),
    );
}
