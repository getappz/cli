//! Args structs for Commands (Moon-style tuple variants).

use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug, Clone)]
pub enum DevSubcommand {
    /// Stop DDEV containers (DDEV projects only)
    Stop,
}

#[derive(Args, Debug, Clone)]
pub struct DevArgs {
    #[command(subcommand)]
    pub command: Option<DevSubcommand>,
    /// Share the dev server with a public URL using cloudflared
    #[arg(long)]
    pub share: bool,
    /// Port for the dev server (default: 3000)
    #[arg(short, long)]
    pub port: Option<u16>,
}

#[cfg(feature = "dev-server")]
#[derive(Args, Debug, Clone)]
pub struct DevServerArgs {
    /// Port to bind to (default: 3000)
    #[arg(short, long, default_value = "3000")]
    pub port: u16,
    /// Directory to serve (default: current directory)
    #[arg(short, long)]
    pub dir: Option<PathBuf>,
    /// Disable hot reload
    #[arg(long)]
    pub no_reload: bool,
    /// Enable form data processing
    #[arg(long)]
    pub enable_forms: bool,
    /// Enable SPA mode: serve index.html for route-like 404s
    #[arg(long)]
    pub spa_fallback: bool,
}

#[cfg(feature = "dev-server")]
#[derive(Args, Debug, Clone)]
pub struct PreviewArgs {
    /// Port to bind to (default: 3000)
    #[arg(short, long, default_value = "3000")]
    pub port: u16,
    /// Directory to serve (default: detect from framework)
    #[arg(short, long)]
    pub dir: Option<PathBuf>,
    /// Share the preview server with a public URL using cloudflared
    #[arg(long)]
    pub share: bool,
    /// Enable SPA mode: serve index.html for route-like 404s
    #[arg(long)]
    pub spa_fallback: bool,
}

#[cfg(feature = "appz-cloud")]
#[derive(Args, Debug, Clone)]
pub struct LsArgs {
    /// See deployments with deployment retention policies (e.g. -p errored=6m -p preview=12m)
    #[arg(long, short = 'p', value_name = "KEY=VALUE")]
    pub policy: Vec<String>,
    /// Skip confirmation prompts (e.g. when linking project in non-interactive mode)
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(Args, Debug, Clone)]
pub struct LinkArgs {
    /// Project ID or slug to link to
    pub project: Option<String>,
    /// Team ID or slug (use -T to avoid conflict with global -t for token)
    #[arg(short = 'T', long)]
    pub team: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct WhoamiArgs {
    /// Output as JSON (username, email, name)
    #[arg(long)]
    pub json: bool,
    /// Output format (e.g. json)
    #[arg(long)]
    pub format: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct InitArgs {
    /// Template name (built-in) or project name
    pub template_or_name: Option<String>,
    /// Project name/directory (explicit, takes precedence over positional)
    #[arg(short, long)]
    pub name: Option<String>,
    /// Template source (GitHub URL, npm package, local path, or built-in template name)
    #[arg(short = 'T', long)]
    pub template: Option<String>,
    /// Skip dependency installation
    #[arg(long)]
    pub skip_install: bool,
    /// Overwrite existing directory
    #[arg(long)]
    pub force: bool,
    /// Output directory (defaults to current directory)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[cfg(feature = "appz-cloud")]
#[derive(Args, Debug, Clone)]
pub struct SwitchArgs {
    /// Team ID or slug
    pub team: String,
}

#[cfg(feature = "appz-cloud")]
#[derive(Args, Debug, Clone)]
pub struct PullArgs {
    /// Target environment (development, preview, production) [default: development]
    #[arg(long, short = 'e', default_value = "development")]
    pub environment: String,
    /// Skip overwrite confirmation
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[cfg(feature = "appz-cloud")]
#[derive(Args, Debug, Clone)]
pub struct LogsArgs {
    /// Deployment URL or ID (uses latest from linked project if omitted)
    pub deployment: Option<String>,
}

#[cfg(feature = "appz-cloud")]
#[derive(Args, Debug, Clone)]
pub struct InspectArgs {
    /// Deployment URL or ID
    pub deployment: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[cfg(feature = "appz-cloud")]
#[derive(Args, Debug, Clone)]
pub struct PromoteArgs {
    /// Deployment ID or URL to promote
    pub deployment: Option<String>,
    /// Time to wait for promotion completion (e.g., "3m", "30s")
    #[arg(long)]
    pub timeout: Option<String>,
    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[cfg(feature = "appz-cloud")]
#[derive(Args, Debug, Clone)]
pub struct RollbackArgs {
    /// Deployment ID or URL to rollback to
    pub deployment: Option<String>,
    /// Time to wait for rollback completion (e.g., "3m", "30s")
    #[arg(long)]
    pub timeout: Option<String>,
    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

#[cfg(feature = "appz-cloud")]
#[derive(Args, Debug, Clone)]
pub struct RemoveArgs {
    /// Deployment URL(s)/ID(s) or project name (Vercel-aligned: deployments by URL, project by name removes entire project)
    pub resources: Vec<String>,
    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
    /// When removing a project: skip if it has deployments with active preview/production URL
    #[arg(long, short = 's')]
    pub safe: bool,
}

#[cfg(feature = "deploy")]
#[derive(Args, Debug, Clone)]
pub struct DeployArgs {
    /// Project path to deploy (defaults to current directory)
    #[arg()]
    pub project_path: Option<PathBuf>,

    /// Target provider (vercel, netlify, cloudflare-pages, github-pages, etc.)
    /// Auto-detected if not specified.
    pub provider: Option<String>,

    /// Create a production deployment (shorthand for --target=production)
    #[arg(long)]
    pub prod: bool,

    /// Deploy as preview instead of production
    #[arg(long)]
    pub preview: bool,

    /// Specify the target deployment environment (preview, production, staging)
    #[arg(long)]
    pub target: Option<String>,

    /// Deploy existing build output (use with `appz build` first)
    #[arg(long)]
    pub prebuilt: bool,

    /// Skip the build step before deploying
    #[arg(long)]
    pub no_build: bool,

    /// Specify environment variables during build-time (e.g. -b KEY1=value1 -b KEY2=value2)
    #[arg(long, short = 'b', value_name = "KEY=VALUE")]
    pub build_env: Vec<String>,

    /// Specify environment variables during run-time (e.g. -e KEY1=value1 -e KEY2=value2)
    #[arg(long, short = 'e', value_name = "KEY=VALUE")]
    pub env: Vec<String>,

    /// Force a new deployment even if nothing has changed
    #[arg(long, short = 'f')]
    pub force: bool,

    /// Receive command suggestions once deployment is complete
    #[arg(long)]
    pub guidance: bool,

    /// Print the build logs
    #[arg(long, short = 'l')]
    pub logs: bool,

    /// Specify metadata for the deployment (e.g. -m KEY1=value1 -m KEY2=value2)
    #[arg(long, short = 'm', value_name = "KEY=VALUE")]
    pub meta: Vec<String>,

    /// Don't wait for the deployment to finish
    #[arg(long)]
    pub no_wait: bool,

    /// Deployment is public
    #[arg(long, short = 'p')]
    pub public: bool,

    /// Disable automatic promotion of domains to the new deployment
    #[arg(long)]
    pub skip_domain: bool,

    /// Retain build cache when using --force
    #[arg(long)]
    pub with_cache: bool,

    /// Show what would happen without actually deploying
    #[arg(long)]
    pub dry_run: bool,

    /// Output results as JSON (useful for CI/CD)
    #[arg(long)]
    pub json: bool,

    /// Deploy to all configured targets in parallel
    #[arg(long)]
    pub all: bool,

    /// Use default options and skip all prompts (for CI/CD)
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Set up deployment configuration for a provider (interactive wizard)
    #[arg(long)]
    pub init: bool,
}

#[cfg(feature = "deploy")]
#[derive(Args, Debug, Clone)]
pub struct DeployListArgs {
    /// Provider to list deployments from
    pub provider: Option<String>,
}

#[cfg(feature = "self_update")]
#[derive(Args, Debug, Clone)]
pub struct SelfUpdateArgs {
    /// Update to a specific version
    pub version: Option<String>,
    /// Update even if already up to date
    #[arg(long, short)]
    pub force: bool,
    /// Skip confirmation prompt
    #[arg(long, short)]
    pub yes: bool,
}
