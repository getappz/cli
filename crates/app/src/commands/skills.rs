//! Bridge: routes skills commands to the skills crate.
//! Converts AppzSession to SkillsContext and delegates to skills::run.

use crate::session::AppzSession;
use skills_lib::{SkillsCommands, SkillsContext};

/// Route skills subcommands to the skills crate.
pub async fn run(session: AppzSession, command: SkillsCommands) -> starbase::AppResult {
    let ctx = SkillsContext {
        working_dir: session.working_dir.clone(),
        verbose: session.cli.verbose,
        user_appz_dir: common::user_config::user_appz_dir(),
    };
    skills_lib::run(ctx, command).await
}
