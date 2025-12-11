use miette::miette;

use crate::log::info;
use task::{Task, TaskRegistry};

mod common;
mod install;
mod verify;

pub fn register_docker_tools(reg: &mut TaskRegistry) {
    // tools:docker:install — install Docker (if available) or Podman across supported platforms
    reg.register(
		Task::new(
			"tools:docker:install",
			task::task_fn_async!(|_ctx: std::sync::Arc<task::Context>| async move {
				info("Checking Docker/Podman installation status...");
				install::install().map_err(|e| miette!("container runtime install failed: {:#}", e))
			}),
		)
		.desc("Installs Docker (if available) or Podman across supported platforms (macOS, Linux, Windows)"),
	);

    // tools:docker:verify — check version and ensure it's working
    reg.register(
        Task::new(
            "tools:docker:verify",
            task::task_fn_async!(|_ctx: std::sync::Arc<task::Context>| async move {
                verify::verify().map_err(|e| miette!("container runtime verify failed: {:#}", e))
            }),
        )
        .desc("Verifies Docker/Podman installation and configuration"),
    );
}
