use crate::systems::bootstrap;
use clap::{Parser, Subcommand, ValueEnum};
use env_var::GlobalEnvBag;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "appz", version, about = "")]
pub struct Cli {
    #[arg(long, short = 'v', global = true, help = "Print live progress")]
    pub verbose: bool,
    #[arg(
        value_enum,
        long,
        global = true,
        help = "Lowest log level to output",
        help_heading = "Global options"
    )]
    pub log: Option<LogLevel>,
    #[arg(
        long,
        global = true,
        help = "Dump a trace profile to the working directory",
        help_heading = "Global options"
    )]
    pub dump: bool,
    #[arg(
        long,
        global = true,
        help = "Path to a file to write logs to",
        help_heading = "Global options"
    )]
    pub log_file: Option<PathBuf>,
    #[arg(
        long,
        short = 't',
        global = true,
        help = "Authentication token (overrides APPZ_TOKEN env var and auth.json)",
        help_heading = "Global options"
    )]
    pub token: Option<String>,
    #[arg(
        long,
        global = true,
        help = "Working directory (defaults to current directory)",
        help_heading = "Global options"
    )]
    pub cwd: Option<String>,
    #[arg(
        long,
        short = 'S',
        global = true,
        help = "Execute command from a scope that's not currently active",
        help_heading = "Global options"
    )]
    pub scope: Option<String>,
    #[command(subcommand)]
    pub command: Commands,
    /// Load a WASM plugin
    #[arg(long)]
    pub plugin: Option<String>,
}

impl Cli {
    /// Setup environment variables based on CLI options
    pub fn setup_env_vars(&self) {
        let bag = GlobalEnvBag::instance();

        // Setup colors
        bootstrap::setup_colors(false);

        // Set verbose flag in environment if needed
        if self.verbose {
            bag.set("APPZ_VERBOSE", "1");
        }

        // Set appz version
        let version = env!("CARGO_PKG_VERSION");
        bag.set("APPZ_VERSION", version);

        // Set starbase log level from --log or --verbose
        if let Some(level) = &self.log {
            bag.set("STARBASE_LOG", level.as_str());
        } else if self.verbose {
            bag.set("STARBASE_LOG", "debug");
        }

        // Dump/profile flag
        if self.dump {
            bag.set("STARBASE_DUMP_TRACE", "1");
        }

        // Log file path
        if let Some(path) = &self.log_file {
            bag.set("STARBASE_LOG_FILE", path.as_os_str());
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum LogLevel {
    Quiet,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Quiet => "quiet",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }

    pub fn is_verbose(&self) -> bool {
        matches!(self, LogLevel::Debug | LogLevel::Trace)
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// List all tasks
    List,
    /// Show execution plan for a task
    Plan { task: String },
    /// Run a task
    Run {
        task: String,
        /// Always execute tasks, ignoring source changes
        #[arg(long)]
        force: bool,
        /// Only run tasks with changed sources
        #[arg(long)]
        changed: bool,
    },
    /// Validate the recipe file (YAML/JSON) without registering tasks
    RecipeValidate { path: Option<String> },
    /// Detect and print the framework used in the project
    Dev {
        /// Share the dev server with a public URL using cloudflared
        #[arg(long)]
        share: bool,
        /// Port for the dev server (default: 3000)
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Start a local development server with hot reloading
    #[cfg(feature = "dev-server")]
    DevServer {
        /// Port to bind to (default: 3000)
        #[arg(short, long, default_value = "3000")]
        port: u16,
        /// Directory to serve (default: current directory)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Disable hot reload
        #[arg(long)]
        no_reload: bool,
        /// Enable form data processing
        #[arg(long)]
        enable_forms: bool,
        /// Enable SPA mode: serve index.html for route-like 404s
        #[arg(long)]
        spa_fallback: bool,
    },
    /// Build the project (install dependencies and build)
    Build,
    /// Preview the built project by serving static files from output directory
    #[cfg(feature = "dev-server")]
    Preview {
        /// Port to bind to (default: 3000)
        #[arg(short, long, default_value = "3000")]
        port: u16,
        /// Directory to serve (default: detect from framework)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Share the preview server with a public URL using cloudflared
        #[arg(long)]
        share: bool,
        /// Enable SPA mode: serve index.html for route-like 404s
        #[arg(long)]
        spa_fallback: bool,
    },
    /// List all deployments
    Ls,
    /// Open the linked project in the Appz Dashboard
    Open,
    /// Link current directory to a project
    Link {
        /// Project ID or slug to link to
        project: Option<String>,
        /// Team ID or slug
        #[arg(short, long)]
        team: Option<String>,
    },
    /// Unlink current directory from project
    Unlink,
    /// Log in to your Appz account
    Login,
    /// Log out and clear authentication token
    Logout,
    /// Show the username of the currently logged-in user
    Whoami {
        /// Output as JSON (username, email, name)
        #[arg(long)]
        json: bool,
        /// Output format (e.g. json)
        #[arg(long)]
        format: Option<String>,
    },
    /// Initialize a new project from a template
    Init {
        /// Template name (built-in) or project name
        template_or_name: Option<String>,
        /// Project name/directory (explicit, takes precedence over positional)
        #[arg(short, long)]
        name: Option<String>,
        /// Template source (GitHub URL, npm package, local path, or built-in template name)
        #[arg(short = 'T', long)]
        template: Option<String>,
        /// Skip dependency installation
        #[arg(long)]
        skip_install: bool,
        /// Overwrite existing directory
        #[arg(long)]
        force: bool,
        /// Output directory (defaults to current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Switch the active team context
    Switch {
        /// Team ID or slug
        team: String,
    },
    /// Manage teams
    Teams {
        #[command(subcommand)]
        command: crate::commands::teams::TeamsCommands,
    },
    /// Enable or disable telemetry collection (Vercel-aligned)
    Telemetry {
        #[command(subcommand)]
        command: crate::commands::telemetry::TelemetryCommands,
    },
    /// Manage projects (Vercel-aligned: project ls | add | inspect | rm)
    #[command(name = "project", alias = "projects")]
    Projects {
        /// Subcommand (defaults to list when omitted)
        #[command(subcommand)]
        command: Option<crate::commands::projects::ProjectsCommands>,
    },
    /// Manage aliases
    Aliases {
        #[command(subcommand)]
        command: crate::commands::aliases::AliasesCommands,
    },
    /// Manage domains
    Domains {
        #[command(subcommand)]
        command: crate::commands::domains::DomainsCommands,
    },
    /// Promote a deployment to production
    Promote {
        /// Deployment ID or URL to promote
        deployment: Option<String>,
        /// Time to wait for promotion completion (e.g., "3m", "30s")
        #[arg(long)]
        timeout: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Rollback to a previous deployment
    Rollback {
        /// Deployment ID or URL to rollback to
        deployment: Option<String>,
        /// Time to wait for rollback completion (e.g., "3m", "30s")
        #[arg(long)]
        timeout: Option<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Remove deployments (by URL/ID) or project (by name). Alias: rm.
    #[command(alias = "rm")]
    Remove {
        /// Deployment URL(s)/ID(s) or project name (Vercel-aligned: deployments by URL, project by name removes entire project)
        resources: Vec<String>,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
        /// When removing a project: skip if it has deployments with active preview/production URL
        #[arg(long, short = 's')]
        safe: bool,
    },
    /// Generate a website from a natural-language prompt (AI)
    #[cfg(feature = "gen")]
    Gen {
        /// Natural-language prompt describing the website to generate
        #[arg(required = true, trailing_var_arg = true)]
        prompt: Vec<String>,
        /// Output directory (default: ./gen-output or ./<name> if --name set)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Project name (used as output dir name if --output not set)
        #[arg(short, long)]
        name: Option<String>,
        /// AI model to use (backend default if not set)
        #[arg(short, long)]
        model: Option<String>,
    },
    /// Deploy to a hosting provider (Vercel, Netlify, Cloudflare Pages, etc.)
    #[cfg(feature = "deploy")]
    Deploy {
        /// Target provider (vercel, netlify, cloudflare-pages, github-pages, etc.)
        /// Auto-detected if not specified.
        provider: Option<String>,

        /// Deploy as preview instead of production
        #[arg(long)]
        preview: bool,

        /// Skip the build step before deploying
        #[arg(long)]
        no_build: bool,

        /// Show what would happen without actually deploying
        #[arg(long)]
        dry_run: bool,

        /// Output results as JSON (useful for CI/CD)
        #[arg(long)]
        json: bool,

        /// Deploy to all configured targets in parallel
        #[arg(long)]
        all: bool,

        /// Skip confirmation prompts (for CI/CD)
        #[arg(long, short)]
        yes: bool,
    },
    /// Set up deployment configuration for a provider
    #[cfg(feature = "deploy")]
    DeployInit {
        /// Target provider to configure (vercel, netlify, etc.)
        provider: Option<String>,
    },
    /// List recent deployments
    #[cfg(feature = "deploy")]
    DeployList {
        /// Provider to list deployments from
        provider: Option<String>,
    },
    // NOTE: The `check` command has been extracted to a downloadable plugin.
    // It is now handled by the External(Vec<String>) variant below.
    // NOTE: The `site` command has been extracted to a downloadable plugin (pro tier).
    // It is now handled by the External(Vec<String>) variant below.
    /// Semantic code search over indexed codebase (Repomix + Qdrant)
    ///
    /// Requires Node.js (for Repomix) and Qdrant. Install Qdrant via:
    /// `mise use -g ubi:qdrant/qdrant` or Docker: `docker run -p 6334:6334 qdrant/qdrant`
    #[cfg(feature = "code-search")]
    Code {
        #[command(subcommand)]
        command: crate::commands::code::CodeCommands,
    },
    /// Manage Agent Skills (install, list, remove, validate, audit, find, init, create, check, update)
    Skills {
        #[command(subcommand)]
        command: skills_lib::SkillsCommands,
    },
    /// Manage downloadable plugins (list, update)
    Plugin {
        #[command(subcommand)]
        command: crate::commands::plugin::PluginCommands,
    },
    /// Run the MCP (Model Context Protocol) server for AI assistants (Cursor, Claude, etc.)
    #[cfg(feature = "mcp")]
    #[command(name = "mcp")]
    McpServer,
    // NOTE: The `convert` command has been extracted to the ssg-migrator plugin.
    // It is now handled by the External(Vec<String>) variant below.
    // NOTE: The `migrate` command has been extracted to a downloadable plugin.
    // It is now handled by the External(Vec<String>) variant below.
    // Users run `appz migrate ...` which triggers the plugin system.
    /// Update appz itself to the latest version
    #[cfg(feature = "self_update")]
    SelfUpdate {
        /// Update to a specific version
        version: Option<String>,
        /// Update even if already up to date
        #[arg(long, short)]
        force: bool,
        /// Skip confirmation prompt
        #[arg(long, short)]
        yes: bool,
    },
    /// Commands provided by downloadable plugins
    #[command(external_subcommand)]
    External(Vec<String>),
}
