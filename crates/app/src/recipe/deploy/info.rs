use crate::log::info;
use task::{Task, TaskRegistry};

pub fn register_info(reg: &mut TaskRegistry) {
    reg.register(
        Task::new(
            "deploy:info",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                let user = std::env::var("USERNAME")
                    .or_else(|_| std::env::var("USER"))
                    .unwrap_or_else(|_| "unknown".to_string());
                let dp = ctx.get("deploy_path").unwrap_or("(unset)".to_string());
                info(&format!("Deploying as {} to {}", user, dp));
                Ok(())
            }),
        )
        .desc("Show deploy info")
        .hidden(),
    );
}
