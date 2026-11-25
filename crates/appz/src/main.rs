use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use appz_core::{detect_toolchains, generate_claude_md, run_doctor};

mod agents;
mod backup;
mod build_stage;
mod ci;
mod cloud_env;
mod deploy;
mod deploy_state;
mod dev_install;
mod init_source;
mod list_filters;
mod mcp;
mod project_ops;
mod redirects;
mod secrets;
mod share;
mod shield;
mod skills;
mod static_serve;
// Some ui primitives land before every command adopts them.
#[allow(dead_code)]
mod ui;
mod update;
mod webhooks;

pub(crate) use ui::{
    error, info, intro, is_interactive_tty, is_stdout_tty, is_tty, outro, step, success, warning,
    with_spinner,
};

// ── CLI ─────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "appz", version, long_version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("APPZ_BUILD_SHA"), ")"))]
struct AppzCli {
    /// JSON output mode (for agent/CI use)
    #[arg(long, global = true)]
    json: bool,
    /// Answer yes to every confirmation (installing tools, linking, setting
    /// secrets, starting the dev server)
    #[arg(short = 'y', long, global = true)]
    yes: bool,
    #[command(subcommand)]
    command: AppzCmd,
}

static ASSUME_YES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether `--yes` / `-y` was given (anywhere on the command line).
fn assume_yes() -> bool {
    ASSUME_YES.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Subcommand)]
enum AppzCmd {
    /// Detect toolchains and set up the project
    Init(InitArgs),
    /// Install mise tools and project dependencies
    Install(InstallArgs),
    /// Build the project
    Build(BuildArgs),
    /// Start the dev server
    Dev(DevArgs),
    /// Serve build output or a static directory locally
    Preview(PreviewArgs),
    /// Share a local port publicly over a secure tunnel
    #[command(alias = "tunnel")]
    Share(share::ShareArgs),
    /// Run tests
    Test(TestArgs),
    /// Run linter
    Lint(LintArgs),
    /// Run code formatter
    Format(FormatArgs),
    /// Remove build/cache output for this project (dry-run unless --force)
    Clean(CleanArgs),
    /// Snapshot local project source in a Rustic repository
    Backup(backup::BackupArgs),
    /// Reconstruct a source snapshot in an empty directory
    Restore(backup::restore::RestoreArgs),
    /// Diagnose project stack, config, and suggestions
    Doctor(DoctorArgs),
    /// Build the checkout and install it over the running `appz` binary
    DevInstall(DevInstallArgs),
    /// Run a stdio MCP server exposing appz to AI agents
    Mcp,
    /// Deploy to a hosting platform (drives that platform's own CLI)
    Deploy(DeployArgs),
    /// Roll back a deployment on edge hosting to a previous version
    Rollback(RollbackArgs),
    /// Promote a ready deployment to production
    Promote(PromoteArgs),
    /// Link this directory to a project on the platform
    Link(LinkArgs),
    /// Point a custom domain or alias to the deployment
    #[command(args_conflicts_with_subcommands = true)]
    Alias(AliasArgs),
    /// List deployments for a project, or for the whole team
    #[command(alias = "deployments", alias = "ls")]
    List(ListArgs),
    /// Inspect edge deployment manifest, hosts, and CAS metrics
    Inspect(InspectArgs),
    /// Open the production site, a deployment, or the project dashboard in the browser
    Open(OpenArgs),
    /// Delete deployments by ID or URL (refused while an alias points at one)
    #[command(alias = "rm")]
    Remove(RemoveArgs),
    /// Manage custom domains and DNS verification on Cloudflare Edge
    #[command(alias = "domain")]
    Domains(DomainsArgs),
    /// Manage appz.dev webhook endpoints and verify signatures
    #[command(alias = "webhook")]
    Webhooks(webhooks::WebhooksArgs),
    /// Manage URL redirects with staged versions (list, add, rm, import, promote)
    #[command(alias = "redirect")]
    Redirects(RedirectsArgs),
    /// Manage projects on the edge hosting platform
    #[command(alias = "project")]
    Projects(ProjectsArgs),
    /// Inspect, accept or cancel project transfers between organizations
    Transfers(TransfersArgs),
    /// Log in to Appz via browser device flow
    Login(LoginArgs),
    /// Log out of Appz and remove the stored session token
    Logout,
    /// Print currently authenticated user and active organization
    Whoami,
    /// List organization audit events (owner session or `activity:read` key)
    Activity(ActivityArgs),
    /// Show resource usage against plan limits, plus plan features
    Usage(UsageArgs),
    /// Store credentials in an encrypted vault used by `appz deploy`
    Env(EnvArgs),
    /// Manage the encrypted credential vault used by `appz deploy`
    Vault(VaultArgs),
    /// Write a GitHub Actions workflow that deploys this project
    Ci(CiArgs),
    /// Discover and install agent skills relevant to this project
    Skills(SkillsArgs),
    /// Detect installed AI agents and diagnose their config (skills: `appz skills`)
    Agents(AgentArgs),
    /// Self-update to the latest (or a specific) release
    Update(UpdateArgs),
    /// Manage toolchain registry
    Toolchain(ToolchainArgs),
}

#[derive(Args)]
struct ToolchainArgs {
    #[command(subcommand)]
    command: ToolchainCmd,
}

#[derive(Subcommand)]
enum ToolchainCmd {
    /// Refresh the toolchain registry cache from GitHub
    Refresh,
}

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
struct SkillsArgs {
    #[command(subcommand)]
    sub: Option<SkillsSub>,
    /// Search skills.sh directly instead of auto-detecting the project
    query: Option<String>,
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Also offer skills from large source repos (slow: full-repo download)
    #[arg(long)]
    include_large: bool,
}

#[derive(Subcommand)]
enum SkillsSub {
    /// Install appz's own agent skill into every detected AI agent
    Add {
        /// Install user-wide (home directory) instead of into this project
        #[arg(long)]
        global: bool,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
}

#[derive(Args)]
struct AgentArgs {
    #[command(subcommand)]
    action: AgentAction,
    /// Project directory to operate on
    #[arg(long, global = true, default_value = ".")]
    dir: PathBuf,
}

#[derive(clap::Subcommand)]
enum AgentAction {
    /// List all known agents and detect which are installed
    List,
    /// Check agent configuration and installed skills
    Doctor,
}

#[derive(Args)]
struct VaultArgs {
    #[command(subcommand)]
    command: VaultCmd,
}

#[derive(Subcommand)]
enum VaultCmd {
    /// Store a secret (omit VALUE to be prompted, keeping it out of shell history)
    Set {
        name: String,
        value: Option<String>,
        /// Use the user-wide vault (~/.appz) instead of this project's
        #[arg(long)]
        global: bool,
        /// Day the credential stops working at its issuer (YYYY-MM-DD, UTC)
        #[arg(long, conflicts_with = "ttl")]
        expires: Option<String>,
        /// Time until it stops working, e.g. 90d, 2w, 12h
        #[arg(long)]
        ttl: Option<String>,
    },
    /// List stored names with their recorded expiry (never values)
    Ls {
        #[arg(long)]
        global: bool,
    },
    /// Record or change the expiry of a stored secret without touching its value
    Expire {
        name: String,
        /// Day the credential stops working at its issuer (YYYY-MM-DD, UTC)
        #[arg(long, conflicts_with = "ttl")]
        expires: Option<String>,
        /// Time until it stops working, e.g. 90d, 2w, 12h
        #[arg(long)]
        ttl: Option<String>,
        /// Use the user-wide vault (~/.appz) instead of this project's
        #[arg(long)]
        global: bool,
    },
    /// Exit non-zero when a stored secret is expired or about to expire
    Check {
        /// Warn this long before expiry, e.g. 14d, 2w
        #[arg(long, default_value = "14d")]
        within: String,
        #[arg(long)]
        global: bool,
    },
    /// Remove a secret
    Rm {
        name: String,
        #[arg(long)]
        global: bool,
    },
    /// Unlock the vault (creating it on first use) for this login session
    Unlock {
        #[arg(long)]
        global: bool,
    },
    /// Forget the cached unlock
    Lock {
        #[arg(long)]
        global: bool,
    },
}

#[derive(Args)]
struct EnvArgs {
    #[command(subcommand)]
    command: EnvCmd,
}

#[derive(Subcommand)]
enum EnvCmd {
    #[command(flatten)]
    Vault(VaultCmd),
    /// Pull decrypted cloud variables into a local .env file (needs the env:decrypt scope)
    Pull {
        /// Target file to write to (default: .env.local)
        #[arg(default_value = ".env.local")]
        file: PathBuf,
        /// Environment to pull: production, preview, development or a custom one; repeatable,
        /// later ones win on conflicts (default: development, like `vercel env pull`)
        #[arg(long = "environment", alias = "target", value_delimiter = ',')]
        environment: Vec<String>,
    },
    /// Push local environment variables from a .env file to the cloud
    Push {
        /// Source file to read from (default: .env.local, falls back to .env)
        #[arg(default_value = ".env.local")]
        file: PathBuf,
        /// Environments to set the variables in; repeatable (default: production, preview and development)
        #[arg(long = "environment", alias = "target", value_delimiter = ',')]
        environment: Vec<String>,
    },
    /// Run a command with cloud variables injected (never written to disk or printed)
    Run {
        /// Environment to load; repeatable, later ones win (default: development)
        #[arg(long = "environment", alias = "target", value_delimiter = ',')]
        environment: Vec<String>,
        /// Command and arguments, after `--`
        #[arg(last = true, required = true, value_name = "CMD")]
        cmd: Vec<String>,
    },
    /// List or remove cloud variables (the local vault is `appz env ls` / `rm`)
    Cloud {
        #[command(subcommand)]
        command: CloudEnvCmd,
    },
    /// Manage custom environments beyond production, preview and development
    Environments {
        #[command(subcommand)]
        command: EnvironmentsCmd,
    },
}
#[derive(Subcommand)]
enum CloudEnvCmd {
    /// List cloud variable keys and metadata (never values)
    Ls {
        /// Only variables in this environment; repeatable
        #[arg(long = "environment", value_delimiter = ',')]
        environment: Vec<String>,
    },
    /// Remove a cloud variable from EVERY environment (per-environment removal is not supported)
    Rm { key: String },
}
#[derive(Subcommand)]
enum EnvironmentsCmd {
    /// List custom environments
    Ls,
    /// Create a custom environment
    Add {
        slug: String,
        /// Git branch pattern deployed to this environment, e.g. "release/*"
        #[arg(long)]
        branch_pattern: Option<String>,
    },
    /// Remove a custom environment and its variable targets (asks unless --yes)
    Rm { slug: String },
}
#[derive(Args)]
struct CiArgs {
    #[command(subcommand)]
    command: CiCmd,
}

#[derive(Subcommand)]
enum CiCmd {
    /// Write .github/workflows/appz-deploy.yml for this project
    Init(CiInitArgs),
}

#[derive(Args)]
struct CiInitArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug to pin in the workflow (default: appz.jsonc deploy.default, else detected)
    #[arg(long)]
    target: Option<String>,
    /// Branch that triggers a production deploy (default: the checked-out branch)
    #[arg(long)]
    branch: Option<String>,
    /// Overwrite an existing workflow
    #[arg(long)]
    force: bool,
    /// Write the workflow but do not set repository secrets
    #[arg(long, conflicts_with = "yes")]
    no_secrets: bool,
}

#[derive(Args)]
struct DeployArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (appzdev, vercel, netlify, cloudflare, fly, firebase, deno-deploy, azure-static, aws-s3, aws-sst, railway, surge, github-pages, render). Uses appz.jsonc's deploy.default if omitted, else detects it from project files.
    #[arg(long)]
    target: Option<String>,
    /// Platform project name to create or link (default: asked, suggesting one)
    #[arg(long)]
    project: Option<String>,
    /// Production deploy (default: preview)
    #[arg(long)]
    prod: bool,
    /// Runtime env var, repeatable (-e KEY=VALUE)
    #[arg(short = 'e', long = "env", value_name = "KEY=VALUE")]
    env: Vec<String>,
    /// Build-time env var, repeatable (-b KEY=VALUE)
    #[arg(short = 'b', long = "build-env", value_name = "KEY=VALUE")]
    build_env: Vec<String>,
    /// Show what would be deployed without deploying
    #[arg(long)]
    dry_run: bool,
    /// Deploy even if nothing changed since the last deploy of this target
    #[arg(long)]
    force: bool,
    /// Deploy the existing build output as-is: skip install and build. Fails if the output directory is missing or empty (run `appz build` first) or the target builds itself (render, fly, railway, ...)
    #[arg(long)]
    prebuilt: bool,
    /// Deploy to every target configured under deploy.targets instead of one
    #[arg(long, conflicts_with_all = ["target", "project"])]
    all: bool,
    /// Skip the automatic source backup after a successful deploy
    #[arg(long)]
    no_backup: bool,
}

#[derive(Args)]
struct RollbackArgs {
    /// Version ID, deployment ID or deployment URL to roll back to. Omit to
    /// roll back to the previous ready production deployment. A lone existing
    /// directory is treated as DIR.
    version: Option<String>,
    /// Project directory
    dir: Option<PathBuf>,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct LinkArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Platform project name or ID to link to
    #[arg(long)]
    project: Option<String>,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

/// `appz alias <DOMAIN> [DIR]` is shorthand for `appz alias set <DOMAIN> [DIR]`.
#[derive(Args)]
struct AliasArgs {
    #[command(subcommand)]
    command: Option<AliasCmd>,
    /// Custom domain or alias to assign (e.g. app.example.com)
    domain: Option<String>,
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Subcommand, Debug)]
enum AliasCmd {
    /// Point a custom domain or alias at the project's latest deployment
    Set {
        /// Custom domain or alias to assign (e.g. app.example.com)
        domain: String,
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Provider slug (default: detects from project or config)
        #[arg(long)]
        target: Option<String>,
    },
    /// List the hosts serving the project
    #[command(alias = "list")]
    Ls {
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Provider slug (default: detects from project or config)
        #[arg(long)]
        target: Option<String>,
    },
    /// Remove an alias (the production host cannot be removed)
    #[command(alias = "remove")]
    Rm {
        /// Alias host to remove (e.g. beta.example.com)
        host: String,
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Provider slug (default: detects from project or config)
        #[arg(long)]
        target: Option<String>,
    },
}

#[derive(Args)]
struct ActivityArgs {
    /// Maximum events per page (1-100)
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
    limit: Option<u32>,
    /// Cursor from a previous page
    #[arg(long)]
    next: Option<String>,
    /// Only show events with this exact action (filters the fetched page client-side)
    #[arg(long)]
    action: Option<String>,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct UsageArgs {
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct ListArgs {
    /// Project id or slug, or a project directory (an existing directory is treated as a
    /// directory, anything else as a project name). Defaults to the linked project of the
    /// current directory, else the whole team.
    project: Option<String>,
    /// List deployments for the whole team even when this directory is linked
    #[arg(long)]
    all: bool,
    /// Only production deployments
    #[arg(long, conflicts_with = "environment")]
    prod: bool,
    /// Only deployments in this environment (production, preview or a custom slug)
    #[arg(long)]
    environment: Option<String>,
    /// Statuses to include: ready, building, error, canceled (comma-separated or repeated)
    #[arg(long, value_delimiter = ',')]
    status: Vec<String>,
    /// Only deployments created since this time: 7d, 12h, YYYY-MM-DD, RFC 3339
    #[arg(long)]
    since: Option<String>,
    /// Only deployments created before this time (same formats as --since)
    #[arg(long)]
    until: Option<String>,
    /// Filter by metadata, key=value (repeatable)
    #[arg(long = "meta", value_name = "KEY=VALUE")]
    meta: Vec<String>,
    /// Maximum deployments to return (1-100)
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
    limit: Option<u32>,
    /// Cursor from a previous page
    #[arg(long)]
    next: Option<String>,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct RemoveArgs {
    /// Deployment IDs or live URLs to delete
    #[arg(required = true)]
    deployments: Vec<String>,
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    dir: PathBuf,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct PromoteArgs {
    /// Deployment ID or live URL to promote
    id_or_url: String,
    /// Project directory (defaults to current directory)
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct InspectArgs {
    /// Deployment ID or live URL to inspect
    id_or_url: String,
    /// Project directory (defaults to current directory)
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Also display all files contained in the deployment manifest
    #[arg(long)]
    files: bool,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct OpenArgs {
    /// Deployment ID or URL to open (defaults to the production site)
    id_or_url: Option<String>,
    /// Project directory (defaults to current directory)
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Open the project's (or the given deployment's) page on the my.appz.dev dashboard instead
    #[arg(long)]
    dashboard: bool,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct DomainsArgs {
    #[command(subcommand)]
    command: DomainsCmd,
    /// Project directory (defaults to current directory)
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Subcommand, Debug)]
enum DomainsCmd {
    /// Add a custom domain to the project and display DNS configuration records
    Add {
        /// Custom domain name (e.g. app.example.com or example.com)
        domain: String,
    },
    /// List custom domains for the whole team (use --project to filter)
    #[command(alias = "ls")]
    List {
        /// Only domains of this project (id or slug)
        #[arg(long)]
        project: Option<String>,
        /// Cursor from a previous page
        #[arg(long)]
        next: Option<String>,
    },
    /// Trigger DNS ownership and SSL verification check for a domain
    Verify {
        /// Domain name to verify
        domain: String,
    },
    /// Remove a custom domain from the project
    #[command(alias = "rm")]
    Remove {
        /// Domain name to remove
        domain: String,
    },
    /// Show a domain's project, verification state, required DNS records and serving status
    Inspect {
        /// Domain name to inspect
        domain: String,
    },
    /// Move a domain to a project in another organization (owners of both only)
    Move {
        /// Domain name to move
        domain: String,
        /// Destination organization id
        #[arg(long, value_name = "ORG")]
        to: String,
        /// Destination project id (in the destination organization)
        #[arg(long)]
        project: String,
    },
}

#[derive(Args)]
struct RedirectsArgs {
    #[command(subcommand)]
    command: RedirectsCmd,
    /// Project directory (defaults to current directory)
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Subcommand, Debug)]
enum RedirectsCmd {
    /// List redirects (staged version if there is one, otherwise live)
    #[command(alias = "list")]
    Ls {
        /// Show this version instead
        #[arg(long)]
        version: Option<u64>,
        /// Only redirects whose source or destination contains this text
        #[arg(long)]
        search: Option<String>,
    },
    /// Add a redirect in a new staged version
    Add {
        /// Path to redirect from, starting with `/`
        source: String,
        /// Where to redirect to
        destination: String,
        /// Redirect status code: 301, 302, 307 or 308
        #[arg(long, default_value_t = 307)]
        status: u16,
    },
    /// Remove a redirect (by ID or source) in a new staged version
    #[command(alias = "remove")]
    Rm {
        /// Redirect ID (see `redirects ls`) or its source
        id: String,
    },
    /// Import redirects from a .json or .csv file as a new staged version
    #[command(alias = "upload")]
    Import {
        /// File with `source`, `destination` and optional `status` per redirect
        file: PathBuf,
    },
    /// List redirect versions and which are live and staged
    #[command(alias = "list-versions")]
    Versions,
    /// Make a staged version live
    Promote {
        /// Version number (see `redirects versions`)
        version: u64,
    },
    /// Restore an earlier version
    Restore {
        /// Version number (see `redirects versions`)
        version: u64,
    },
}

#[derive(Args)]
struct ProjectsArgs {
    #[command(subcommand)]
    command: ProjectsCmd,
    /// Project directory (defaults to current directory)
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Subcommand, Debug)]
enum ProjectsCmd {
    /// List all projects in the active organization
    #[command(alias = "ls")]
    List,
    /// Create a new project on the platform
    Add {
        /// Name of the project
        name: String,
    },
    /// Inspect or get details of a project
    #[command(alias = "get")]
    Inspect {
        /// Name or ID of the project
        name: String,
    },
    /// Rename a project's display name
    Rename {
        /// Name or ID of the project to rename
        name: String,
        /// New display name for the project
        new_name: String,
    },
    /// Remove a project from the platform
    #[command(alias = "rm")]
    Remove {
        /// Name or ID of the project to remove
        name: String,
    },
    /// Restore a hosted project by slug or ID within 30 days of deletion
    Restore { name: String },
    /// Take a project's production aliases offline (asks first)
    Pause { name: String },
    /// Bring a paused project's production aliases back
    Resume { name: String },
    /// Make a temporary project permanent
    Keep { name: String },
    /// Change project settings (team owners and admins only)
    Update {
        /// Slug or ID of the project
        project: String,
        /// New display name
        #[arg(long = "name")]
        new_name: Option<String>,
        /// Where the preview toolbar shows
        #[arg(long, value_parser = ["off", "previews", "all"])]
        toolbar_mode: Option<String>,
        /// Serve markdown versions of pages
        #[arg(long)]
        markdown: Option<bool>,
        /// Turn deployment protection off (true) or on (false)
        #[arg(long)]
        deployment_protection_disabled: Option<bool>,
    },
    /// Request a transfer to another organization and preview what moves
    Transfer {
        /// Slug or ID of the project
        name: String,
        /// Destination organization ID
        #[arg(long)]
        to: String,
    },
}

#[derive(Args)]
struct TransfersArgs {
    #[command(subcommand)]
    command: TransfersCmd,
    /// Project directory (defaults to current directory)
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Subcommand, Debug)]
enum TransfersCmd {
    /// List incoming and outgoing transfers
    #[command(alias = "list")]
    Ls,
    /// Show a transfer and what it moves
    Inspect { id: String },
    /// Accept an incoming transfer (asks first)
    Accept {
        id: String,
        /// Seconds to wait while the server finishes reconciling
        #[arg(long, default_value_t = 120)]
        timeout: u64,
    },
    /// Cancel a pending transfer (asks first)
    #[command(alias = "rm")]
    Cancel { id: String },
}

#[derive(Args)]
struct LoginArgs {
    /// Skip opening browser automatically
    #[arg(long)]
    no_browser: bool,
}

#[derive(Args)]
struct UpdateArgs {
    /// Install this version instead of the latest (e.g. 0.3.0 or v0.3.0)
    version: Option<String>,
    /// Only report whether a newer version is available; don't install it
    #[arg(long)]
    check: bool,
    /// Suppress progress output
    #[arg(long)]
    quiet: bool,
}

#[derive(Args)]
struct DevInstallArgs {
    #[arg(long, help = "Build in debug mode instead of the default --release")]
    debug: bool,
    #[arg(
        long,
        help = "Build and verify, but report what would be installed without replacing"
    )]
    dry_run: bool,
}

#[derive(Args)]
struct InitArgs {
    #[arg(
        default_value = ".",
        help = "Local path, or a github.com/gitlab.com/bitbucket.org URL to clone/download"
    )]
    source: String,
    #[arg(long, help = "Skip mise install after init")]
    skip_mise: bool,
    #[arg(long, help = "Skip package manager install after init")]
    skip_pm: bool,
    #[arg(long, help = "Generate CLAUDE.md from detected toolchains")]
    claude: bool,
    #[arg(
        long,
        help = "Overwrite the target directory if a remote source's destination already exists"
    )]
    force: bool,
}

#[derive(Args)]
struct InstallArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    #[arg(long, help = "Skip mise install")]
    skip_mise: bool,
    #[arg(long, help = "Skip package manager install")]
    skip_pm: bool,
}

#[derive(Args)]
struct TestArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    #[arg(short, long, help = "Override test command")]
    command: Option<String>,
    #[arg(long, help = "Skip auto-install before test")]
    skip_install: bool,
}

#[derive(Args)]
struct LintArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    #[arg(short, long, help = "Override lint command")]
    command: Option<String>,
    #[arg(long, help = "Skip auto-install before lint")]
    skip_install: bool,
}

#[derive(Args)]
struct FormatArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    #[arg(short, long, help = "Override format command")]
    command: Option<String>,
    #[arg(long, help = "Skip auto-install before format")]
    skip_install: bool,
}

#[derive(Args)]
struct CleanArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Also remove dependency dirs (node_modules, .venv) — expensive to regenerate
    #[arg(long)]
    deps: bool,
    /// Actually delete (default is a dry-run listing)
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct DoctorArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
}

#[derive(Args)]
struct BuildArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    #[arg(short, long, help = "Override build command")]
    command: Option<String>,
    #[arg(long, help = "Skip auto-install before build")]
    skip_install: bool,
}

#[derive(Args)]
struct DevArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    #[arg(short, long, help = "Override dev command")]
    command: Option<String>,
    #[arg(long, help = "Skip auto-install before dev")]
    skip_install: bool,
    /// Share dev server publicly over a secure tunnel
    #[arg(long, help = "Share dev server publicly over a secure tunnel")]
    share: bool,
    /// Require a 4-digit PIN to access the shared dev server
    #[arg(
        long,
        value_name = "CODE",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "auto"
    )]
    pin: Option<String>,
    /// Require HTTP Basic Auth in format username:password
    #[arg(long, value_name = "USER:PASS")]
    auth: Option<String>,
    /// Request a specific subdomain on supported tunnel providers
    #[arg(short = 's', long, value_name = "SUBDOMAIN")]
    subdomain: Option<String>,
    /// With --share: print each request that comes in through the tunnel
    #[arg(long)]
    inspect: bool,
    /// Serve extensionless routes from root index.html (SPA mode)
    #[arg(long)]
    spa: bool,
    /// Disable file watching / hot-reload WebSocket for static HTML sites
    #[arg(long)]
    no_reload: bool,
}

#[derive(Args)]
struct PreviewArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    #[arg(long, value_name = "PATH")]
    serve_dir: Option<PathBuf>,
    #[arg(short, long, default_value_t = 4321)]
    port: u16,
    #[arg(long, help = "SPA fallback to index.html")]
    spa: bool,
    #[arg(long, help = "Skip detect/install before preview")]
    skip_install: bool,
    #[arg(long, help = "Skip mise run build before serving")]
    skip_build: bool,
}

// ── UI helpers ──────────────────────────────────────────────────

/// Shown once, at `appz init` — a clean, subtle header.
/// Skipped outside a real terminal so it never lands in piped/CI output.
fn print_brand() {
    if is_stdout_tty() {
        println!("\n  appz • universal toolchain & deployment\n");
    }
}

// ── Core logic ──────────────────────────────────────────────────

fn resolve_root(dir: &Path) -> PathBuf {
    match appz_core::canonicalize(dir) {
        Ok(p) => p,
        Err(e) => error(&format!("cannot resolve path '{}': {e}", dir.display())),
    }
}

// ── JSON output ─────────────────────────────────────────────────

fn json_out(toolchains: &[appz_core::DetectedToolchain], status: &str) {
    let r = serde_json::json!({
        "status": status,
        "toolchains": toolchains,
    });
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
}

// ── CLAUDE.md generator ────────────────────────────────

fn do_make_claude(root: &Path) {
    if root.join("CLAUDE.md").exists() {
        warning("CLAUDE.md exists — use `appz init --claude --force` to overwrite");
        return;
    }
    let report = run_doctor(root);
    let content = generate_claude_md(&report, root);
    with_spinner("generating CLAUDE.md...", "CLAUDE.md written", || {
        let _ = std::fs::write(root.join("CLAUDE.md"), content);
    });
}

// ── Init scaffolding ────────────────────────────────────────────

fn scaffold_init(root: &Path) {
    let appz_jsonc = root.join("appz.jsonc");
    if !appz_jsonc.exists() {
        let template = r#"{
  // Command overrides for appz
  // "buildCommand": "npm run build",
  // "installCommand": "npm install",
  // "devCommand": "npm run dev"
}
"#;
        let _ = std::fs::write(&appz_jsonc, template);
    }
}

fn run_cmd(program: &str, args: &[&str], dir: &std::path::Path) -> bool {
    // Shell-wrapped (cmd /C on Windows) so npm's `.cmd`/`.ps1` shims resolve —
    // std::process::Command's direct CreateProcess call won't find them even
    // with the right PATH, since only a real shell does PATHEXT lookup.
    let mut cmd = command::Command::new(program);
    cmd.args(args)
        .cwd(dir)
        .prepend_paths(appz_core::find_node_modules_bin_paths(dir));
    match cmd.exec_interactive() {
        Ok(status) if status.success() => true,
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(e) => error(&format!("failed to run '{program}': {e}")),
    }
}

fn do_detect_and_write(root: &Path) -> Vec<appz_core::DetectedToolchain> {
    let toolchains = with_spinner("detecting toolchains...", "toolchains detected", || {
        let tc =
            detect_toolchains(root).unwrap_or_else(|e| error(&format!("detection failed: {e}")));
        if tc.is_empty() {
            error(&format!("no toolchains detected in '{}'", root.display()));
        }
        tc
    });

    if let Err(e) = with_spinner("writing project state...", "state written", || {
        appz_core::write_state(root, &toolchains)
    }) {
        error(&format!("cannot write project state: {e}"));
    }

    for tc in &toolchains {
        let fw = if tc.frameworks.is_empty() {
            String::new()
        } else {
            let names: Vec<&str> = tc.frameworks.iter().map(|f| f.name).collect();
            format!(" — {}", names.join(", "))
        };
        info(&format!(
            "{} ({} @ {}){}",
            tc.name, tc.mise_plugin, tc.version, fw
        ));
    }

    toolchains
}

fn do_mise_install(root: &Path) -> bool {
    if let Err(e) = appz_core::ensure_mise(root) {
        warning(&e);
        return false;
    }
    if let Err(e) = appz_core::trust(root) {
        warning(&format!("mise trust: {e}"));
    }

    with_spinner("installing mise tools...", "mise tools ready", || {
        run_cmd("mise", &["install"], root)
    })
}

fn do_pm_install(root: &Path, toolchains: &[appz_core::DetectedToolchain]) {
    let has_node = toolchains.iter().any(|t| t.slug == "node");
    let has_rust = toolchains.iter().any(|t| t.slug == "rust");

    if has_node {
        let pm = appz_core::detect_package_manager(root).unwrap_or("npm");
        let cmd = appz_core::pm_install_cmd(pm);
        let parts: Vec<&str> = cmd.split(' ').collect();
        let (prog, args) = parts.split_first().unwrap_or((&"npm", &[]));
        run_cmd(prog, args, root);
    }
    if has_rust {
        run_cmd("cargo", &["build"], root);
    }
}

/// Like `do_pm_install` but through mise's cached `install` task, so an
/// unchanged lockfile/manifest is not reinstalled on every run.
fn do_cached_pm_install(root: &Path, toolchains: &[appz_core::DetectedToolchain]) {
    let mise_ready = do_mise_install(root);
    install_dependencies(root, toolchains, mise_ready);
}

fn install_dependencies(
    root: &Path,
    toolchains: &[appz_core::DetectedToolchain],
    mise_ready: bool,
) {
    let has_task = toolchains.iter().any(|t| t.install_command.is_some());
    if !has_task {
        do_pm_install(root, toolchains);
    } else if mise_ready {
        run_cmd("mise", cached_install_args(root, toolchains), root);
    } else {
        let mut seen = std::collections::HashSet::new();
        for cmd in toolchains
            .iter()
            .filter_map(|t| t.install_command.as_deref())
        {
            if seen.insert(cmd) {
                run_cmd(cmd, &[], root);
            }
        }
    }
}

fn cached_install_args(
    root: &Path,
    toolchains: &[appz_core::DetectedToolchain],
) -> &'static [&'static str] {
    // The task cache can survive a deleted dependency directory.
    if toolchains
        .iter()
        .any(|t| t.mise_plugin == "node" && t.install_command.is_some())
        && !root.join("node_modules").is_dir()
        && !root.join(".pnp.cjs").is_file()
        && !root.join(".pnp.js").is_file()
    {
        &["run", "--force", "install"]
    } else {
        &["run", "install"]
    }
}

fn do_full_install(root: &Path) -> Vec<appz_core::DetectedToolchain> {
    let toolchains = do_detect_and_write(root);
    do_mise_install(root);
    do_pm_install(root, &toolchains);
    toolchains
}

// ── Command impls ───────────────────────────────────────────────

fn run_init(args: InitArgs, json: bool) {
    if json {
        let resolved = init_source::resolve(&args.source, args.force).unwrap_or_else(|e| error(&e));
        let canonical = resolve_root(&resolved);
        let toolchains = do_detect_and_write(&canonical);
        scaffold_init(&canonical);
        if args.claude {
            do_make_claude(&canonical);
        }
        if !args.skip_mise {
            do_mise_install(&canonical);
        }
        if !args.skip_pm {
            do_pm_install(&canonical, &toolchains);
        }
        json_out(&toolchains, "ok");
        return;
    }
    print_brand();
    intro("appz init");
    let resolved = init_source::resolve(&args.source, args.force).unwrap_or_else(|e| error(&e));
    let canonical = resolve_root(&resolved);
    let toolchains = do_detect_and_write(&canonical);
    scaffold_init(&canonical);
    if args.claude {
        do_make_claude(&canonical);
    }
    if !args.skip_mise {
        do_mise_install(&canonical);
    }
    if !args.skip_pm {
        do_pm_install(&canonical, &toolchains);
    }
    outro("project ready");
}

fn run_install(args: InstallArgs, json: bool) {
    if json {
        let canonical = resolve_root(&args.dir);
        let toolchains = do_detect_and_write(&canonical);
        if !args.skip_mise {
            do_mise_install(&canonical);
        }
        if !args.skip_pm {
            do_pm_install(&canonical, &toolchains);
        }
        json_out(&toolchains, "ok");
        return;
    }
    intro("appz install");
    let canonical = resolve_root(&args.dir);

    if !args.skip_mise && appz_core::inputs_unchanged(&canonical) && args.skip_pm {
        success("inputs unchanged, tools already up to date");
        outro("nothing to install");
        return;
    }

    let toolchains = do_detect_and_write(&canonical);
    if !args.skip_mise {
        do_mise_install(&canonical);
    }
    if !args.skip_pm {
        do_pm_install(&canonical, &toolchains);
    }
    outro("dependencies ready");
}

fn run_build(args: BuildArgs, json: bool) {
    if json {
        let canonical = resolve_root(&args.dir);
        let toolchains = do_detect_and_write(&canonical);
        if !args.skip_install {
            do_cached_pm_install(&canonical, &toolchains);
        }
        json_out(&toolchains, "ok");
        return;
    }
    intro("appz build");
    let canonical = resolve_root(&args.dir);

    // Detect + write mise.toml ([tools] + [tasks] with sources/outputs) so
    // `mise run build` below has an up-to-date task to skip or run. mise's
    // own task cache decides "inputs unchanged" now — appz doesn't hand-roll
    // that anymore (see `appz_core::generator::generate_merged`).
    let toolchains = do_detect_and_write(&canonical);
    if !args.skip_install {
        do_cached_pm_install(&canonical, &toolchains);
    }

    match args.command {
        Some(cmd) => {
            step("building...");
            let parts: Vec<&str> = cmd.split(' ').collect();
            let (prog, cmd_args) = parts.split_first().unwrap_or((&"npm", &[]));
            if !run_cmd(prog, cmd_args, &canonical) {
                std::process::exit(1);
            }
        }
        None => {
            if toolchains
                .iter()
                .any(|tc| tc.slug == appz_core::STATIC_HTML_SLUG)
            {
                match appz_core::static_prebuild_ready(&canonical, &toolchains) {
                    Ok(Some(())) => {
                        success("static HTML — output already on disk (no build step)");
                        outro("build complete");
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => error(&e),
                }
            }
            if !toolchains.iter().any(|tc| tc.build_command.is_some()) {
                error("no build command detected — use --command to specify one");
            }
            step("building (mise run build)...");
            if !run_cmd("mise", &["run", "build"], &canonical) {
                std::process::exit(1);
            }
        }
    }
    outro("build complete");
}

fn run_dev(args: DevArgs, json: bool) {
    if !json {
        intro("appz dev");
    }
    let canonical = resolve_root(&args.dir);
    let toolchains = do_detect_and_write(&canonical);
    if !args.skip_install {
        do_cached_pm_install(&canonical, &toolchains);
    }
    if json {
        json_out(&toolchains, "ok");
        return;
    }

    let cmds: Vec<String> = match args.command.clone() {
        Some(c) => vec![c],
        None => {
            let mut cmds = Vec::new();
            for tc in &toolchains {
                if let Some(ref d) = tc.dev_command {
                    cmds.push(d.to_string());
                }
            }
            if cmds.is_empty() {
                error("no dev command detected — use --command to specify one");
            }
            cmds
        }
    };

    let frameworks: Vec<&str> = toolchains
        .iter()
        .flat_map(|t| t.frameworks.iter().map(|f| f.name))
        .collect();
    let sharing = start_dev_share(&args, &canonical, &frameworks, &cmds);
    step("starting dev server...");
    run_dev_commands(
        &cmds,
        &canonical,
        sharing,
        appz_core::detect_dev_port(&canonical, &frameworks, cmds.first().map(String::as_str)),
        static_dev_options(&cmds, args.no_reload, args.spa),
    );
}

/// `appz dev --share`: pick the tunnel backend now, while the terminal is
/// free to ask about installing one; the tunnel opens once the server is up.
fn start_dev_share(
    args: &DevArgs,
    root: &Path,
    frameworks: &[&str],
    cmds: &[String],
) -> Option<share::DevShare> {
    if !args.share {
        return None;
    }
    if let Some(auth) = &args.auth
        && let Err(e) = share::validate_basic_auth(auth)
    {
        error(&e);
    }
    let install = if assume_yes() {
        deploy::Consent::Yes
    } else if is_tty() {
        deploy::Consent::Ask
    } else {
        deploy::Consent::Never
    };
    let backend = share::resolve_backend(root, install).unwrap_or_else(|e| error(&e));
    let port = appz_core::detect_dev_port(root, frameworks, cmds.first().map(String::as_str));
    let mut shield = shield::ShieldConfig::new(port);
    shield.pin = args.pin.as_deref().map(share::resolve_pin);
    shield.basic_auth = args.auth.clone();
    if let Some(pin) = &shield.pin {
        info(&format!("shield mode: PIN {pin}"));
    }
    step(&format!("share: waiting for the dev server on port {port}"));
    Some(share::DevShare::start(backend, shield, args.inspect))
}

fn static_dev_options(cmds: &[String], no_reload: bool, spa: bool) -> Option<(bool, bool)> {
    cmds.first()
        .filter(|c| static_serve::is_static_serve_command(c))
        .map(|_| (!no_reload, spa))
}

/// Run the dev commands to completion, then close the share tunnel (also
/// when a command fails, which exits non-zero).
fn run_dev_commands(
    cmds: &[String],
    dir: &Path,
    sharing: Option<share::DevShare>,
    port: u16,
    static_dev: Option<(bool, bool)>,
) {
    let ok = if cmds.len() == 1
        && static_serve::is_static_serve_command(&cmds[0])
        && let Some((hot_reload, spa)) = static_dev
    {
        static_serve::run_static_server(dir, port, hot_reload, spa).is_ok()
    } else {
        cmds.iter().all(|cmd| {
            let parts: Vec<&str> = cmd.split(' ').collect();
            let (prog, args) = parts.split_first().unwrap_or((&"npm", &[]));
            run_cmd(prog, args, dir)
        })
    };
    if let Some(sharing) = sharing {
        sharing.stop();
    }
    if !ok {
        std::process::exit(1);
    }
    outro("dev server stopped");
}

fn run_preview_build(project: &std::path::Path, toolchains: &[appz_core::DetectedToolchain]) {
    if !toolchains.iter().any(|tc| tc.build_command.is_some()) {
        return;
    }
    step("building (mise run build)...");
    if !run_cmd("mise", &["run", "build"], project) {
        std::process::exit(1);
    }
}
fn run_preview(args: PreviewArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    let toolchains = if args.skip_install {
        do_detect_and_write(&canonical)
    } else {
        do_full_install(&canonical)
    };

    if json {
        let serve_root =
            appz_core::resolve_preview_root(&canonical, &toolchains, args.serve_dir.as_deref())
                .unwrap_or_else(|e| error(&e));
        json_out(&toolchains, "ok");
        println!(
            "{}",
            serde_json::json!({"serve_root": serve_root.to_string_lossy(), "port": args.port, "hot_reload": false})
        );
        return;
    }

    intro("appz preview");
    if !args.skip_build {
        step("preparing production-like output...");
        run_preview_build(&canonical, &toolchains);
    }
    let serve_root =
        appz_core::resolve_preview_root(&canonical, &toolchains, args.serve_dir.as_deref())
            .unwrap_or_else(|e| error(&e));
    success(&format!("serving built output {}", serve_root.display()));
    step("starting preview server (no hot reload)...");
    if static_serve::run_preview_server(&serve_root, args.port, args.spa).is_err() {
        std::process::exit(1);
    }
    outro("preview server stopped");
}

fn run_share(args: share::ShareArgs, json: bool) {
    if let Some(ref auth) = args.auth
        && let Err(e) = share::validate_basic_auth(auth)
    {
        error(&e);
    }
    let ttl = match share::parse_ttl_duration(&args.ttl) {
        Ok(secs) => std::time::Duration::from_secs(secs),
        Err(e) => error(&e),
    };
    let target_port = share::parse_target_port(args.port.as_deref()).ok();
    if json {
        let config = share::ShareConfigJson {
            target_port,
            pin_protected: args.pin.is_some(),
            pin: args.pin.clone(),
            auth_required: args.auth.is_some(),
            subdomain: args.subdomain.clone(),
            ttl: args.ttl.clone(),
            inspect: args.inspect,
        };
        println!("{}", serde_json::to_string_pretty(&config).unwrap());
        return;
    }
    intro("appz share");
    let canonical = resolve_root(&args.dir);
    let port = match args.port.as_deref() {
        Some(raw) => {
            let port = share::parse_target_port(Some(raw)).unwrap_or_else(|e| error(&e));
            if !appz_core::is_port_listening(port, 200) {
                warning(&format!(
                    "nothing is listening on port {port} yet; visitors get a 502 until it is"
                ));
            }
            port
        }
        None => {
            // The last detection run knows the frameworks and dev command.
            let snap = appz_core::read_latest_snapshot(&canonical);
            let toolchains = snap.as_ref().map_or(&[][..], |s| &s.toolchains[..]);
            let frameworks: Vec<&str> = toolchains
                .iter()
                .flat_map(|t| t.frameworks.iter().map(String::as_str))
                .collect();
            let dev_command = toolchains.iter().find_map(|t| t.dev_command.as_deref());
            let port = appz_core::detect_dev_port(&canonical, &frameworks, dev_command);
            if !appz_core::is_port_listening(port, 200) {
                let start = assume_yes()
                    || is_tty()
                        && cliclack::confirm(format!(
                            "No dev server is running (tried port {port}). Start it and share it?"
                        ))
                        .initial_value(true)
                        .interact()
                        .unwrap_or(false);
                if !start {
                    error(&format!(
                        "no dev server found (tried port {port}). Start it and run `appz share <PORT>`, or use `appz dev --share`"
                    ));
                }
                // The dev server needs the terminal for its own output, so
                // this is `appz dev --share` with the same options.
                return run_dev(
                    DevArgs {
                        dir: args.dir,
                        command: None,
                        skip_install: false,
                        share: true,
                        pin: args.pin,
                        auth: args.auth,
                        subdomain: args.subdomain,
                        inspect: args.inspect,
                        spa: false,
                        no_reload: false,
                    },
                    false,
                );
            }
            step(&format!("detected dev server on port {port}"));
            port
        }
    };
    share::run_session(&args, &canonical, port, ttl);
}

macro_rules! lifecycle_fn {
    ($name:ident, $ty:ty, $field:ident, $label:literal) => {
        fn $name(args: $ty, json: bool) {
            let canonical = resolve_root(&args.dir);

            if json {
                let toolchains = do_detect_and_write(&canonical);
                if !args.skip_install {
                    do_mise_install(&canonical);
                    do_pm_install(&canonical, &toolchains);
                }
                json_out(&toolchains, "ok");
                return;
            }

            intro(concat!("appz ", $label));

            let canonical = resolve_root(&args.dir);

            // Cache hit
            if !args.command.is_some()
                && !args.skip_install
                && appz_core::inputs_unchanged(&canonical)
            {
                if let Some(snap) = appz_core::read_latest_snapshot(&canonical) {
                    let cmds: Vec<String> = snap
                        .toolchains
                        .iter()
                        .filter_map(|t| t.$field.clone())
                        .collect();
                    if !cmds.is_empty() {
                        success("inputs unchanged — running cached command");
                        for cmd in &cmds {
                            let parts: Vec<&str> = cmd.split(' ').collect();
                            let (prog, args) = parts.split_first().unwrap_or((&"npm", &[]));
                            if !run_cmd(prog, args, &canonical) {
                                std::process::exit(1);
                            }
                        }
                        outro(concat!($label, " complete"));
                        return;
                    }
                }
            }

            // Full pipeline
            let toolchains = {
                if !args.skip_install {
                    do_full_install(&canonical)
                } else {
                    do_detect_and_write(&canonical)
                }
            };

            let cmds: Vec<String> = match args.command {
                Some(c) => vec![c],
                None => {
                    let mut cmds = Vec::new();
                    for tc in &toolchains {
                        if let Some(ref b) = tc.$field {
                            cmds.push(b.to_string());
                        }
                    }
                    if cmds.is_empty() {
                        error(&format!(
                            "no {} command detected — use --command to specify one",
                            $label
                        ));
                    }
                    cmds
                }
            };

            step(concat!("running ", $label, "..."));
            for cmd in &cmds {
                let parts: Vec<&str> = cmd.split(' ').collect();
                let (prog, args) = parts.split_first().unwrap_or((&"npm", &[]));
                if !run_cmd(prog, args, &canonical) {
                    std::process::exit(1);
                }
            }
            outro(concat!($label, " complete"));
        }
    };
}

lifecycle_fn!(run_test, TestArgs, test_command, "test");
lifecycle_fn!(run_lint, LintArgs, lint_command, "lint");
lifecycle_fn!(run_format, FormatArgs, format_command, "format");

fn run_doctor_cmd(args: DoctorArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    let report = run_doctor(&canonical);

    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    intro("appz doctor");

    // cliclack's log::info/step each end their own block with a trailing
    // blank line — one call per field (the old shape) meant one blank line
    // per field. Batching a section into a single multi-line call keeps one
    // bullet + continuation bars for the whole section, with only one
    // trailing blank at the end of it.
    step("toolchains");
    let toolchain_lines: Vec<String> = report
        .toolchains
        .iter()
        .map(|tc| {
            let fw: Vec<&str> = tc.frameworks.iter().map(|f| f.name).collect();
            let fw_str = if fw.is_empty() {
                String::new()
            } else {
                format!(" [{}]", fw.join(", "))
            };
            let ver = if tc.version.is_empty() {
                String::new()
            } else {
                format!(" @{}", tc.version)
            };
            let build = tc.build_command.as_deref().unwrap_or("-");
            let dev = tc.dev_command.as_deref().unwrap_or("-");
            format!(
                "{}: {}{}{}  build: {}  dev: {}",
                tc.name, tc.slug, ver, fw_str, build, dev
            )
        })
        .collect();
    info(&if toolchain_lines.is_empty() {
        "(none detected)".to_string()
    } else {
        toolchain_lines.join("\n")
    });

    step("environment");
    let mut env_lines = vec![match &report.package_manager {
        Some(pm) => format!("package manager: {pm}"),
        None => "package manager: (none detected)".to_string(),
    }];
    if let Some(ref mgr) = report.monorepo.manager {
        env_lines.push(format!(
            "monorepo: {} ({} paths)",
            mgr,
            report.monorepo.package_paths.len()
        ));
    }
    info(&env_lines.join("\n"));

    step("config");
    let config_line = [
        ("appz.jsonc", report.has_appz_jsonc),
        ("CLAUDE.md", report.has_claude_md),
        ("AGENTS.md", report.has_agents_md),
        ("mise.toml", report.has_mise_config),
        ("state (JSONL)", report.has_state),
    ]
    .iter()
    .map(|(label, exists)| format!("{} {label}", if *exists { "✓" } else { "✗" }))
    .collect::<Vec<_>>()
    .join("   ");
    info(&config_line);

    if !report.suggestions.is_empty() {
        step("suggestions");
        let suggestion_lines: Vec<String> = report
            .suggestions
            .iter()
            .map(|s| format!("→ {s}"))
            .collect();
        info(&suggestion_lines.join("\n"));
    }

    outro("diagnosis complete");
}

fn run_clean(args: CleanArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    let targets = appz_core::plan_clean(&canonical, args.deps);

    if !args.force {
        if json {
            println!("{}", serde_json::to_string_pretty(&targets).unwrap());
            return;
        }
        intro("appz clean");
        if targets.is_empty() {
            success("nothing to clean");
            outro("done");
            return;
        }
        step("would remove");
        let lines: Vec<String> = targets
            .iter()
            .map(|t| {
                format!(
                    "{:>8}  {}  ({})",
                    appz_core::format_size(t.size_bytes),
                    t.path.display(),
                    t.toolchain
                )
            })
            .collect();
        info(&lines.join("\n"));
        let total: u64 = targets.iter().map(|t| t.size_bytes).sum();
        outro(&format!(
            "{} to free — re-run with --force to delete",
            appz_core::format_size(total)
        ));
        return;
    }

    let report = appz_core::delete_targets(&targets);

    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        if !report.failed.is_empty() {
            std::process::exit(1);
        }
        return;
    }

    intro("appz clean");
    if report.deleted.is_empty() && report.failed.is_empty() {
        success("nothing to clean");
        outro("done");
        return;
    }
    for t in &report.deleted {
        success(&format!(
            "removed {} ({})",
            t.path.display(),
            appz_core::format_size(t.size_bytes)
        ));
    }
    for (path, err) in &report.failed {
        warning(&format!("failed to remove {}: {err}", path.display()));
    }
    if report.failed.is_empty() {
        outro(&format!(
            "freed {}",
            appz_core::format_size(report.freed_bytes)
        ));
    } else {
        error("some paths could not be removed");
    }
}

fn main() {
    ui::init();
    let cli = AppzCli::parse();
    let json = cli.json;
    ASSUME_YES.store(cli.yes, std::sync::atomic::Ordering::Relaxed);
    match cli.command {
        AppzCmd::Init(a) => run_init(a, json),
        AppzCmd::Install(a) => run_install(a, json),
        AppzCmd::Build(a) => run_build(a, json),
        AppzCmd::Dev(a) => run_dev(a, json),
        AppzCmd::Preview(a) => run_preview(a, json),
        AppzCmd::Share(a) => run_share(a, json),
        AppzCmd::Test(a) => run_test(a, json),
        AppzCmd::Lint(a) => run_lint(a, json),
        AppzCmd::Format(a) => run_format(a, json),
        AppzCmd::Clean(a) => run_clean(a, json),
        AppzCmd::Backup(a) => {
            if let Err(e) = backup::run(a, json) {
                error(&e.to_string());
            }
        }
        AppzCmd::Restore(a) => {
            if let Err(e) = backup::restore::run(a, json) {
                error(&e.to_string());
            }
        }
        AppzCmd::Doctor(a) => run_doctor_cmd(a, json),
        AppzCmd::DevInstall(a) => dev_install::run(!a.debug, a.dry_run),
        AppzCmd::Mcp => {
            if let Err(e) = mcp::serve() {
                error(&e);
            }
        }
        AppzCmd::Deploy(a) => run_deploy(a, json),
        AppzCmd::Rollback(a) => run_rollback(a, json),
        AppzCmd::Promote(a) => run_promote(a, json),
        AppzCmd::Link(a) => run_link(a, json),
        AppzCmd::Alias(a) => run_alias(a, json),
        AppzCmd::List(a) => run_list(a, json),
        AppzCmd::Inspect(a) => run_inspect(a, json),
        AppzCmd::Open(a) => run_open(a, json),
        AppzCmd::Remove(a) => run_remove(a, json),
        AppzCmd::Domains(a) => run_domains(a, json),
        AppzCmd::Webhooks(a) => webhooks::run(a, json),
        AppzCmd::Redirects(a) => redirects::run(a, json),
        AppzCmd::Projects(a) => run_projects(a, json),
        AppzCmd::Transfers(a) => run_transfers(a, json),
        AppzCmd::Login(a) => run_login(a, json),
        AppzCmd::Logout => run_logout(json),
        AppzCmd::Whoami => run_whoami(json),
        AppzCmd::Activity(a) => run_activity(a, json),
        AppzCmd::Usage(a) => run_usage(a, json),
        AppzCmd::Env(a) => run_env(a, json),
        AppzCmd::Vault(a) => run_vault_cmd(a.command, json),
        AppzCmd::Ci(a) => run_ci(a, json),
        AppzCmd::Skills(a) => run_skills(a, json),
        AppzCmd::Agents(a) => run_agents(a, json),
        AppzCmd::Update(a) => update::run(a.version, a.check, a.quiet),
        AppzCmd::Toolchain(a) => run_toolchain(a),
    }
}

fn run_toolchain(args: ToolchainArgs) {
    match args.command {
        ToolchainCmd::Refresh => match appz_core::refresh_registry_cache() {
            Ok(count) => success(&format!("toolchain registry refreshed ({count} entries)")),
            Err(e) => error(&format!("refresh failed: {e}")),
        },
    }
}

fn run_agents(args: AgentArgs, json: bool) {
    let cmd = agents::AgentCommand {
        kind: match args.action {
            AgentAction::List => agents::AgentSubcommand::List,
            AgentAction::Doctor => agents::AgentSubcommand::Doctor,
        },
        dir: args.dir,
        json,
    };
    agents::run(&cmd);
}

fn run_skills_add(dir: PathBuf, global: bool, json: bool) {
    let root = resolve_root(&dir);
    let sum = match skills::install_bundled(&root, global) {
        Ok(sum) => sum,
        Err(e) => error(&e),
    };
    if json {
        println!(
            "{}",
            serde_json::json!({ "installed": sum.installed, "failed": sum.failed })
        );
    } else {
        if !sum.installed.is_empty() {
            success(&format!("installed skill: {}", sum.installed.join(", ")));
        }
        for f in &sum.failed {
            warning(&format!("failed: {f}"));
        }
    }
    if !sum.failed.is_empty() || sum.installed.is_empty() {
        std::process::exit(1);
    }
}

fn run_skills(args: SkillsArgs, json: bool) {
    if let Some(SkillsSub::Add { global, dir }) = args.sub {
        return run_skills_add(dir, global, json);
    }
    let root = resolve_root(&args.dir);
    let report = match skills::search(
        &root,
        &skills::SkillsRequest {
            query: args.query,
            include_large: args.include_large,
        },
    ) {
        Ok(r) => r,
        Err(e) => error(&e),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    intro("appz skills");
    info(&report.context);

    if !report.excluded_large_sources.is_empty() {
        warning(&format!(
            "skipped {} large source repo(s) (slow full-repo download): {} — pass --include-large to include them",
            report.excluded_large_sources.len(),
            report.excluded_large_sources.join(", ")
        ));
    }

    if report.hits.is_empty() {
        outro("no matching skills found — try `appz skills <query>`");
        return;
    }

    step("matching skills");
    let skill_lines: Vec<String> = report
        .hits
        .iter()
        .map(|hit| {
            let mark = if hit.installed { " (installed)" } else { "" };
            format!(
                "{}  {} installs  {}{mark}",
                hit.name,
                skills::format_installs(hit.installs),
                hit.source
            )
        })
        .collect();
    info(&skill_lines.join("\n"));

    let installable: Vec<skills::SkillHit> =
        report.hits.into_iter().filter(|h| !h.installed).collect();
    if installable.is_empty() {
        outro("all recommended skills are already installed");
        return;
    }

    let selected: Vec<skills::SkillHit> = if assume_yes() {
        installable
    } else if !is_interactive_tty() {
        error("confirmation required — pass --yes to install without prompting");
    } else {
        let mut ms = cliclack::multiselect("select skills to install");
        for hit in &installable {
            ms = ms.item(hit.skill_id.clone(), hit.name.clone(), hit.source.clone());
        }
        match ms.interact() {
            Ok(ids) => installable
                .into_iter()
                .filter(|h| ids.contains(&h.skill_id))
                .collect(),
            Err(_) => {
                warning("selection cancelled");
                return;
            }
        }
    };

    if selected.is_empty() {
        outro("no skills selected");
        return;
    }

    let summary = with_spinner("installing skills...", "installation complete", || {
        skills::install(&root, &selected)
    });

    match summary {
        Ok(s) => {
            for name in &s.installed {
                success(&format!("installed {name}"));
            }
            for name in &s.failed {
                warning(&format!("failed to install {name}"));
            }
            outro("done — review skills before use; they run with full agent permissions");
        }
        Err(e) => error(&e),
    }
}

fn vault_path_for(global: bool) -> PathBuf {
    secrets::vault_path(Path::new("."), global).unwrap_or_else(|e| error(&e))
}

/// One `ls`/`check` line: the name, plus its recorded expiry when there is one.
fn vault_row(row: &appz_vault::SecretExpiry) -> String {
    use appz_vault::ExpiryStatus::*;
    let (name, at, status) = (&row.name, &row.expires_at, row.status);
    let Some(at) = at else {
        return name.clone();
    };
    let days = (*at - chrono::Utc::now()).num_days();
    let note = match status {
        Expired => format!("EXPIRED {}", at.format("%Y-%m-%d")),
        Expiring => format!("expires in {days}d ({})", at.format("%Y-%m-%d")),
        _ => format!("expires {} (in {days}d)", at.format("%Y-%m-%d")),
    };
    format!("{name}  {note}")
}

fn run_vault_cmd(cmd: VaultCmd, json: bool) {
    match cmd {
        VaultCmd::Set {
            name,
            value,
            global,
            expires,
            ttl,
        } => {
            let expires_at =
                secrets::parse_expiry(expires.as_deref(), ttl.as_deref(), chrono::Utc::now())
                    .unwrap_or_else(|e| error(&e));
            match secrets::set(&vault_path_for(global), &name, value, expires_at) {
                Ok(()) => success(&format!("stored {name}")),
                Err(e) => error(&e),
            }
        }
        VaultCmd::Ls { global } => {
            match secrets::expiries(&vault_path_for(global), chrono::Duration::days(14)) {
                Ok(rows) if json => {
                    let names: Vec<_> = rows.into_iter().map(|r| r.name).collect();
                    println!("{}", serde_json::to_string(&names).unwrap());
                }
                Ok(rows) if rows.is_empty() => info("no secrets stored"),
                Ok(rows) => info(&rows.iter().map(vault_row).collect::<Vec<_>>().join(
                    "
",
                )),
                Err(e) => error(&e),
            }
        }
        VaultCmd::Expire {
            name,
            expires,
            ttl,
            global,
        } => {
            let expires_at =
                secrets::parse_expiry(expires.as_deref(), ttl.as_deref(), chrono::Utc::now())
                    .unwrap_or_else(|e| error(&e));
            if expires_at.is_none() {
                error("give --expires YYYY-MM-DD or --ttl 90d");
            }
            match secrets::set_expiry(&vault_path_for(global), &name, expires_at) {
                Ok(()) => success(&format!("recorded expiry for {name}")),
                Err(e) => error(&e),
            }
        }
        VaultCmd::Check { within, global } => {
            use appz_vault::ExpiryStatus::{Expired, Expiring};
            let within = secrets::parse_duration(&within).unwrap_or_else(|e| error(&e));
            let rows =
                secrets::expiries(&vault_path_for(global), within).unwrap_or_else(|e| error(&e));
            let due: Vec<_> = rows
                .iter()
                .filter(|r| matches!(r.status, Expired | Expiring))
                .collect();
            if json {
                let out: Vec<_> = rows
                    .iter()
                    .map(|r| serde_json::to_value(r).unwrap())
                    .collect();
                println!("{}", serde_json::to_string(&out).unwrap());
                if !due.is_empty() {
                    std::process::exit(1);
                }
            } else if due.is_empty() {
                success("no secrets expired or expiring soon");
            } else {
                let lines: Vec<_> = due.into_iter().map(vault_row).collect();
                error(&format!(
                    "rotate these:
{}",
                    lines.join(
                        "
"
                    )
                ));
            }
        }
        VaultCmd::Rm { name, global } => match secrets::remove(&vault_path_for(global), &name) {
            Ok(true) => success(&format!("removed {name}")),
            Ok(false) => error(&format!("{name} is not stored")),
            Err(e) => error(&e),
        },
        VaultCmd::Unlock { global } => match secrets::unlock(&vault_path_for(global)) {
            Ok((_, true)) => success("vault unlocked"),
            Ok((_, false)) => warning(concat!(
                "vault unlocked for this command only: the OS keyring is unavailable, ",
                "so it will ask again. Set APPZ_VAULT_PASSPHRASE, or pass the platform ",
                "token as an environment variable instead"
            )),
            Err(e) => error(&e),
        },
        VaultCmd::Lock { global } => {
            secrets::lock(&vault_path_for(global));
            success("vault locked");
        }
    }
}

fn run_env(args: EnvArgs, json: bool) {
    match args.command {
        EnvCmd::Vault(cmd) => run_vault_cmd(cmd, json),
        EnvCmd::Pull { file, environment } => {
            cloud_env::pull_to_file(&resolve_root(Path::new(".")), &file, &environment, json)
        }
        EnvCmd::Run { environment, cmd } => {
            cloud_env::run(&resolve_root(Path::new(".")), &environment, &cmd)
        }
        EnvCmd::Cloud { command } => {
            let root = resolve_root(Path::new("."));
            match command {
                CloudEnvCmd::Ls { environment } => cloud_env::ls(&root, &environment, json),
                CloudEnvCmd::Rm { key } => cloud_env::rm(&root, &key, json),
            }
        }
        EnvCmd::Environments { command } => {
            let root = resolve_root(Path::new("."));
            match command {
                EnvironmentsCmd::Ls => cloud_env::environments_ls(&root, json),
                EnvironmentsCmd::Add {
                    slug,
                    branch_pattern,
                } => cloud_env::environments_add(&root, &slug, branch_pattern.as_deref(), json),
                EnvironmentsCmd::Rm { slug } => cloud_env::environments_rm(&root, &slug, json),
            }
        }
        EnvCmd::Push { file, environment } => {
            if !json {
                intro("appz env push");
            }
            let root = resolve_root(Path::new("."));
            let project_id = appz_deploy::resolve_project_id_from_dir(&root);
            let vault_tok = crate::secrets::deploy_env(&root, &["APPZ_TOKEN".to_string()])
                .ok()
                .and_then(|v| v.vars.get("APPZ_TOKEN").cloned());
            let token = std::env::var("APPZ_TOKEN")
                .ok()
                .filter(|t| !t.is_empty())
                .or(vault_tok)
                .or_else(|| appz_deploy::resolve_auth_token(None));
            if token.is_none() {
                error("not authenticated. Run `appz login` or set APPZ_TOKEN");
            }
            let api_base = std::env::var("APPZ_API_URL")
                .unwrap_or_else(|_| "https://api.appz.dev".to_string());
            let target_path = if root.join(&file).is_file() {
                root.join(&file)
            } else if file.as_path() == std::path::Path::new(".env.local")
                && root.join(".env").is_file()
            {
                root.join(".env")
            } else {
                error(&format!(
                    "environment file not found: {}",
                    root.join(&file).display()
                ));
            };

            let content = std::fs::read_to_string(&target_path).unwrap_or_else(|e| {
                error(&format!("failed to read {}: {e}", target_path.display()))
            });
            let kvs = appz_deploy::parse_env_file(&content);
            if kvs.is_empty() {
                if json {
                    println!("{}", serde_json::json!({ "pushed": 0 }));
                } else {
                    info("no environment variables found in file");
                    outro("nothing to push");
                }
                return;
            }

            let targets = if environment.is_empty() {
                cloud_env::default_push_environments()
            } else {
                cloud_env::validate_environments(&environment)
            };
            let cloud_vars: Vec<appz_deploy::CloudEnvVar> = kvs
                .into_iter()
                .map(|(key, value)| appz_deploy::CloudEnvVar {
                    id: None,
                    key,
                    value,
                    target: targets.clone(),
                    git_branch: None,
                    created_at: None,
                })
                .collect();

            match appz_deploy::push_cloud_env(&api_base, &project_id, token.as_deref(), &cloud_vars)
            {
                Ok(count) => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({ "pushed": count, "projectId": project_id })
                        );
                    } else {
                        success(&format!(
                            "uploaded {} environment variables to project '{}'",
                            count, project_id
                        ));
                        outro("done");
                    }
                }
                Err(e) => error(&e.to_string()),
            }
        }
    }
}

fn run_ci(args: CiArgs, json: bool) {
    let CiCmd::Init(init) = args.command;
    let root = resolve_root(&init.dir);
    // Setting repository secrets is an account-visible action: only with an
    // explicit --yes, or after the user agrees at a real terminal.
    let set_secrets = if init.no_secrets {
        false
    } else if assume_yes() {
        true
    } else if is_tty() {
        cliclack::confirm("Store the deploy token(s) as GitHub repository secrets with gh?")
            .initial_value(false)
            .interact()
            .unwrap_or(false)
    } else {
        false
    };

    let report = match ci::run(
        &root,
        &ci::CiRequest {
            target: init.target,
            branch: init.branch,
            force: init.force,
            set_secrets,
        },
    ) {
        Ok(r) => r,
        Err(e) => error(&e),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }

    intro("appz ci");
    success(&format!("wrote {}", report.workflow));
    info(&format!(
        "repo: {}\ntarget: {}\nbranch: {}",
        report.repo, report.target, report.branch
    ));
    for secret in &report.secrets {
        match secret.status {
            "set" => success(&format!("set repository secret {}", secret.name)),
            "failed" => warning(&format!(
                "could not set {}: {}",
                secret.name,
                secret.detail.as_deref().unwrap_or("unknown error")
            )),
            _ => info(&format!(
                "secret {} not set: {}",
                secret.name,
                secret.detail.as_deref().unwrap_or("no value")
            )),
        }
    }
    outro("commit the workflow to turn on deploy-on-push");
}

fn run_deploy(args: DeployArgs, json: bool) {
    appz_deploy::enable_cli_deploy_interrupts();
    let canonical = resolve_root(&args.dir);
    let all = args.all;
    let req = deploy::DeployRequest {
        target: args.target,
        project: args.project,
        force: args.force,
        prebuilt: args.prebuilt,
        prod: args.prod,
        env: args.env,
        build_env: args.build_env,
        dry_run: args.dry_run,
        json,
        backup: !args.no_backup,
        install: if assume_yes() {
            deploy::Consent::Yes
        } else if !json && is_tty() {
            deploy::Consent::Ask
        } else {
            deploy::Consent::Never
        },
        link: if assume_yes() {
            deploy::Consent::Yes
        } else if !json && is_tty() {
            deploy::Consent::Ask
        } else {
            deploy::Consent::Never
        },
        // Login needs a browser, so it requires a real terminal even with --yes.
        login: if json || !is_tty() {
            deploy::Consent::Never
        } else if assume_yes() {
            deploy::Consent::Yes
        } else {
            deploy::Consent::Ask
        },
    };
    let started = Instant::now();
    if !json {
        intro("appz deploy");
    }

    if all {
        match deploy::run_all(&canonical, req) {
            Ok(results) => {
                let failed = results.iter().filter(|r| r.error.is_some()).count();
                if json {
                    println!("{}", serde_json::to_string_pretty(&results).unwrap());
                } else {
                    for r in &results {
                        if let (Some(_), Some(e)) = (&r.output, &r.error) {
                            eprintln!("{}: deployed, but a hook failed: {e}", r.target);
                        }
                        match (&r.output, &r.error) {
                            (Some(o), _) if args.dry_run => print_deploy_dry_run(o),
                            (Some(o), _) if o.unchanged => {
                                info(&format!("{}: unchanged, skipped", r.target))
                            }
                            (Some(o), _) => success(&format!(
                                "{} ({}): {}",
                                r.target,
                                if o.is_preview {
                                    "preview"
                                } else {
                                    "production"
                                },
                                o.url.as_deref().unwrap_or("deployed (url unknown)")
                            )),
                            (_, Some(e)) => eprintln!("{}: failed: {e}", r.target),
                            _ => {}
                        }
                    }
                    outro(if args.dry_run {
                        "nothing was deployed"
                    } else if failed == 0 {
                        "deploy complete"
                    } else {
                        "deploy finished with failures"
                    });
                }
                if failed > 0 {
                    std::process::exit(1);
                }
            }
            Err(e) => error(&e),
        }
        return;
    }
    match deploy::run(&canonical, req) {
        Ok(output) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else if args.dry_run {
                print_deploy_dry_run(&output);
                outro("Nothing was deployed");
            } else {
                if output.unchanged {
                    success(&format!(
                        "Unchanged since the last deploy to {}; nothing was redeployed",
                        output.provider
                    ));
                    if let Some(url) = &output.url {
                        ui::print_kv(&[(deploy_kind(&output), url.clone())]);
                    }
                    ui::hint("Pass --force to deploy anyway");
                    outro("Done");
                    return;
                }
                for f in &output.setup_files {
                    ui::hint(&format!("Created {f} (the project had none)"));
                }
                let took = ui::fmt::duration_ms(started.elapsed().as_millis() as u64);
                success(&format!("Deployed to {} [{took}]", output.provider));
                match &output.url {
                    Some(url) => ui::print_kv(&[(deploy_kind(&output), url.clone())]),
                    None => warning(
                        "Deploy succeeded but the live URL could not be confirmed from CLI output",
                    ),
                }
                if let Some(id) = output.backup.as_ref().and_then(|b| b.snapshot.as_deref()) {
                    ui::hint(&format!("Source snapshot saved ({id})"));
                }
                outro("Deploy complete");
            }
        }
        Err(e) => error(&e),
    }
}

fn deploy_kind(output: &appz_deploy::DeployOutput) -> &'static str {
    if output.is_preview {
        "Preview"
    } else {
        "Production"
    }
}

fn print_deploy_dry_run(output: &appz_deploy::DeployOutput) {
    success(&format!(
        "Dry run: would deploy to {} ({})",
        output.provider,
        deploy_kind(output).to_lowercase()
    ));
    let auth = output.auth.as_ref().map(|a| match a {
        appz_deploy::AuthState::Authenticated => "signed in",
        appz_deploy::AuthState::NotAuthenticated => {
            "not signed in (a real deploy would offer to log in)"
        }
        appz_deploy::AuthState::Unknown => "could not be checked",
    });
    let mut rows = vec![
        (
            "Changes",
            if output.unchanged {
                "none since the last deploy (--force to deploy anyway)".to_string()
            } else {
                "would be deployed".to_string()
            },
        ),
        (
            "CLI",
            if output.cli_missing {
                "not installed (a real deploy installs it with mise)".into()
            } else {
                "installed".into()
            },
        ),
        (
            "Project",
            if output.link_needed {
                "not linked (a real deploy asks to create/link one)".into()
            } else {
                "linked".into()
            },
        ),
        ("Auth", auth.unwrap_or("").to_string()),
    ];
    if !output.setup_files.is_empty() {
        rows.push(("Would create", output.setup_files.join(", ")));
    }
    ui::print_kv(&rows);
}

fn run_rollback(args: RollbackArgs, json: bool) {
    // `appz rollback some/dir`: a lone existing path is the directory.
    let (version, dir) = match (args.version, args.dir) {
        (Some(v), None) if Path::new(&v).is_dir() => (None, PathBuf::from(v)),
        (v, d) => (v, d.unwrap_or_else(|| PathBuf::from("."))),
    };
    let canonical = resolve_root(&dir);

    let resolved = match &version {
        Some(v) => deploy::resolve_version(&canonical, args.target.clone(), v).map(|id| (id, None)),
        None => deploy::plan_rollback(&canonical, args.target.clone()).map(|plan| {
            (
                plan.target.version_id.clone().unwrap_or_default(),
                Some(plan),
            )
        }),
    };
    let (version_id, plan) = match resolved {
        Ok(r) => r,
        Err(e) => error(&e),
    };
    if let Some(plan) = &plan {
        let label = |d: &appz_deploy::TeamDeployment| {
            format!(
                "{} ({})",
                d.id,
                d.url
                    .as_deref()
                    .unwrap_or(d.version_id.as_deref().unwrap_or("-"))
            )
        };
        let from = plan
            .current
            .as_ref()
            .map(label)
            .unwrap_or_else(|| "-".into());
        let msg = format!("Roll back production: {from} -> {}", label(&plan.target));
        if json {
            // Non-interactive consumers must opt in with --yes.
            if !assume_yes() {
                println!(
                    "{}",
                    serde_json::json!({ "status": "confirmation_required", "plan": plan })
                );
                error("pass --yes to roll back to the previous production deployment");
            }
        } else {
            ui::hint(&msg);
            let ok = assume_yes()
                || is_tty()
                    && cliclack::confirm("Roll back?")
                        .initial_value(false)
                        .interact()
                        .unwrap_or(false);
            if !ok {
                return warning("Rollback cancelled");
            }
        }
    }

    match deploy::rollback(&canonical, args.target, &version_id) {
        Ok(output) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                success(&format!(
                    "Rolled back {} to version {}",
                    output.provider, version_id
                ));
                if let Some(url) = &output.url {
                    println!("{url}");
                }
            }
        }
        Err(e) => error(&e),
    }
}

fn run_promote(args: PromoteArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    if !assume_yes() && (json || !is_tty()) {
        error("confirmation required — pass --yes to promote without prompting");
    }

    let confirm = |d: &appz_deploy::DeploymentInspect| {
        if assume_yes() {
            return true;
        }
        let current = d
            .production_url
            .as_deref()
            .map(|u| format!(" (production now: {u})"))
            .unwrap_or_default();
        cliclack::confirm(format!(
            "Promote deployment {} (version {}) to production? It replaces the current production deployment{current}.",
            d.id, d.version_id
        ))
        .initial_value(false)
        .interact()
        .unwrap_or(false)
    };
    match deploy::promote(&canonical, args.target, &args.id_or_url, &confirm) {
        Ok(Some(r)) if json => println!(
            "{}",
            serde_json::json!({ "status": "promoted", "provider": r.provider, "project_id": r.project_id,
                "deployment_id": r.deployment_id, "version_id": r.version_id, "url": r.url })
        ),
        Ok(Some(r)) => {
            success(&format!("Promoted {} to production", r.deployment_id));
            println!("{}", r.url);
        }
        Ok(None) if json => println!("{}", serde_json::json!({ "status": "canceled" })),
        Ok(None) => warning("Canceled"),
        Err(e) => error(&e),
    }
}

fn run_link(args: LinkArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    match deploy::link(&canonical, args.target, args.project) {
        Ok(provider_name) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "status": "linked", "provider": provider_name })
                );
            } else {
                success(&format!("Linked project to {provider_name}"));
            }
        }
        Err(e) => error(&e),
    }
}

fn run_alias(args: AliasArgs, json: bool) {
    match args.command {
        Some(AliasCmd::Set {
            domain,
            dir,
            target,
        }) => run_alias_set(&dir, target, &domain, json),
        Some(AliasCmd::Ls { dir, target }) => run_alias_ls(&dir, target, json),
        Some(AliasCmd::Rm { host, dir, target }) => run_alias_rm(&dir, target, &host, json),
        None => match args.domain {
            Some(domain) => run_alias_set(&args.dir, args.target, &domain, json),
            None => error("usage: appz alias <DOMAIN> [DIR] | set <DOMAIN> | ls | rm <HOST>"),
        },
    }
}

fn run_alias_set(dir: &Path, target: Option<String>, domain: &str, json: bool) {
    let canonical = resolve_root(dir);
    match deploy::alias(&canonical, target, domain) {
        Ok(url) => {
            if json {
                println!("{}", serde_json::json!({ "status": "aliased", "url": url }));
            } else {
                success(&format!("Assigned alias {domain}"));
                println!("{url}");
            }
        }
        Err(e) => error(&e),
    }
}

fn event_ms(t: Option<f64>) -> Option<i64> {
    // The API does not document the unit; treat small values as seconds.
    t.map(|t| if t < 1e11 { t * 1000.0 } else { t } as i64)
}

fn run_activity(args: ActivityArgs, json: bool) {
    let fetch = ui::fetching("activity");
    let page = match deploy::list_activity(
        Path::new("."),
        args.target,
        args.limit,
        args.next.as_deref(),
    ) {
        Ok(p) => p,
        Err(e) => error(&e),
    };
    let items: Vec<_> = page
        .items
        .iter()
        .filter(|e| args.action.as_deref().is_none_or(|a| e.action == a))
        .collect();
    if json {
        let out = serde_json::json!({ "items": items, "nextCursor": page.next_cursor });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
        return;
    }
    fetch.done(&format!("{} found", ui::fmt::plural(items.len(), "event")));
    if items.is_empty() {
        ui::empty("activity events", None);
    } else {
        let now = ui::fmt::now_ms();
        let mut t = ui::Table::new(&["Age", "Actor", "Action", "Target", "IP"]);
        for e in &items {
            let actor = e
                .actor_user_id
                .as_deref()
                .or(e.actor_key_id.as_deref())
                .or(e.actor_type.as_deref())
                .unwrap_or("");
            let target = match (e.resource_type.as_deref(), e.resource_id.as_deref()) {
                (Some(t), Some(id)) => format!("{t}:{id}"),
                (Some(t), None) => t.to_string(),
                _ => String::new(),
            };
            let age = event_ms(e.created_at).map_or_else(String::new, |ms| ui::fmt::age(ms, now));
            t.row(vec![
                age,
                actor.to_string(),
                e.action.clone(),
                target,
                e.ip.clone().unwrap_or_default(),
            ]);
        }
        t.print();
    }
    print_next_hint(&page.next_cursor, "appz activity");
}

fn usage_percent(used: f64, limit: f64) -> Option<f64> {
    (limit > 0.0).then(|| used / limit * 100.0)
}

/// One usage row: `[label, "20 of 100", "20%"]`; the percent is colored as it
/// nears the limit.
fn usage_row(u: &appz_deploy::UsageEntry) -> Vec<String> {
    let amount = if !u.detail.trim().is_empty() {
        u.detail.clone()
    } else if u.limit > 0.0 {
        format!("{} of {}", u.used, u.limit)
    } else {
        format!("{} (unlimited)", u.used)
    };
    let pct = match usage_percent(u.used, u.limit) {
        Some(p) if p >= 100.0 => ui::theme::out::red(&format!("{p:.0}%")),
        Some(p) if p >= 80.0 => ui::theme::out::yellow(&format!("{p:.0}%")),
        Some(p) => format!("{p:.0}%"),
        None => String::new(),
    };
    vec![u.label.clone(), amount, pct]
}

fn run_usage(args: UsageArgs, json: bool) {
    let fetch = ui::fetching("usage");
    let report = match deploy::usage_report(Path::new("."), args.target) {
        Ok(r) => r,
        Err(e) => error(&e),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }
    fetch.done("Usage against plan limits");
    if report.usage.is_empty() {
        ui::empty("usage", None);
    } else {
        let mut t = ui::Table::new(&["Resource", "Usage", "%"]);
        for u in &report.usage {
            t.row(usage_row(u));
        }
        t.print();
    }
    let features = if report.entitlements.is_empty() {
        "none".to_string()
    } else {
        report.entitlements.join(", ")
    };
    ui::hint(&format!("Plan features: {features}"));
}

fn alias_created(ms: i64) -> String {
    ui::fmt::datetime(ms)
}

fn run_alias_ls(dir: &Path, target: Option<String>, json: bool) {
    let canonical = resolve_root(dir);
    let fetch = ui::fetching("aliases");
    match deploy::list_aliases(&canonical, target) {
        Ok(aliases) if json => println!("{}", serde_json::to_string_pretty(&aliases).unwrap()),
        Ok(aliases) => {
            fetch.done(&format!(
                "{} found",
                ui::fmt::plural(aliases.len(), "alias")
            ));
            if aliases.is_empty() {
                ui::empty("aliases", Some("Run `appz alias set <domain>` to add one"));
                return;
            }
            let now = ui::fmt::now_ms();
            let mut t = ui::Table::new(&["Host", "Deployment", "Age", "Production"]);
            for a in &aliases {
                t.row(vec![
                    a.host.clone(),
                    a.deployment_id.clone(),
                    ui::fmt::age(a.created_at, now),
                    if a.is_production {
                        "✓".into()
                    } else {
                        String::new()
                    },
                ]);
            }
            t.print();
        }
        Err(e) => error(&e),
    }
}

fn run_alias_rm(dir: &Path, target: Option<String>, host: &str, json: bool) {
    let canonical = resolve_root(dir);
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    // Look the alias up first: it lets us refuse the production host and show what is
    // about to be removed. The server enforces the same rule for races.
    let aliases = match deploy::list_aliases(&canonical, target.clone()) {
        Ok(a) => a,
        Err(e) => error(&e),
    };
    let Some(alias) = aliases.iter().find(|a| a.host == host) else {
        error(&format!(
            "alias {host} not found; run `appz alias ls` to see this project's aliases"
        ));
    };
    if alias.is_production {
        error(&format!(
            "{host} is the production host and cannot be removed"
        ));
    }
    if !assume_yes() {
        if json || !is_interactive_tty() {
            error("confirmation required — pass --yes to remove the alias without prompting");
        }
        let prompt = format!(
            "Remove alias {host} (deployment {}, created {})?",
            alias.deployment_id,
            alias_created(alias.created_at)
        );
        match cliclack::confirm(prompt).initial_value(false).interact() {
            Ok(true) => {}
            _ => {
                warning("Canceled");
                return;
            }
        }
    }

    match deploy::remove_alias(&canonical, target, &host) {
        Ok(()) if json => println!(
            "{}",
            serde_json::json!({ "status": "removed", "host": host })
        ),
        Ok(()) => success(&format!("Removed alias {host}")),
        Err(e) => error(&e),
    }
}

fn print_next_hint(next: &Option<String>, cmd: &str) {
    if let Some(cursor) = next {
        ui::hint(&format!("Next page: {cmd} --next {cursor}"));
    }
}

fn fmt_millis(ms: Option<i64>) -> String {
    ms.map_or_else(|| "-".to_string(), ui::fmt::datetime)
}

/// `● Ready (current)` for the deployment production points at.
fn deployment_status(status: &str, is_current: bool) -> String {
    let s = ui::fmt::status(status);
    if is_current {
        format!("{s} {}", ui::theme::out::dim("(current)"))
    } else {
        s
    }
}

/// The pre-team-wide listing, still used for providers other than appz.dev.
fn run_list_legacy(dir: &Path, target: Option<String>, json: bool) {
    let fetch = ui::fetching("deployments");
    match deploy::list(dir, target) {
        Ok(deployments) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&deployments).unwrap());
                return;
            }
            fetch.done(&format!(
                "{} found",
                ui::fmt::plural(deployments.len(), "deployment")
            ));
            if deployments.is_empty() {
                ui::empty("deployments for this project", None);
                return;
            }
            let now = ui::fmt::now_ms();
            let mut t = ui::Table::new(&["Age", "ID", "Status", "URL"]);
            for d in &deployments {
                t.row(vec![
                    d.created_at
                        .map(|c| ui::fmt::age(c.timestamp_millis(), now))
                        .unwrap_or_default(),
                    d.id.clone(),
                    deployment_status(&format!("{:?}", d.status), d.is_current),
                    d.url.clone().unwrap_or_default(),
                ]);
            }
            t.print();
        }
        Err(e) => error(&e),
    }
}

/// Validate the `appz ls` filter flags and convert them to an API query.
fn build_list_query(
    args: &ListArgs,
    project: Option<String>,
) -> Result<appz_deploy::DeploymentQuery, String> {
    use list_filters::*;
    let now = chrono::Utc::now();
    let since = args
        .since
        .as_deref()
        .map(|v| parse_time(v, false, now))
        .transpose()?;
    let until = args
        .until
        .as_deref()
        .map(|v| parse_time(v, true, now))
        .transpose()?;
    if let (Some(s), Some(u)) = (since, until)
        && s > u
    {
        return Err("--since must not be later than --until".into());
    }
    Ok(appz_deploy::DeploymentQuery {
        project,
        production: args.prod,
        environment: args
            .environment
            .as_deref()
            .map(parse_environment)
            .transpose()?,
        status: parse_statuses(&args.status)?,
        since,
        until,
        meta: args
            .meta
            .iter()
            .map(|m| parse_meta(m))
            .collect::<Result<_, _>>()?,
        limit: args.limit,
        cursor: args.next.clone(),
    })
}

/// The filters actually sent to the API, for `--json` output.
fn list_filters_json(q: &appz_deploy::DeploymentQuery) -> serde_json::Value {
    let meta: serde_json::Map<String, serde_json::Value> = q
        .meta
        .iter()
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.into()))
        .collect();
    serde_json::json!({
        "project": q.project,
        "type": q.production.then_some("production"),
        "environment": q.environment,
        "status": q.status,
        "since": q.since,
        "until": q.until,
        "meta": meta,
        "limit": q.limit,
    })
}

fn run_list(args: ListArgs, json: bool) {
    let (dir, named) = match args.project.clone() {
        Some(p) if !Path::new(&p).is_dir() => (PathBuf::from("."), Some(p)),
        Some(p) => (PathBuf::from(p), None),
        None => (PathBuf::from("."), None),
    };
    let canonical = resolve_root(&dir);
    match deploy::lists_via_appzdev(&canonical, args.target.as_deref()) {
        Ok(true) => {}
        Ok(false) => {
            let new_flags = named.is_some()
                || args.all
                || args.prod
                || args.environment.is_some()
                || !args.status.is_empty()
                || args.since.is_some()
                || args.until.is_some()
                || !args.meta.is_empty()
                || args.limit.is_some()
                || args.next.is_some();
            if new_flags {
                error(
                    "project names, --all, --prod, --environment, --status, --since, --until, --meta, --limit and --next are only supported for appz.dev projects",
                );
            }
            return run_list_legacy(&canonical, args.target, json);
        }
        Err(e) => error(&e),
    }
    let project = named.or_else(|| {
        if args.all {
            None
        } else {
            deploy::linked_project(&canonical)
        }
    });
    let team_wide = project.is_none();
    let query = match build_list_query(&args, project) {
        Ok(q) => q,
        Err(e) => error(&e),
    };
    let fetch = ui::fetching("deployments");
    match deploy::list_page(&canonical, args.target, &query) {
        Ok(page) => {
            if json {
                let mut out = serde_json::to_value(&page).unwrap();
                out["filters"] = list_filters_json(&query);
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
                return;
            }
            let scope = query
                .project
                .as_deref()
                .map_or_else(|| "this team".to_string(), |p| p.to_string());
            fetch.done(&format!(
                "{} found for {scope}",
                ui::fmt::plural(page.items.len(), "deployment")
            ));
            if page.items.is_empty() {
                ui::empty("deployments", Some("Run `appz deploy` to create one"));
                return;
            }
            let now = ui::fmt::now_ms();
            let mut headers = vec!["Age"];
            if team_wide {
                headers.push("Project");
            }
            headers.extend(["ID", "Status", "URL"]);
            let mut t = ui::Table::new(&headers);
            for d in &page.items {
                let mut row = vec![
                    d.created_at
                        .map(|c| ui::fmt::age(c, now))
                        .unwrap_or_default(),
                ];
                if team_wide {
                    row.push(
                        d.project_slug
                            .clone()
                            .or_else(|| d.project_name.clone())
                            .or_else(|| d.project_id.clone())
                            .unwrap_or_default(),
                    );
                }
                row.push(d.id.clone());
                row.push(deployment_status(
                    d.status.as_deref().unwrap_or("unknown"),
                    d.is_current,
                ));
                row.push(d.url.clone().unwrap_or_default());
                t.row(row);
            }
            t.print();
            print_next_hint(&page.next, "appz ls");
        }
        Err(e) => error(&e),
    }
}

/// Open `url` in the default browser (best effort).
fn open_in_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn run_open(mut args: OpenArgs, json: bool) {
    // `appz open ./dir` means the directory, not a deployment ID.
    if let Some(first) = &args.id_or_url
        && args.dir == Path::new(".")
        && Path::new(first).is_dir()
    {
        args.dir = PathBuf::from(args.id_or_url.take().unwrap());
    }
    let canonical = resolve_root(&args.dir);
    match deploy::open_target(
        &canonical,
        args.target,
        args.id_or_url.as_deref(),
        args.dashboard,
    ) {
        Ok(t) => {
            if json || !is_interactive_tty() {
                if json {
                    println!("{}", serde_json::to_string_pretty(&t).unwrap());
                } else {
                    println!("{}", t.url);
                }
            } else {
                ui::hint(&format!("Opening {}", t.url));
                open_in_browser(&t.url);
            }
        }
        Err(e) => error(&e),
    }
}

fn run_remove(args: RemoveArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    let mut refs: Vec<String> = Vec::new();
    for d in args.deployments {
        if !refs.contains(&d) {
            refs.push(d);
        }
    }
    if !json {
        ui::hint(&format!("Deployments to delete: {}", refs.join(", ")));
    }

    if !assume_yes() {
        if json || !is_interactive_tty() {
            error("confirmation required — pass --yes to delete without prompting");
        }
        let plural = if refs.len() == 1 { "" } else { "s" };
        let ok = cliclack::confirm(format!("Delete {} deployment{plural}?", refs.len()))
            .initial_value(false)
            .interact()
            .unwrap_or(false);
        if !ok {
            warning("Cancelled, nothing deleted");

            return;
        }
    }
    let results = match deploy::remove_deployments(&canonical, args.target, &refs) {
        Ok(r) => r,
        Err(e) => error(&e),
    };
    let failed = results.iter().filter(|(_, r)| r.is_err()).count();
    if json {
        let out: Vec<_> = results
            .iter()
            .map(|(r, res)| match res {
                Ok(id) => serde_json::json!({ "id": id, "removed": true, "error": null }),
                Err(e) => serde_json::json!({ "id": r, "removed": false, "error": e }),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        for (r, res) in &results {
            match res {
                Ok(id) => success(&format!("Removed {id}")),
                Err(e) => warning(&format!("{r}: {e}")),
            }
        }
        if results.len() > 1 {
            ui::hint(&format!(
                "{} removed, {failed} failed",
                results.len() - failed
            ));
        }
    }

    if failed > 0 {
        std::process::exit(1);
    }
}

fn run_inspect(args: InspectArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    let insp = match deploy::inspect(&canonical, args.target, &args.id_or_url) {
        Ok(i) => i,
        Err(e) => error(&e),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&insp).unwrap());
        return;
    }
    let created = insp.created_at.map(|c| {
        let ms = c.timestamp_millis();
        format!(
            "{} ({} ago)",
            ui::fmt::datetime(ms),
            ui::fmt::age(ms, ui::fmt::now_ms())
        )
    });
    ui::heading("Deployment");
    ui::print_kv(&[
        ("ID", insp.id.clone()),
        ("Project", insp.project_id.clone()),
        ("Version", insp.version_id.clone()),
        ("Status", ui::fmt::status(&format!("{:?}", insp.status))),
        ("Target", insp.target.to_string()),
        ("Created", created.unwrap_or_default()),
    ]);
    if !insp.hosts.is_empty() {
        ui::heading("Hosts");
        for h in &insp.hosts {
            let clean = h
                .trim_start_matches("https://")
                .trim_start_matches("http://");
            println!("  https://{clean}");
        }
    }
    ui::heading("Edge storage");
    let mut storage = vec![
        ("Files", insp.file_count.to_string()),
        ("Size", appz_core::format_size(insp.total_bytes)),
    ];
    if let Some(mkey) = &insp.manifest_key {
        storage.push(("Manifest", mkey.clone()));
    }
    ui::print_kv(&storage);
    if !insp.redirects.is_empty() || !insp.headers.is_empty() {
        ui::heading("Routing rules");
        let mut t = ui::Table::new(&["Type", "Rule"]);
        for r in &insp.redirects {
            t.row(vec!["redirect".into(), r.to_string()]);
        }
        for h in &insp.headers {
            t.row(vec!["header".into(), h.to_string()]);
        }
        t.print();
    }
    let files = if args.files {
        insp.files.as_deref().unwrap_or_default()
    } else {
        &[]
    };
    if !files.is_empty() {
        ui::heading("Manifest files");
        let mut t = ui::Table::new(&["Path", "Size", "Type", "Hash"]);
        for f in files {
            t.row(vec![
                f.path.clone(),
                appz_core::format_size(f.size),
                f.content_type.clone().unwrap_or_default(),
                f.hash.chars().take(12).collect(),
            ]);
        }
        t.print();
    } else if !args.files && insp.file_count > 0 {
        ui::hint("Pass --files to list manifest files");
    }
}

fn run_domains(args: DomainsArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    match args.command {
        DomainsCmd::Add { domain } => match deploy::add_domain(&canonical, args.target, &domain) {
            Ok(dom) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&dom).unwrap());
                } else {
                    intro("appz domains add");
                    let mut details = vec![
                        format!("domain:      {}", dom.domain),
                        format!("status:      {}", dom.display_status()),
                    ];
                    if let Some(ssl) = &dom.ssl_status {
                        details.push(format!("ssl:         {ssl}"));
                    }
                    info(&details.join("\n"));

                    if let Some(ch) = &dom.challenge
                        && !dom.verified
                    {
                        step("1. Ownership verification (required once):");
                        info(&format!("{:<8}  {:<36}  {}", ch.r#type, ch.name, ch.value));
                    }
                    if dom.cname.is_some() || dom.apex.is_some() {
                        step("2. DNS routing target:");
                        let mut dns_targets = Vec::new();
                        if let Some(cname) = &dom.cname {
                            dns_targets.push(format!(
                                "{:<8}  {:<36}  {}",
                                "CNAME", cname.name, cname.target
                            ));
                        }
                        if let Some(apex) = &dom.apex {
                            dns_targets.push(format!("{:<8}  {:<36}  {}", "A", apex.name, apex.ip));
                        }
                        info(&dns_targets.join("\n"));
                    } else if !dom.verification.is_empty() {
                        step("dns configuration records");
                        let recs: Vec<String> = dom
                            .verification
                            .iter()
                            .map(|v| {
                                let reason = v.reason.as_deref().unwrap_or("-");
                                format!(
                                    "{:<8}  {:<32}  {}  ({})",
                                    v.r#type.to_uppercase(),
                                    v.domain,
                                    v.value,
                                    reason
                                )
                            })
                            .collect();
                        info(&recs.join("\n"));
                    }
                    if let Some(hint) = dom.dns_setup_hint() {
                        step("DNS routing at your provider");
                        info(&hint);
                    }
                    step("next steps");
                    info(&format!(
                        "1. Configure the DNS records above\n\
                         2. Run `appz domains verify {}` once configured",
                        dom.domain
                    ));
                    outro("domain added");
                }
            }
            Err(e) => error(&e),
        },
        DomainsCmd::List { project, next } => {
            let query = appz_deploy::DomainQuery {
                project,
                limit: None,
                cursor: next,
            };
            match deploy::list_domains_page(&canonical, args.target, &query) {
                Ok(page) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&page).unwrap());
                    } else {
                        intro("appz domains");
                        if page.items.is_empty() {
                            info(
                                "no custom domains found\nrun `appz domains add <domain>` to connect a custom domain",
                            );
                        } else {
                            let lines: Vec<String> = page
                                .items
                                .iter()
                                .map(|t| {
                                    let d = &t.domain;
                                    let ssl = d.ssl_status.as_deref().unwrap_or("-");
                                    let verified_str =
                                        if d.verified { "verified" } else { "pending" };
                                    let project = t
                                        .project_slug
                                        .as_deref()
                                        .or(t.project_name.as_deref())
                                        .or(t.project_id.as_deref())
                                        .unwrap_or("-");
                                    format!(
                                        "{:<30}  {:<20}  status:{:<12}  ssl:{:<10}  {}",
                                        d.domain,
                                        project,
                                        d.display_status(),
                                        ssl,
                                        verified_str
                                    )
                                })
                                .collect();
                            info(&lines.join("\n"));
                            print_next_hint(&page.next, "appz domains ls");
                        }
                        outro("done");
                    }
                }
                Err(e) => error(&e),
            }
        }
        DomainsCmd::Verify { domain } => {
            match deploy::verify_domain(&canonical, args.target, &domain) {
                Ok(res) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&res).unwrap());
                    } else {
                        intro("appz domains verify");
                        let mut details = vec![
                            format!("domain:      {}", res.domain),
                            format!("status:      {}", res.display_status()),
                        ];
                        if let Some(ssl) = &res.ssl_status {
                            details.push(format!("ssl:         {ssl}"));
                        }
                        info(&details.join("\n"));

                        if res.verified {
                            success(&format!("{} is verified and active!", res.domain));
                        } else {
                            warning("verification still pending");
                            if let Some(ch) = &res.challenge {
                                step("required ownership challenge");
                                info(&format!("{:<8}  {:<36}  {}", ch.r#type, ch.name, ch.value));
                            }
                            if res.cname.is_some() || res.apex.is_some() {
                                step("routing target");
                                let mut targets = Vec::new();
                                if let Some(cname) = &res.cname {
                                    targets.push(format!(
                                        "{:<8}  {:<36}  {}",
                                        "CNAME", cname.name, cname.target
                                    ));
                                }
                                if let Some(apex) = &res.apex {
                                    targets.push(format!(
                                        "{:<8}  {:<36}  {}",
                                        "A", apex.name, apex.ip
                                    ));
                                }
                                info(&targets.join("\n"));
                            }
                            if !res.verification.is_empty() {
                                step("required dns records");
                                let dns_lines: Vec<String> = res
                                    .verification
                                    .iter()
                                    .map(|v| {
                                        format!(
                                            "{:<8}  {:<32}  {}",
                                            v.r#type.to_uppercase(),
                                            v.domain,
                                            v.value
                                        )
                                    })
                                    .collect();
                                info(&dns_lines.join("\n"));
                            }
                        }
                        outro("done");
                    }
                }
                Err(e) => error(&e),
            }
        }
        DomainsCmd::Inspect { domain } => {
            match deploy::inspect_domain(&canonical, args.target, &domain) {
                Ok(Some(t)) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&t).unwrap());
                    } else {
                        intro("appz domains inspect");
                        let d = &t.domain;
                        let project = t
                            .project_slug
                            .as_deref()
                            .or(t.project_name.as_deref())
                            .or(t.project_id.as_deref())
                            .unwrap_or("-");
                        let mut details = vec![
                            format!("domain:      {}", d.domain),
                            format!("project:     {project}"),
                            format!("status:      {}", d.display_status()),
                            format!("serving:     {}", d.serving_status()),
                        ];
                        if let Some(ssl) = &d.ssl_status {
                            details.push(format!("ssl:         {ssl}"));
                        }
                        if let Some(ts) = d
                            .created_at
                            .and_then(chrono::DateTime::from_timestamp_millis)
                        {
                            details
                                .push(format!("added:       {}", ts.format("%Y-%m-%d %H:%M UTC")));
                        }
                        if let Some(l) = &d.lapsed_at {
                            details.push(format!("lapsed at:   {l}"));
                        }
                        if let Some(r) = &d.removes_at {
                            details.push(format!("removes at:  {r}"));
                        }
                        info(&details.join("\n"));
                        if !d.verified {
                            if let Some(ch) = &d.challenge {
                                step("required ownership challenge");
                                info(&format!("{:<8}  {:<36}  {}", ch.r#type, ch.name, ch.value));
                            }
                            if d.cname.is_some() || d.apex.is_some() {
                                step("routing target");
                                let mut targets = Vec::new();
                                if let Some(c) = &d.cname {
                                    targets.push(format!(
                                        "{:<8}  {:<36}  {}",
                                        "CNAME", c.name, c.target
                                    ));
                                }
                                if let Some(a) = &d.apex {
                                    targets.push(format!("{:<8}  {:<36}  {}", "A", a.name, a.ip));
                                }
                                info(&targets.join("\n"));
                            }
                            warning("domain is not verified");
                            info(&format!(
                                "Configure the DNS records above, then run `appz domains verify {}`",
                                d.domain
                            ));
                        }
                        info(
                            "nameservers and registrar details are not shown: appz.dev is not a domain registrar",
                        );
                        outro("done");
                    }
                }
                Ok(None) => error(&format!(
                    "{domain} is not attached to any project. Run `appz domains add {domain}` to add it"
                )),
                Err(e) => error(&e),
            }
        }
        DomainsCmd::Move {
            domain,
            to,
            project,
        } => {
            if !assume_yes() {
                if json || !is_interactive_tty() {
                    error("confirmation required: pass --yes to move the domain without prompting");
                }
                let go = cliclack::confirm(format!(
                    "Move {domain} to organization {to}? Traffic stops until its DNS and verification are set up there"
                ))
                .initial_value(false)
                .interact()
                .unwrap_or(false);
                if !go {
                    error("cancelled");
                }
            }
            match deploy::move_domain(&canonical, args.target, &domain, &to, &project) {
                Ok(res) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&res).unwrap());
                    } else {
                        intro("appz domains move");
                        if res.status == "pending" {
                            warning(&format!(
                                "{} moved to {}, but routing is still reconciling; re-run the same command to finish",
                                res.domain, res.organization_id
                            ));
                        } else {
                            success(&format!(
                                "moved {} to organization {} (project {})",
                                res.domain, res.organization_id, res.project_id
                            ));
                        }
                        outro("done");
                    }
                }
                Err(e) => error(&e),
            }
        }
        DomainsCmd::Remove { domain } => {
            match deploy::remove_domain(&canonical, args.target, &domain) {
                Ok(_) => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({ "status": "removed", "domain": domain })
                        );
                    } else {
                        intro("appz domains remove");
                        success(&format!("removed domain {domain} from project"));
                        outro("done");
                    }
                }
                Err(e) => error(&e),
            }
        }
    }
}

fn project_restore_command(dir: &Path, name: &str) -> String {
    let dir = dir.to_string_lossy();
    #[cfg(windows)]
    let quoted = {
        // PowerShell also treats typographic single quotes as string delimiters.
        let mut escaped = String::new();
        for c in dir.chars() {
            if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}') {
                escaped.push('\'');
            }
            escaped.push(c);
        }
        escaped
    };
    #[cfg(not(windows))]
    let quoted = dir.replace('\'', "'\\''");
    format!("appz projects '{quoted}' --target appzdev restore {name}")
}

fn run_projects(args: ProjectsArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    match args.command {
        ProjectsCmd::List => match deploy::list_projects(&canonical, args.target) {
            Ok(projects) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&projects).unwrap());
                } else {
                    intro("appz projects");
                    if projects.is_empty() {
                        info(
                            "no projects found in active organization\nrun `appz projects add <name>` to create a project",
                        );
                    } else {
                        let lines: Vec<String> = projects
                            .iter()
                            .map(|p| {
                                let fw = p.framework.as_deref().unwrap_or("-");
                                format!("{:<30}  id:{:<24}  framework:{:<12}", p.name, p.id, fw)
                            })
                            .collect();
                        info(&lines.join("\n"));
                    }
                    outro("done");
                }
            }
            Err(e) => error(&e),
        },
        ProjectsCmd::Add { name } => match deploy::add_project(&canonical, args.target, &name) {
            Ok(p) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&p).unwrap());
                } else {
                    intro("appz projects add");
                    success(&format!("created project {}", p.name));
                    info(&format!("id:   {}\nslug: {}", p.id, p.slug));
                    outro("done");
                }
            }
            Err(e) => error(&e),
        },
        ProjectsCmd::Inspect { name } => {
            match deploy::get_project(&canonical, args.target, &name) {
                Ok(p) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&p).unwrap());
                    } else {
                        intro("appz projects inspect");
                        let mut lines = vec![
                            format!("name:            {}", p.name),
                            format!("id:              {}", p.id),
                            format!("slug:            {}", p.slug),
                        ];
                        if let Some(org) = &p.organization_id {
                            lines.push(format!("organization:    {}", org));
                        }
                        if let Some(url) = &p.production_url {
                            lines.push(format!("production url:  {}", url));
                        }
                        info(&lines.join("\n"));
                        outro("done");
                    }
                }
                Err(e) => error(&e),
            }
        }
        ProjectsCmd::Rename { name, new_name } => {
            match deploy::rename_project(&canonical, args.target, &name, &new_name) {
                Ok(p) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&p).unwrap());
                    } else {
                        intro("appz projects rename");
                        success(&format!("renamed project to {}", p.name));
                        info(&format!("id:   {}\nslug: {}", p.id, p.slug));
                        outro("done");
                    }
                }
                Err(e) => error(&e),
            }
        }
        ProjectsCmd::Restore { name } => {
            match deploy::restore_project(&canonical, args.target, &name) {
                Ok(project) if json => {
                    println!("{}", serde_json::to_string_pretty(&project).unwrap())
                }
                Ok(project) => success(&format!(
                    "restored project {} ({})",
                    project.slug, project.id
                )),
                Err(e) => error(&e),
            }
        }
        ProjectsCmd::Pause { name } => project_ops::lifecycle(
            &canonical,
            args.target,
            &name,
            project_ops::Lifecycle::Pause,
            json,
        ),
        ProjectsCmd::Resume { name } => project_ops::lifecycle(
            &canonical,
            args.target,
            &name,
            project_ops::Lifecycle::Resume,
            json,
        ),
        ProjectsCmd::Keep { name } => project_ops::lifecycle(
            &canonical,
            args.target,
            &name,
            project_ops::Lifecycle::Keep,
            json,
        ),
        ProjectsCmd::Update {
            project: name,
            new_name,
            toolbar_mode,
            markdown,
            deployment_protection_disabled,
        } => {
            let upd = appz_deploy::ProjectSettingsUpdate {
                name: new_name,
                toolbar_mode,
                markdown,
                deployment_protection_disabled,
            };
            project_ops::update(&canonical, args.target, &name, upd, json)
        }
        ProjectsCmd::Transfer { name, to } => {
            project_ops::transfer(&canonical, args.target, &name, &to, json)
        }
        ProjectsCmd::Remove { name } => {
            let deadline = chrono::Utc::now() + chrono::Duration::days(30);
            match deploy::remove_project(&canonical, args.target, &name) {
                Ok(_) => {
                    let command = project_restore_command(&canonical, &name);
                    let deadline = deadline.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({ "status": "removed", "project": name,
                                "estimated_restore_deadline": deadline, "restore_command": command })
                        );
                    } else {
                        intro("appz projects remove");
                        success(&format!("removed project {name}"));
                        info(&format!(
                            "Recover within 30 days (approximately {deadline}). Restore before deploying to the same slug again.\nRun: {command}"
                        ));
                        outro("done");
                    }
                }
                Err(e) => error(&e),
            }
        }
    }
}

fn run_transfers(args: TransfersArgs, json: bool) {
    let dir = resolve_root(&args.dir);
    let target = args.target;
    match args.command {
        TransfersCmd::Ls => project_ops::transfers_list(&dir, target, json),
        TransfersCmd::Inspect { id } => project_ops::transfers_inspect(&dir, target, &id, json),
        TransfersCmd::Accept { id, timeout } => {
            project_ops::transfers_accept(&dir, target, &id, timeout, json)
        }
        TransfersCmd::Cancel { id } => project_ops::transfers_cancel(&dir, target, &id, json),
    }
}

fn run_login(args: LoginArgs, json: bool) {
    let api_base =
        std::env::var("APPZ_API_URL").unwrap_or_else(|_| "https://api.appz.dev".to_string());
    if !json {
        intro("appz login");
    }

    let code_resp = match appz_deploy::request_device_code(&api_base) {
        Ok(c) => c,
        Err(e) => {
            error(&e.to_string());
        }
    };

    let verify_url = code_resp
        .verification_uri_complete
        .as_deref()
        .unwrap_or(&code_resp.verification_uri);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "user_code": code_resp.user_code,
                "verification_uri": code_resp.verification_uri,
                "verification_uri_complete": code_resp.verification_uri_complete,
                "expires_in": code_resp.expires_in,
                "interval": code_resp.interval,
            })
        );
    } else {
        step("device authorization");
        info(&format!(
            "1. Confirm code in browser:  {}",
            code_resp.user_code
        ));
        info(&format!("2. Open URL:                  {}", verify_url));

        if !args.no_browser {
            open_in_browser(verify_url);
        }

        info("waiting for authorization...");
    }

    let interval = Duration::from_secs(code_resp.interval.max(2));
    let start = Instant::now();
    let max_duration = Duration::from_secs(code_resp.expires_in.max(60));

    loop {
        if start.elapsed() > max_duration {
            error("Device authorization timed out. Please try again.");
        }
        std::thread::sleep(interval);

        match appz_deploy::poll_device_token(&api_base, &code_resp.device_code) {
            appz_deploy::DevicePollResult::Success(token) => {
                let session_url = format!("{api_base}/api/auth/get-session");
                let session_res = ureq::get(&session_url)
                    .set("Authorization", &format!("Bearer {token}"))
                    .call();
                if matches!(session_res, Err(ureq::Error::Status(401, _))) {
                    error(
                        "login was not completed: the new session is not authorized (finish any two-factor verification in your browser, then run `appz login` again)",
                    );
                }

                let mut user_id = None;
                let mut email = None;
                let mut organization_id = None;

                if let Some(v) = session_res
                    .ok()
                    .and_then(|r| r.into_json::<serde_json::Value>().ok())
                {
                    user_id = v
                        .get("user")
                        .and_then(|u| u.get("id"))
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    email = v
                        .get("user")
                        .and_then(|u| u.get("email"))
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    organization_id = v
                        .get("session")
                        .and_then(|s| s.get("activeOrganizationId"))
                        .and_then(|s| s.as_str())
                        .map(String::from);
                }

                let stored = appz_deploy::StoredAuth {
                    token,
                    user_id,
                    email: email.clone(),
                    organization_id: organization_id.clone(),
                };

                if let Err(e) = appz_deploy::save_stored_auth(&stored) {
                    error(&format!("Failed to save session token: {e}"));
                }

                if json {
                    println!("{}", serde_json::to_string_pretty(&stored).unwrap());
                } else {
                    success(&format!(
                        "Logged in as {}{}",
                        email.unwrap_or_else(|| "authorized user".to_string()),
                        organization_id
                            .map(|o| format!(" (org: {o})"))
                            .unwrap_or_default()
                    ));
                    outro("ready to deploy");
                }
                return;
            }
            appz_deploy::DevicePollResult::Pending => {}
            appz_deploy::DevicePollResult::SlowDown => {
                std::thread::sleep(Duration::from_secs(5));
            }
            appz_deploy::DevicePollResult::Failed(err) => {
                error(&err);
            }
        }
    }
}

fn run_whoami(json: bool) {
    let token = appz_deploy::resolve_auth_token(None);
    let Some(t) = token else {
        if json {
            println!("{}", serde_json::json!({ "authenticated": false }));
        } else {
            ui::hint("Not logged in. Run `appz login` or set APPZ_TOKEN");
        }
        return;
    };
    let stored = appz_deploy::load_stored_auth();
    let api_base =
        std::env::var("APPZ_API_URL").unwrap_or_else(|_| "https://api.appz.dev".to_string());
    let session_url = format!("{api_base}/api/auth/get-session");
    let session = ureq::get(&session_url)
        .set("Authorization", &format!("Bearer {t}"))
        .call()
        .ok()
        .and_then(|r| r.into_json::<serde_json::Value>().ok());
    let (email, org) = match session {
        Some(v) => (
            v.get("user")
                .and_then(|u| u.get("email"))
                .and_then(|s| s.as_str())
                .map(String::from),
            v.get("session")
                .and_then(|s| s.get("activeOrganizationId"))
                .and_then(|s| s.as_str())
                .map(String::from),
        ),
        None => (
            stored.as_ref().and_then(|s| s.email.clone()),
            stored.as_ref().and_then(|s| s.organization_id.clone()),
        ),
    };
    if json {
        println!(
            "{}",
            serde_json::json!({
                "authenticated": true,
                "email": email,
                "organizationId": org,
            })
        );
    } else {
        ui::print_kv(&[
            (
                "User",
                email.unwrap_or_else(|| "authenticated via token".into()),
            ),
            ("Organization", org.unwrap_or_default()),
        ]);
    }
}

fn run_logout(json: bool) {
    match appz_deploy::clear_stored_auth() {
        Ok(()) => {
            if json {
                println!("{}", serde_json::json!({ "status": "logged_out" }));
            } else {
                success("Logged out");
            }
        }
        Err(e) => error(&format!("failed to clear session token: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn usage(label: &str, used: f64, limit: f64, detail: &str) -> appz_deploy::UsageEntry {
        appz_deploy::UsageEntry {
            label: label.into(),
            used,
            limit,
            detail: detail.into(),
        }
    }

    #[test]
    fn deployment_status_marks_current() {
        console::set_colors_enabled(false);
        assert_eq!(deployment_status("READY", true), "● Ready (current)");
        assert_eq!(deployment_status("error", false), "● Error");
    }

    #[test]
    fn usage_row_prefers_api_detail_and_shows_percent() {
        console::set_colors_enabled(false);
        assert_eq!(
            usage_row(&usage(
                "Stored bytes",
                335005295.0,
                5368709120.0,
                "0.31 GB of 5.00 GB"
            )),
            vec!["Stored bytes", "0.31 GB of 5.00 GB", "6%"]
        );
        assert_eq!(
            usage_row(&usage("Projects", 20.0, 100.0, "")),
            vec!["Projects", "20 of 100", "20%"]
        );
        assert_eq!(
            usage_row(&usage("Seats", 3.0, 0.0, "")),
            vec!["Seats", "3 (unlimited)", ""]
        );
    }

    #[test]
    fn project_lifecycle_and_transfer_commands_parse() {
        let parse = |args: &[&str]| {
            AppzCli::try_parse_from(std::iter::once("appz").chain(args.iter().copied()))
        };
        let AppzCmd::Projects(a) = parse(&["projects", "pause", "demo", "--yes"])
            .unwrap()
            .command
        else {
            panic!()
        };
        assert!(matches!(a.command, ProjectsCmd::Pause { name } if name == "demo"));
        let AppzCmd::Projects(a) = parse(&[
            "projects",
            "update",
            "demo",
            "--name",
            "New",
            "--toolbar-mode",
            "all",
            "--markdown",
            "true",
        ])
        .unwrap()
        .command
        else {
            panic!()
        };
        assert!(
            matches!(a.command, ProjectsCmd::Update { project, new_name: Some(n), markdown: Some(true), .. }
            if project == "demo" && n == "New")
        );
        assert!(parse(&["projects", "update", "demo", "--toolbar-mode", "bogus"]).is_err());
        assert!(parse(&["projects", "transfer", "demo"]).is_err());
        let AppzCmd::Transfers(t) = parse(&["transfers", "accept", "trf_1", "--timeout", "5"])
            .unwrap()
            .command
        else {
            panic!()
        };
        assert!(matches!(t.command, TransfersCmd::Accept { id, timeout: 5 } if id == "trf_1"));
        assert!(parse(&["transfers", "rm", "trf_1"]).is_ok());
    }

    #[test]
    fn domains_inspect_and_move_parse() {
        let cli = AppzCli::try_parse_from(["appz", "domains", "inspect", "a.example.com"]).unwrap();
        assert!(matches!(
            cli.command,
            AppzCmd::Domains(DomainsArgs {
                command: DomainsCmd::Inspect { .. },
                ..
            })
        ));
        let cli = AppzCli::try_parse_from([
            "appz",
            "domains",
            "move",
            "a.example.com",
            "--to",
            "org_x",
            "--project",
            "prj_y",
        ])
        .unwrap();
        match cli.command {
            AppzCmd::Domains(DomainsArgs {
                command: DomainsCmd::Move { to, project, .. },
                ..
            }) => assert_eq!((to.as_str(), project.as_str()), ("org_x", "prj_y")),
            _ => panic!("expected domains move"),
        }
        assert!(
            AppzCli::try_parse_from(["appz", "domains", "move", "a.example.com", "--to", "org_x"])
                .is_err(),
            "--project is required"
        );
    }

    #[test]
    fn remove_parses_multiple_ids_and_rm_alias() {
        let cli = AppzCli::try_parse_from(["appz", "rm", "dpl_a", "dpl_b", "--yes"]).unwrap();
        let AppzCmd::Remove(a) = cli.command else {
            panic!("expected remove")
        };
        assert_eq!(a.deployments, ["dpl_a", "dpl_b"]);
        assert!(AppzCli::try_parse_from(["appz", "remove"]).is_err());
    }

    #[test]
    fn restore_hint_preserves_the_project_directory() {
        let dir = Path::new("team's project $archive");
        let hint = project_restore_command(dir, "demo");
        #[cfg(windows)]
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Command",
                &format!("function appz {{ $args }}; {hint}"),
            ])
            .output()
            .unwrap();
        #[cfg(not(windows))]
        let output = std::process::Command::new("sh")
            .args([
                "-c",
                &format!("appz() {{ printf '%s\\n' \"$@\"; }}; {hint}"),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let cli = AppzCli::try_parse_from(std::iter::once("appz").chain(stdout.lines())).unwrap();
        let AppzCmd::Projects(args) = cli.command else {
            panic!("expected projects")
        };
        assert_eq!(args.dir, dir);
        assert_eq!(args.target.as_deref(), Some("appzdev"));
        assert!(matches!(args.command, ProjectsCmd::Restore { name } if name == "demo"));
    }

    #[test]
    fn framework_install_recovers_missing_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let detected = [appz_core::DetectedToolchain {
            name: "Next.js",
            slug: "nextjs",
            mise_plugin: "node",
            version: "lts".into(),
            version_files: &[],
            frameworks: vec![],
            build_command: None,
            install_command: Some("echo installed >> marker.txt".into()),
            dev_command: None,
            test_command: None,
            lint_command: None,
            format_command: None,
            format_tool: None,
            output_directory: None,
            env_prefix: None,
        }];
        assert_eq!(
            cached_install_args(root, &detected),
            ["run", "--force", "install"]
        );
        for marker in [".pnp.js", ".pnp.cjs"] {
            std::fs::write(root.join(marker), "").unwrap();
            assert_eq!(cached_install_args(root, &detected), ["run", "install"]);
            std::fs::remove_file(root.join(marker)).unwrap();
        }
        std::fs::create_dir(root.join("node_modules")).unwrap();
        assert_eq!(cached_install_args(root, &detected), ["run", "install"]);

        install_dependencies(root, &detected, false);
        assert_eq!(
            std::fs::read_to_string(root.join("marker.txt"))
                .unwrap()
                .trim(),
            "installed"
        );
    }

    /// clap checks the whole command tree here (argument order, duplicate
    /// names and short flags, conflicting settings), so an invalid definition
    /// fails the test suite instead of panicking for a user.
    #[test]
    fn cli_definition_is_valid() {
        AppzCli::command().debug_assert();
    }

    #[test]
    fn activity_and_usage_parse() {
        assert!(
            AppzCli::try_parse_from(["appz", "activity", "--limit", "20", "--next", "c"]).is_ok()
        );
        assert!(AppzCli::try_parse_from(["appz", "activity", "--limit", "0"]).is_err());
        assert!(AppzCli::try_parse_from(["appz", "--json", "usage"]).is_ok());
        assert_eq!(usage_percent(8.0, 10.0), Some(80.0));
        assert_eq!(usage_percent(8.0, 0.0), None);
    }

    #[test]
    fn deploy_accepts_prebuilt() {
        assert!(AppzCli::try_parse_from(["appz", "deploy", "--prebuilt", "--prod"]).is_ok());
    }

    #[test]
    fn promote_takes_deployment_then_optional_dir() {
        assert!(AppzCli::try_parse_from(["appz", "promote", "dpl_abc123def456"]).is_ok());
        let cli = AppzCli::try_parse_from([
            "appz",
            "promote",
            "https://x.appz.to",
            "site",
            "--target",
            "appzdev",
            "-y",
            "--json",
        ])
        .unwrap();
        assert!(cli.yes && cli.json);
        assert!(AppzCli::try_parse_from(["appz", "promote"]).is_err());
    }

    #[test]
    fn rollback_and_alias_take_the_required_argument_first() {
        assert!(AppzCli::try_parse_from(["appz", "rollback", "v42"]).is_ok());
        assert!(AppzCli::try_parse_from(["appz", "rollback", "v42", "site"]).is_ok());
        assert!(AppzCli::try_parse_from(["appz", "rollback"]).is_ok());
        assert!(AppzCli::try_parse_from(["appz", "rollback", "some/dir"]).is_ok());
        assert!(AppzCli::try_parse_from(["appz", "alias", "app.example.com"]).is_ok());
        assert!(AppzCli::try_parse_from(["appz", "alias", "ls", "extra", "more"]).is_err());
        match AppzCli::try_parse_from([
            "appz",
            "ls",
            "--status",
            "ready,error",
            "--status",
            "building",
            "--since",
            "7d",
            "--meta",
            "a=b",
            "--meta",
            "c=d",
        ])
        .unwrap()
        .command
        {
            AppzCmd::List(a) => {
                assert_eq!(a.status, ["ready", "error", "building"]);
                assert_eq!(a.meta.len(), 2);
                let q = build_list_query(&a, None).unwrap();
                assert!(q.since.is_some() && q.until.is_none());
                assert_eq!(list_filters_json(&q)["meta"]["c"], "d");
            }
            _ => panic!("expected list"),
        }
        assert!(AppzCli::try_parse_from(["appz", "ls", "--prod", "--environment", "x"]).is_err());
    }

    #[test]
    fn alias_shorthand_and_subcommands_parse() {
        let parse = |args: &[&str]| {
            let cli = AppzCli::try_parse_from(std::iter::once("appz").chain(args.iter().copied()))
                .unwrap();
            let AppzCmd::Alias(a) = cli.command else {
                panic!("expected alias")
            };
            a
        };
        let a = parse(&["alias", "app.example.com", "site", "--target", "appzdev"]);
        assert!(a.command.is_none());
        assert_eq!(a.domain.as_deref(), Some("app.example.com"));
        assert_eq!(a.dir, PathBuf::from("site"));
        assert_eq!(a.target.as_deref(), Some("appzdev"));
        let a = parse(&["alias", "set", "app.example.com"]);
        assert!(
            matches!(a.command, Some(AliasCmd::Set { domain, .. }) if domain == "app.example.com")
        );
        assert!(matches!(
            parse(&["alias", "ls"]).command,
            Some(AliasCmd::Ls { .. })
        ));
        assert!(matches!(
            parse(&["alias", "list"]).command,
            Some(AliasCmd::Ls { .. })
        ));
        let a = parse(&["alias", "rm", "beta.example.com", "--target", "appzdev"]);
        assert!(matches!(a.command, Some(AliasCmd::Rm { host, target, .. })
            if host == "beta.example.com" && target.as_deref() == Some("appzdev")));
        assert!(AppzCli::try_parse_from(["appz", "alias", "rm"]).is_err());
        let cli =
            AppzCli::try_parse_from(["appz", "alias", "rm", "b.example.com", "--yes"]).unwrap();
        assert!(cli.yes);

        match AppzCli::try_parse_from(["appz", "deploy", "--no-backup"])
            .unwrap()
            .command
        {
            AppzCmd::Deploy(a) => assert!(a.no_backup),
            _ => panic!("not deploy"),
        }

        for args in [
            &["appz", "backup"][..],
            &["appz", "backup", "create"],
            &["appz", "backup", "ls"],
            &["appz", "backup", "list"],
        ] {
            assert!(
                matches!(
                    AppzCli::try_parse_from(args).unwrap().command,
                    AppzCmd::Backup(_)
                ),
                "{args:?}"
            );
        }
    }

    #[test]
    fn yes_is_accepted_before_and_after_any_subcommand() {
        for args in [
            &["appz", "-y", "share"][..],
            &["appz", "share", "-y"],
            &["appz", "share", "--yes", "3000"],
            &["appz", "dev", "--share", "-y"],
            &["appz", "--yes", "deploy"],
            &["appz", "deploy", "-y"],
            &["appz", "ci", "init", "-y"],
            &["appz", "skills", "-y"],
        ] {
            let cli = AppzCli::try_parse_from(args).unwrap_or_else(|e| panic!("{args:?}: {e}"));
            assert!(cli.yes, "{args:?}");
        }
        assert!(!AppzCli::try_parse_from(["appz", "share"]).unwrap().yes);
    }

    /// The same options in any order mean the same thing. `--pin` takes its
    /// code only as `--pin=CODE`, so a port after a bare `--pin` stays a port.
    #[test]
    fn open_parses_optional_id_dir_and_dashboard() {
        let parse = |args: &[&str]| match AppzCli::try_parse_from(args).unwrap().command {
            AppzCmd::Open(a) => (a.id_or_url, a.dir, a.dashboard),
            _ => panic!("not open"),
        };
        assert_eq!(parse(&["appz", "open"]), (None, PathBuf::from("."), false));
        assert_eq!(
            parse(&["appz", "open", "dpl_1", "app", "--dashboard"]),
            (Some("dpl_1".into()), PathBuf::from("app"), true)
        );
    }

    #[test]
    fn share_options_parse_the_same_in_any_order() {
        let parse = |args: &[&str]| match AppzCli::try_parse_from(args).unwrap().command {
            AppzCmd::Share(a) => (a.port, a.pin, a.inspect),
            _ => panic!("not share"),
        };
        let want = (Some("3000".to_string()), Some("auto".to_string()), true);
        assert_eq!(
            parse(&["appz", "share", "3000", "--pin", "--inspect"]),
            want
        );
        assert_eq!(
            parse(&["appz", "share", "--pin", "3000", "--inspect"]),
            want
        );
        assert_eq!(
            parse(&["appz", "share", "--inspect", "--pin", "3000"]),
            want
        );
        assert_eq!(
            parse(&["appz", "-y", "share", "--inspect", "3000", "--pin"]),
            want
        );
        assert_eq!(
            parse(&["appz", "share", "--pin=8492", "3000"]).1.as_deref(),
            Some("8492")
        );
        // `ci init --no-secrets` still refuses to be combined with --yes.
        assert!(AppzCli::try_parse_from(["appz", "ci", "init", "--no-secrets", "-y"]).is_err());
    }

    #[test]
    fn vault_and_env_subcommands_parse_uniformly() {
        for top in ["vault", "env"] {
            assert!(matches!(
                AppzCli::try_parse_from(["appz", top, "set", "VERCEL_TOKEN"])
                    .unwrap()
                    .command,
                AppzCmd::Vault(_) | AppzCmd::Env(_)
            ));
            assert!(
                AppzCli::try_parse_from(["appz", top, "set", "VERCEL_TOKEN", "val", "--global"])
                    .is_ok()
            );
            assert!(AppzCli::try_parse_from(["appz", top, "ls", "--global"]).is_ok());
            assert!(AppzCli::try_parse_from(["appz", top, "rm", "VERCEL_TOKEN"]).is_ok());
            assert!(AppzCli::try_parse_from(["appz", top, "unlock", "--global"]).is_ok());
            assert!(AppzCli::try_parse_from(["appz", top, "lock"]).is_ok());
        }
        assert!(AppzCli::try_parse_from(["appz", "env", "pull"]).is_ok());
        assert!(AppzCli::try_parse_from(["appz", "env", "push"]).is_ok());
        // Per-environment cloud commands; the vault `ls`/`rm` keep working unchanged.
        let ok = |a: &[&str]| AppzCli::try_parse_from(a).is_ok();
        assert!(ok(&["appz", "env", "pull", "--environment", "staging"]));
        assert!(ok(&["appz", "env", "pull", "--target", "preview"]));
        assert!(ok(&[
            "appz",
            "env",
            "push",
            "--environment",
            "production,preview"
        ]));
        assert!(ok(&[
            "appz",
            "env",
            "cloud",
            "ls",
            "--environment",
            "preview"
        ]));
        assert!(ok(&["appz", "env", "cloud", "rm", "API_KEY", "--yes"]));
        assert!(ok(&["appz", "env", "environments", "ls"]));
        assert!(ok(&[
            "appz",
            "env",
            "environments",
            "add",
            "qa",
            "--branch-pattern",
            "qa/*"
        ]));
        assert!(ok(&["appz", "env", "environments", "rm", "qa", "--yes"]));
        assert!(!ok(&["appz", "env", "run"]));
        match AppzCli::try_parse_from([
            "appz",
            "env",
            "run",
            "--environment",
            "preview",
            "--",
            "node",
            "-e",
            "1",
        ])
        .unwrap()
        .command
        {
            AppzCmd::Env(EnvArgs {
                command: EnvCmd::Run { environment, cmd },
            }) => {
                assert_eq!(environment, ["preview"]);
                assert_eq!(cmd, ["node", "-e", "1"]);
            }
            _ => panic!("expected env run"),
        }
        // Cloud .env sync stays under `appz env`, not `appz vault`.
        assert!(AppzCli::try_parse_from(["appz", "vault", "pull"]).is_err());
        assert!(AppzCli::try_parse_from(["appz", "vault", "push"]).is_err());
    }
}
