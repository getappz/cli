use starbase_utils::fs as starbase_fs;
use std::{fs, path::PathBuf};

use miette::miette;

use common::consts::{DOT_ENV_FILE, K_ENV_TEMPLATE, K_RELEASE_PATH};
use task::{Task, TaskRegistry};

pub fn register_env(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "deploy:env",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let release = ctx
                    .get(K_RELEASE_PATH)
                    .ok_or_else(|| miette!("release_path not set"))?;
                let env_path = PathBuf::from(release).join(DOT_ENV_FILE);
                if let Some(tpl) = ctx.get(K_ENV_TEMPLATE) {
                    let tplp = PathBuf::from(tpl);
                    if tplp.exists() {
                        if let Some(parent) = env_path.parent() {
                            starbase_fs::create_dir_all(parent)
                                .map_err(|e| miette!("Failed to create parent directory: {}", e))?;
                        }
                        fs::copy(tplp, &env_path)
                            .map_err(|e| miette!("Failed to copy env template: {}", e))?;
                    }
                }
                // Note: Context mutation via Arc requires interior mutability
                // For now, dotenv setting is handled separately - this is a limitation
                Ok(())
            }),
        )
        .desc("Set env"),
    );
}
