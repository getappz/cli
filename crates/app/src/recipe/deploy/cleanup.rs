use crate::log::info;
use common::consts::{K_DEPLOY_PATH, K_KEEP_RELEASES, RELEASES_DIR};
use miette::miette;
use starbase_utils::fs as starbase_fs;
use std::{fs, path::PathBuf};
use task::{Task, TaskRegistry};
pub fn register_cleanup(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "deploy:cleanup",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let deploy = ctx
                    .get(K_DEPLOY_PATH)
                    .ok_or_else(|| miette!("deploy_path not set"))?;
                let releases_path = PathBuf::from(deploy).join(RELEASES_DIR);
                let keep: usize = ctx
                    .get(K_KEEP_RELEASES)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10);
                if !releases_path.exists() {
                    return Ok(());
                }
                let mut entries: Vec<_> = fs::read_dir(&releases_path)
                    .map_err(|e| miette!("Failed to read releases directory: {}", e))?
                    .filter_map(|e| e.ok())
                    .collect();
                entries.sort_by_key(|e| e.file_name());
                if entries.len() <= keep {
                    return Ok(());
                }
                let to_delete = entries.len() - keep;
                for e in entries.into_iter().take(to_delete) {
                    let p = releases_path.join(e.file_name());
                    let _ = starbase_fs::remove_dir_all(&p);
                }
                info("Cleaned up old releases");
                Ok(())
            }),
        )
        .desc("Cleanup old releases"),
    );
}
