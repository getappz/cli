use miette::miette;

use crate::log::info;
use task::{Task, TaskRegistry};

mod common;
mod install;
mod mkcert;
mod uninstall;
mod verify;

pub fn register_ddev_tools(reg: &mut TaskRegistry) {
    // tools:ddev:install — install ddev across supported platforms based on availability
    reg.register(
        Task::new(
            "tools:ddev:install",
            task::task_fn_async!(|_ctx: std::sync::Arc<task::Context>| async move {
                info("Checking ddev installation status...");
                install::install().map_err(|e| miette!("ddev install failed: {:#}", e))
            }),
        )
        .desc("Installs ddev across supported platforms (macOS, Linux, Windows)"),
    );

    // tools:ddev:verify — check version and ensure mkcert is configured
    reg.register(
        Task::new(
            "tools:ddev:verify",
            task::task_fn_async!(|_ctx: std::sync::Arc<task::Context>| async move {
                verify::verify().map_err(|e| miette!("ddev verify failed: {:#}", e))
            }),
        )
        .desc("Verifies ddev installation and mkcert configuration"),
    );

    // tools:ddev:install_mkcert — install and configure mkcert
    reg.register(
        Task::new(
            "tools:ddev:install_mkcert",
            task::task_fn_async!(|_ctx: std::sync::Arc<task::Context>| async move {
                mkcert::install_mkcert().map_err(|e| miette!("mkcert install failed: {:#}", e))
            }),
        )
        .desc("Installs and configures mkcert for local SSL certificates"),
    );

    // tools:ddev:uninstall — comprehensive uninstall of ddev
    reg.register(
        Task::new(
            "tools:ddev:uninstall",
            task::task_fn_async!(|_ctx: std::sync::Arc<task::Context>| async move {
                uninstall::uninstall().map_err(|e| miette!("ddev uninstall failed: {:#}", e))
            }),
        )
        .desc("Uninstalls ddev, removes all projects, and cleans up related files"),
    );
}
