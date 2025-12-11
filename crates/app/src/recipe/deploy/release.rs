use starbase_utils::fs as starbase_fs;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use miette::miette;

use crate::log::info;
use common::consts::{K_DEPLOY_PATH, RELEASES_DIR};
use task::{Context, Task, TaskRegistry};

fn resolve_deploy_path(ctx: &Context) -> miette::Result<PathBuf> {
    let p = ctx
        .get(K_DEPLOY_PATH)
        .ok_or_else(|| miette!("deploy_path is required"))?;
    Ok(PathBuf::from(p))
}

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", secs)
}

pub fn register_release(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "deploy:release",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let deploy_path = resolve_deploy_path(&ctx)?;
                let releases_path = deploy_path.join(RELEASES_DIR);
                starbase_fs::create_dir_all(&releases_path)
                    .map_err(|e| miette!("Failed to create releases directory: {}", e))?;

                // previous_release if any (latest dir)
                let mut entries: Vec<_> = fs::read_dir(&releases_path)
                    .map_err(|e| miette!("Failed to read releases directory: {}", e))?
                    .filter_map(|e| e.ok())
                    .collect();
                entries.sort_by_key(|e| e.file_name());
                // Note: Context mutation via Arc requires interior mutability
                // TODO: Handle context mutations properly
                let _previous_release = entries.last().map(|last| {
                    releases_path
                        .join(last.file_name())
                        .to_string_lossy()
                        .to_string()
                });

                // create new release dir
                let name = timestamp();
                let release_path = releases_path.join(&name);
                starbase_fs::create_dir_all(&release_path)
                    .map_err(|e| miette!("Failed to create release directory: {}", e))?;
                // Note: Context mutations disabled - needs Arc<Mutex<Context>> or interior mutability
                info(&format!("Created release {:?}", release_path));
                Ok(())
            }),
        )
        .desc("Create release"),
    );
}
