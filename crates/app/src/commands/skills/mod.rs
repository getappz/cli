//! Skills command module — install and manage Agent Skills.
//!
//! Similar to [skills.sh CLI](https://skills.sh/docs/cli): add skills from
//! GitHub, URLs, or local paths; list installed skills; remove skills.

use crate::session::AppzSession;
use clap::Subcommand;
use starbase::AppResult;

pub mod add;
pub mod audit;
pub mod list;
pub mod remove;
pub mod validate;

pub use add::add;
pub use audit::audit;
pub use list::list;
pub use remove::remove;
pub use validate::validate;

#[derive(Subcommand, Debug, Clone)]
pub enum SkillsCommands {
    /// Install a skill from GitHub (owner/repo), URL, or local path
    Add {
        /// Skill source: owner/repo, https://..., or ./local-path
        source: String,
        /// Install to ~/.appz/skills (user-global)
        #[arg(long, short = 'g')]
        global: bool,
        /// Install to project .agents/skills
        #[arg(long, short = 'p')]
        project: bool,
        /// Skip confirmation prompts
        #[arg(long, short = 'y')]
        yes: bool,
        /// Install only a specific skill from a repo with multiple skills
        #[arg(long, short = 's')]
        skill: Option<String>,
    },
    /// List installed skills
    List,
    /// Remove a skill by name
    Remove {
        /// Skill name to remove
        name: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Validate skill structure (frontmatter, body)
    Validate {
        /// Path to skill directory or SKILL.md (default: all installed skills)
        path: Option<std::path::PathBuf>,
        /// Treat warnings as errors
        #[arg(long)]
        strict: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Security audit of skill content
    Audit {
        /// Skill name or path (default: all installed skills)
        name_or_path: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Only show skills with risk at or above this level
        #[arg(long)]
        min_risk: Option<String>,
    },
}

/// Route skills subcommands to their respective handlers.
pub async fn run(session: AppzSession, command: SkillsCommands) -> AppResult {
    match command {
        SkillsCommands::Add {
            source,
            global,
            project,
            yes,
            skill,
        } => add(session, source, global, project, yes, skill).await,
        SkillsCommands::List => list(session).await,
        SkillsCommands::Remove { name, yes } => remove(session, name, yes).await,
        SkillsCommands::Validate { path, strict, json } => {
            validate(session, path, strict, json).await
        }
        SkillsCommands::Audit {
            name_or_path,
            json,
            min_risk,
        } => audit(session, name_or_path, json, min_risk).await,
    }
}
