use starbase_utils::fs as starbase_fs;
use std::path::PathBuf;

use miette::miette;

use crate::log::info;
use common::consts::{DEP_DIR, K_DEPLOY_PATH, RELEASES_DIR, SHARED_DIR};
use task::{Context, Task, TaskRegistry};

fn resolve_deploy_path(ctx: &Context) -> miette::Result<PathBuf> {
    let p = ctx
        .get(K_DEPLOY_PATH)
        .ok_or_else(|| miette!("deploy_path is required"))?;
    Ok(PathBuf::from(p))
}

pub fn register_setup(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "deploy:setup",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let deploy_path = resolve_deploy_path(&ctx)?;
                let dep = deploy_path.join(DEP_DIR);
                let releases = deploy_path.join(RELEASES_DIR);
                let shared = deploy_path.join(SHARED_DIR);
                starbase_fs::create_dir_all(&dep)
                    .map_err(|e| miette!("Failed to create .dep directory: {}", e))?;
                starbase_fs::create_dir_all(&releases)
                    .map_err(|e| miette!("Failed to create releases directory: {}", e))?;
                starbase_fs::create_dir_all(&shared)
                    .map_err(|e| miette!("Failed to create shared directory: {}", e))?;
                info(&format!(
                    "Prepared {:?} (created .dep, releases, shared)",
                    deploy_path
                ));
                Ok(())
            }),
        )
        .desc("Setup directories"),
    );
}
