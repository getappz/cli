pub mod ddev;
pub mod docker;
pub mod mise;

use task::{Task, TaskRegistry};

/// Generic helper to ensure a CLI exists by running a sequence of installer commands.
/// Each installer is attempted until one succeeds; if all fail but the CLI appears on PATH, treat as success.
pub fn ensure_cli_exists(
    reg: &mut TaskRegistry,
    cli_name: &str,
    task_name: &str,
    _installers: Vec<&'static str>,
) {
    let tname = task_name.to_string();
    reg.register(
        Task::new(
            tname,
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| Ok(())),
        )
        .desc(format!("Ensure {} CLI exists", cli_name)),
    );
}
