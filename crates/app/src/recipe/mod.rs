use crate::log::{info, warning};
use task::{Task, TaskRegistry};

/// Registers the common deploy recipe similar to Deployer's `recipe/common.php`.
pub fn register_common(reg: &mut TaskRegistry) {
    // Building block tasks (real implementations)
    deploy::info::register_info(reg);
    deploy::setup::register_setup(reg);
    deploy::lock::register_lock(reg);
    deploy::release::register_release(reg);
    deploy::update_code::register_update_code(reg);
    deploy::env::register_env(reg);
    deploy::shared::register_shared(reg);
    deploy::writable::register_writable(reg);

    reg.register(
        Task::new(
            "deploy:prepare",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| Ok(())),
        )
        .desc("Prepares a new release")
        .depends_on("deploy:info")
        .depends_on("deploy:setup")
        .depends_on("deploy:lock")
        .depends_on("deploy:release")
        .depends_on("deploy:update_code")
        .depends_on("deploy:env")
        .depends_on("deploy:shared")
        .depends_on("deploy:writable"),
    );

    deploy::symlink::register_symlink(reg);
    // unlock already registered in lock.rs
    deploy::cleanup::register_cleanup(reg);
    reg.register(
        Task::new(
            "deploy:success",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| {
                info("successfully deployed!");
                Ok(())
            }),
        )
        .hidden(),
    );

    reg.register(
        Task::new(
            "deploy:publish",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| Ok(())),
        )
        .desc("Publishes the release")
        .depends_on("deploy:symlink")
        .depends_on("deploy:unlock")
        .depends_on("deploy:cleanup")
        .depends_on("deploy:success"),
    );

    reg.register(
        Task::new(
            "deploy",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| Ok(())),
        )
        .desc("Deploys your project")
        .depends_on("deploy:prepare")
        .depends_on("deploy:publish"),
    );

    reg.register(
        Task::new(
            "deploy:failed",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| {
                warning("Deploy failed");
                Ok(())
            }),
        )
        .hidden(),
    );
    reg.fail("deploy", "deploy:failed");
}

pub mod common;
pub mod deploy;
pub mod laravel;
pub mod tools;
pub mod vercel;
