//! Context passed to skills commands. Decouples skills crate from app's AppzSession.

use std::path::PathBuf;

/// Minimal context for skills operations.
/// App constructs this from AppzSession when routing to skills commands.
#[derive(Clone)]
pub struct SkillsContext {
    /// Current working directory (project root).
    pub working_dir: PathBuf,
    /// Whether verbose output is enabled.
    pub verbose: bool,
    /// User's ~/.appz directory (or None if home not available).
    pub user_appz_dir: Option<PathBuf>,
}
