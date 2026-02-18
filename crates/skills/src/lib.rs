//! Agent skills management: install, list, remove, validate, audit, find, init, create, check, update.

mod add;
mod agents;
mod config;
mod monorepo;
mod audit;
mod gitignore;
mod install;
mod detect;
mod check;
mod skill_lock;
mod plugin_manifest;
mod providers;
mod source_parser;
mod context;
mod find;
mod init_cmd;
mod list;
mod remove;
mod update;
mod validate;

pub use context::SkillsContext;
pub use validate::ValidationResult;

use clap::Subcommand;
use starbase::AppResult;

#[derive(Subcommand, Debug, Clone)]
pub enum SkillsCommands {
    /// Install all skills from skills.json (declarative)
    #[command(alias = "i")]
    Install {
        /// Install to ~/.appz/skills (user-global)
        #[arg(long, short = 'g')]
        global: bool,
        /// Skip confirmation prompts
        #[arg(long, short = 'y')]
        yes: bool,
        /// Target agent(s) (default: claude-code)
        #[arg(long, short = 'a')]
        agent: Vec<String>,
    },
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
        /// List skills from source without installing
        #[arg(long)]
        list: bool,
        /// Target agent(s) to install to (e.g. cursor, claude)
        #[arg(long, short = 'a')]
        agent: Vec<String>,
        /// Install all skills from a multi-skill repo (default when no -s)
        #[arg(long)]
        all: bool,
        /// Recurse full depth when discovering skills
        #[arg(long)]
        full_depth: bool,
        /// Do not add source to skills.json
        #[arg(long, default_value_t = false)]
        no_save: bool,
    },
    /// List installed skills
    List {
        /// Show global skills only
        #[arg(long, short = 'g')]
        global: bool,
        /// Show project skills only
        #[arg(long, short = 'p')]
        project: bool,
        /// Filter by agent
        #[arg(long, short = 'a')]
        agent: Vec<String>,
    },
    /// Remove a skill by name
    Remove {
        /// Skill name to remove (omit with --all)
        name: Option<String>,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
        /// Remove from global only
        #[arg(long, short = 'g')]
        global: bool,
        /// Remove from project only
        #[arg(long, short = 'p')]
        project: bool,
        /// Filter by agent
        #[arg(long, short = 'a')]
        agent: Vec<String>,
        /// Remove all installed skills
        #[arg(long)]
        all: bool,
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
    /// Search for skills (local search of installed skills)
    Find {
        /// Search query (searches name and description)
        query: Option<String>,
    },
    /// Initialize project with skills.json (detect project characteristics and recommend skills)
    Init {
        /// Path to project directory (default: current directory)
        #[arg(long, short = 'C')]
        path: Option<std::path::PathBuf>,
        /// Output only JSON
        #[arg(long)]
        json: bool,
        /// Skip searching for skills (detection only)
        #[arg(long)]
        skip_search: bool,
        /// Output file path (default: skills.json in project dir)
        #[arg(long, short = 'o')]
        output: Option<std::path::PathBuf>,
    },
    /// Create a new skill with SKILL.md template
    Create {
        /// Skill name
        name: Option<String>,
        /// Output directory (default: current directory)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
    /// Check installed skills for updates
    Check,
    /// Update outdated skills
    Update,
}

/// Route skills subcommands to their respective handlers.
pub async fn run(ctx: SkillsContext, command: SkillsCommands) -> AppResult {
    match command {
        SkillsCommands::Install {
            global,
            yes,
            agent,
        } => install::install(&ctx, global, agent, yes).await,
        SkillsCommands::Add {
            source,
            global,
            project,
            yes,
            skill,
            list,
            agent,
            all: _all,
            full_depth,
            no_save,
        } => add::add(&ctx, source, global, project, yes, skill, list, &agent, _all, full_depth, None, no_save).await,
        SkillsCommands::List {
            global: global_only,
            project: project_only,
            agent,
        } => list::list(&ctx, global_only, project_only, &agent).await,
        SkillsCommands::Remove {
            name,
            yes,
            global: global_only,
            project: project_only,
            agent,
            all,
        } => {
            remove::remove(&ctx, name, yes, global_only, project_only, &agent, all).await
        }
        SkillsCommands::Validate { path, strict, json } => validate::validate(&ctx, path, strict, json).await,
        SkillsCommands::Audit {
            name_or_path,
            json,
            min_risk,
        } => audit::audit(&ctx, name_or_path, json, min_risk).await,
        SkillsCommands::Find { query } => find::find(&ctx, query).await,
        SkillsCommands::Init {
            path,
            json,
            skip_search,
            output,
        } => detect::run_detect(&ctx, path, json, skip_search, output).await,
        SkillsCommands::Create { name, output } => init_cmd::init_cmd(&ctx, name, output).await,
        SkillsCommands::Check => check::check(&ctx).await,
        SkillsCommands::Update => update::update(&ctx).await,
    }
}
