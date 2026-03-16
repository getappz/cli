use crate::args::*;
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
    /// Detect and print the framework used in the project
    Dev(DevArgs),
    /// Start a local development server with hot reloading
    #[cfg(feature = "dev-server")]
    DevServer(DevServerArgs),
    /// Build the project (install dependencies and build)
    Build,
    /// Preview the built project by serving static files from output directory
    #[cfg(feature = "dev-server")]
    Preview(PreviewArgs),
    /// List all deployments (Vercel parity: appz list [project] [--policy KEY=value])
    #[cfg(feature = "appz-cloud")]
    Ls(LsArgs),
    /// Open the linked project in the Appz Dashboard
    #[cfg(feature = "appz-cloud")]
    Open,
    /// Link current directory to a project
    #[cfg(feature = "appz-cloud")]
    Link(LinkArgs),
    /// Unlink current directory from project
    #[cfg(feature = "appz-cloud")]
    Unlink,
    /// Log in to your Appz account
    #[cfg(feature = "appz-cloud")]
    Login,
    /// Log out and clear authentication token
    #[cfg(feature = "appz-cloud")]
    Logout,
    /// Show the username of the currently logged-in user
    #[cfg(feature = "appz-cloud")]
    Whoami(WhoamiArgs),
    /// Initialize a new project from a template
    Init(InitArgs),
    /// Apply a WordPress Playground blueprint
    Blueprint {
        #[command(subcommand)]
        command: crate::commands::blueprint::BlueprintCommands,
    },
    /// Switch the active team context
    #[cfg(feature = "appz-cloud")]
    Switch(SwitchArgs),
    /// Manage teams
    #[cfg(feature = "appz-cloud")]
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
    #[cfg(feature = "appz-cloud")]
    #[command(name = "project", alias = "projects")]
    Projects {
        /// Subcommand (defaults to list when omitted)
        #[command(subcommand)]
        command: Option<crate::commands::projects::ProjectsCommands>,
    },
    /// Transfer projects between teams (Vercel-aligned: transfer <project> | transfer accept <code>)
    #[cfg(feature = "appz-cloud")]
    #[command(subcommand_required = false)]
    Transfer {
        #[command(subcommand)]
        command: Option<crate::commands::transfer::TransferCommands>,
        /// Project to transfer (optional – uses linked project from CWD if omitted)
        #[arg(required = false)]
        project: Option<String>,
        /// Target team for direct transfer (with project)
        #[arg(long)]
        to_team: Option<String>,
    },
    /// Manage aliases (Vercel parity: alias set | ls | rm)
    #[cfg(feature = "appz-cloud")]
    #[command(name = "alias", alias = "aliases")]
    Aliases {
        #[command(subcommand)]
        command: crate::commands::aliases::AliasesCommands,
    },
    /// Manage domains (Vercel parity: domains ls | add | rm)
    #[cfg(feature = "appz-cloud")]
    #[command(name = "domains", alias = "domain")]
    Domains {
        #[command(subcommand)]
        command: crate::commands::domains::DomainsCommands,
    },
    /// Pull project config and env from Appz (writes .appz/project.json, .env[.environment].local)
    #[cfg(feature = "appz-cloud")]
    Pull(PullArgs),
    /// Show deployment logs
    #[cfg(feature = "appz-cloud")]
    Logs(LogsArgs),
    /// Inspect deployment details
    #[cfg(feature = "appz-cloud")]
    Inspect(InspectArgs),
    /// Manage environment variables (Vercel-aligned: env ls | add | rm | pull)
    #[cfg(feature = "appz-cloud")]
    #[command(name = "env")]
    Env {
        #[command(subcommand)]
        command: crate::commands::env::EnvCommands,
    },
    /// Promote a deployment to production
    #[cfg(feature = "appz-cloud")]
    Promote(PromoteArgs),
    /// Rollback to a previous deployment
    #[cfg(feature = "appz-cloud")]
    Rollback(RollbackArgs),
    /// Remove deployments (by URL/ID) or project (by name). Alias: rm.
    #[cfg(feature = "appz-cloud")]
    #[command(alias = "rm")]
    Remove(RemoveArgs),
    /// Export a WordPress site as static HTML for deployment to Vercel/Netlify
    WpExport(WpExportArgs),
    /// Deploy to a hosting provider (Vercel, Netlify, Cloudflare Pages, Appz, etc.)
    /// Vercel-parity: project-path, --prod, --prebuilt, -e, -b, -f, --logs, --target, etc.
    #[cfg(feature = "deploy")]
    Deploy(DeployArgs),
    /// List recent deployments
    #[cfg(feature = "deploy")]
    DeployList(DeployListArgs),
    // NOTE: The `check` command has been extracted to a downloadable plugin.
    // It is now handled by the External(Vec<String>) variant below.
    // NOTE: The `site` command has been extracted to a downloadable plugin (pro tier).
    // It is now handled by the External(Vec<String>) variant below.
    // NOTE: The `convert` command has been extracted to the ssg-migrator plugin.
    // It is now handled by the External(Vec<String>) variant below.
    // NOTE: The `migrate` command has been extracted to a downloadable plugin.
    // It is now handled by the External(Vec<String>) variant below.
    // Users run `appz migrate ...` which triggers the plugin system.
    /// Update appz itself to the latest version
    #[cfg(feature = "self_update")]
    SelfUpdate(SelfUpdateArgs),
    /// Commands provided by downloadable plugins
    #[command(external_subcommand)]
    External(Vec<String>),
}
