use starbase_utils::fs as starbase_fs;
use std::path::PathBuf;

use miette::miette;

use crate::log::info;
use common::consts::{K_CURRENT_PATH, K_RELEASE_PATH};
use task::{Task, TaskRegistry};

#[cfg(target_os = "windows")]
fn replace_symlink(current: &PathBuf, release: &PathBuf) -> std::io::Result<()> {
    // Best-effort: remove current and copy tree (symlink may require elevation)
    if current.exists() {
        let _ = starbase_fs::remove_dir_all(current);
    }
    crate::shell::copy_path_recursive(release, current).map_err(std::io::Error::other)
}

#[cfg(not(target_os = "windows"))]
fn replace_symlink(current: &PathBuf, release: &PathBuf) -> std::io::Result<()> {
    use std::os::unix::fs as unixfs;
    if current.exists() {
        let _ = starbase_fs::remove_file(current);
        let _ = starbase_fs::remove_dir_all(current);
    }
    unixfs::symlink(release, current)
}

pub fn register_symlink(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "deploy:symlink",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let current = ctx
                    .get(K_CURRENT_PATH)
                    .unwrap_or("{{deploy_path}}/current".to_string());
                let current_path = PathBuf::from(ctx.parse(&current));
                let release = ctx
                    .get(K_RELEASE_PATH)
                    .ok_or_else(|| miette!("release_path not set"))?;
                let release_path = PathBuf::from(release);
                replace_symlink(&current_path, &release_path)
                    .map_err(|e| miette!("Failed to create symlink: {}", e))?;
                info(&format!("Published release to {:?}", current_path));
                Ok(())
            }),
        )
        .desc("Symlink new release"),
    );
}
