use std::path::PathBuf;

use miette::miette;

use crate::{
    log::info,
    shell::{copy_path_recursive, run_local_with, which, RunOptions},
};
use common::consts::{K_BRANCH, K_LOCAL_SOURCE, K_RELEASE_PATH, K_REPOSITORY};
use task::{Task, TaskRegistry};

pub fn register_update_code(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "deploy:update_code",
            task::task_fn_async!(|ctx: std::sync::Arc<task::Context>| async move {
                let release = ctx
                    .get(K_RELEASE_PATH)
                    .ok_or_else(|| miette!("release_path not set"))?;
                let release_path = PathBuf::from(release);

                if let Some(src) = ctx.get(K_LOCAL_SOURCE) {
                    info(&format!("Copying local source from {}", src));
                    copy_path_recursive(&PathBuf::from(src), &release_path)
                        .map_err(|e| miette!("Failed to copy local source: {}", e))?;
                    return Ok(());
                }

                if let Some(repo) = ctx.get(K_REPOSITORY) {
                    let branch = ctx.get(K_BRANCH).unwrap_or("main".to_string());
                    // try git clone --depth=1 -b branch repo release_path
                    let _ = which("git").map_err(|e| miette!("Failed to find git: {}", e))?;
                    let cmd = format!(
                        "git clone --depth=1 -b {} {} {}",
                        branch,
                        repo,
                        release_path.to_string_lossy()
                    );
                    run_local_with(&ctx, &cmd, RunOptions::default())
                        .await
                        .map_err(|e| miette!("Failed to clone repository: {}", e))?;
                    return Ok(());
                }

                // Nothing to do
                info("No repository or local_source specified; skipping update_code");
                Ok(())
            }),
        )
        .desc("Update code"),
    );
}
