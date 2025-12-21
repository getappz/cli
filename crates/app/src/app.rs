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
    },
    /// Build the project (install dependencies and build)
    Build,
    /// Preview the built project by serving static files from output directory
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
    },
    /// SEO commands
    Seo {
        #[command(subcommand)]
        command: crate::commands::seo::SeoCommands,
    },
    /// List all deployments
    Ls,
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
    /// Initialize a new project from a template
    Init {
        /// Template source (GitHub URL, npm package, local path, or built-in template name) or project name
        template_or_name: Option<String>,
        /// Project name/directory (when template is provided as first positional argument)
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
    /// Manage projects
    Projects {
        #[command(subcommand)]
        command: crate::commands::projects::ProjectsCommands,
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
    /// Remove resources (projects, aliases, domains, teams)
    Remove {
        /// Resource identifiers (project IDs/slugs, alias IDs/strings, domain names, team IDs/slugs)
        resources: Vec<String>,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
        /// Skip resources with active aliases (not fully implemented)
        #[arg(long)]
        safe: bool,
    },
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
}
