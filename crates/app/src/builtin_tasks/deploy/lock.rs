use starbase_utils::fs as starbase_fs;
use std::{env, path::PathBuf};

use miette::miette;

use crate::log::info;
use common::consts::{DEP_DIR, LOCK_FILE};
use task::{Context, Task, TaskRegistry};

fn resolve_deploy_path(ctx: &Context) -> miette::Result<PathBuf> {
    if let Some(p) = ctx.get("deploy_path") {
        return Ok(PathBuf::from(p));
    }
    env::current_dir().map_err(|e| miette!("Failed to get current directory: {}", e))
}

fn resolve_user() -> String {
    env::var("GITLAB_USER_NAME")
        .or_else(|_| env::var("GITHUB_ACTOR"))
        .or_else(|_| env::var("CIRCLE_USERNAME"))
        .or_else(|_| env::var("DRONE_BUILD_TRIGGER"))
        .or_else(|_| env::var("USER"))
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "ci".to_string())
}

pub fn register_lock(reg: &mut TaskRegistry) {
    // Locks deploy
    reg.register(
        Task::new(
            "deploy:lock",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let deploy_path = resolve_deploy_path(&ctx)?;
                let dep_dir = deploy_path.join(DEP_DIR);
                let lock_file = dep_dir.join(LOCK_FILE);

                starbase_fs::create_dir_all(&dep_dir)
                    .map_err(|e| miette!("Failed to create directory: {}", e))?;
                if lock_file.exists() {
                    let locked_user = starbase_fs::read_file(&lock_file)
                        .unwrap_or_else(|_| "unknown".to_string());
                    return Err(miette!(
                        "Deploy locked by {}.\nExecute \"deploy:unlock\" task to unlock.",
                        locked_user.trim()
                    ));
                }

                let user = resolve_user();
                starbase_fs::write_file(&lock_file, &user)
                    .map_err(|e| miette!("Failed to write lock file: {}", e))?;
                Ok(())
            }),
        )
        .desc("Locks deploy"),
    );

    // Unlocks deploy
    reg.register(
        Task::new(
            "deploy:unlock",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let deploy_path = resolve_deploy_path(&ctx)?;
                let lock_file = deploy_path.join(DEP_DIR).join(LOCK_FILE);
                let _ = starbase_fs::remove_file(&lock_file);
                Ok(())
            }),
        )
        .desc("Unlocks deploy"),
    );

    // Checks if deploy is locked
    reg.register(
        Task::new(
            "deploy:is_locked",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let deploy_path = resolve_deploy_path(&ctx)?;
                let lock_file = deploy_path.join(DEP_DIR).join(LOCK_FILE);
                if lock_file.exists() {
                    let locked_user = starbase_fs::read_file(&lock_file)
                        .unwrap_or_else(|_| "unknown".to_string());
                    return Err(miette!("Deploy is locked by {}.", locked_user.trim()));
                }
                info("Deploy is unlocked.");
                Ok(())
            }),
        )
        .desc("Checks if deploy is locked"),
    );
}
