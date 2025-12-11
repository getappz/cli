use std::path::PathBuf;
use std::sync::Arc;
use task::TaskRegistry;

/// Application context that holds shared components accessible throughout the CLI.
///
/// Similar to moon-repo's AppContext pattern, this provides a centralized way
/// to access application state and components without passing individual pieces around.
///
/// Components can access the context through `session.get_app_context()`.
#[derive(Clone)]
pub struct AppContext {
    /// Working directory where the command was executed
    pub working_dir: PathBuf,

    /// Task registry with all registered tasks
    pub task_registry: Arc<TaskRegistry>,

    /// Verbose mode flag
    pub verbose: bool,
}

impl std::fmt::Debug for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContext")
            .field("working_dir", &self.working_dir)
            .field(
                "task_registry",
                &format!(
                    "Arc<TaskRegistry> ({} tasks)",
                    self.task_registry.all().count()
                ),
            )
            .field("verbose", &self.verbose)
            .finish()
    }
}

impl AppContext {
    pub fn new(working_dir: PathBuf, task_registry: Arc<TaskRegistry>, verbose: bool) -> Self {
        Self {
            working_dir,
            task_registry,
            verbose,
        }
    }
}
