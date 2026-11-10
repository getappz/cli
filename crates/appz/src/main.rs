use clap::{Args, Parser, Subcommand};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use appz_core::{detect_toolchains, generate_claude_md, run_doctor};

mod agents;
// Scaffolding: `appz backup` subcommands are wired up in a later task.
#[allow(dead_code)]
mod backup;
mod build_stage;
mod deploy;
mod deploy_state;
mod dev_install;
mod init_source;
mod mcp;
mod secrets;
mod skills;
mod update;

// ── CLI ─────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "appz", version)]
struct AppzCli {
    /// JSON output mode (for agent/CI use)
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: AppzCmd,
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
    /// Run tests
    Test(TestArgs),
    /// Run linter
    Lint(LintArgs),
    /// Run code formatter
    Format(FormatArgs),
    /// Remove build/cache output for this project (dry-run unless --force)
    Clean(CleanArgs),
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
    /// Link this directory to a project on the platform
    Link(LinkArgs),
    /// Point a custom domain or alias to the deployment
    Alias(AliasArgs),
    /// List deployments for the project
    #[command(alias = "deployments", alias = "ls")]
    List(ListArgs),
    /// Inspect edge deployment manifest, hosts, and CAS metrics
    Inspect(InspectArgs),
    /// Manage custom domains and DNS verification on Cloudflare Edge
    #[command(alias = "domain")]
    Domains(DomainsArgs),
    /// Manage projects on the edge hosting platform
    #[command(alias = "project")]
    Projects(ProjectsArgs),
    /// Log in to Appz via browser device flow
    Login(LoginArgs),
    /// Log out of Appz and remove the stored session token
    Logout,
    /// Print currently authenticated user and active organization
    Whoami,
    /// Store credentials in an encrypted vault used by `appz deploy`
    Env(EnvArgs),
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
    /// Install all recommended/matching skills without prompting
    #[arg(short = 'y', long)]
    yes: bool,
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
struct EnvArgs {
    #[command(subcommand)]
    command: EnvCmd,
}

#[derive(Subcommand)]
enum EnvCmd {
    /// Store a secret (omit VALUE to be prompted, keeping it out of shell history)
    Set {
        name: String,
        value: Option<String>,
        /// Use the user-wide vault (~/.appz) instead of this project's
        #[arg(long)]
        global: bool,
    },
    /// List stored names (never values)
    Ls {
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
    /// Pull cloud environment variables from Cloudflare Edge into a local .env file
    Pull {
        /// Target file to write to (default: .env.local)
        #[arg(default_value = ".env.local")]
        file: PathBuf,
        /// Target environment filter (production, preview, development)
        #[arg(long)]
        target: Option<String>,
    },
    /// Push local environment variables from a .env file to Cloudflare Edge
    Push {
        /// Source file to read from (default: .env.local, falls back to .env)
        #[arg(default_value = ".env.local")]
        file: PathBuf,
        /// Target environments to associate (default: production and preview)
        #[arg(long, value_delimiter = ',')]
        target: Option<Vec<String>>,
    },
}

#[derive(Args)]
struct DeployArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (appzdev, vercel, netlify, cloudflare, fly, firebase, deno-deploy, azure-static, aws-s3, aws-sst, railway, surge, github-pages). Uses appz.jsonc's deploy.default if omitted, else detects it from project files.
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
    /// Install a missing platform CLI through mise without asking
    #[arg(short = 'y', long)]
    yes: bool,
    /// Deploy to every target configured under deploy.targets instead of one
    #[arg(long, conflicts_with = "target")]
    all: bool,
}

#[derive(Args)]
struct RollbackArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Version ID to roll back to
    version: String,
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

#[derive(Args)]
struct AliasArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Custom domain or alias to assign (e.g. app.example.com)
    domain: String,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct ListArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (default: detects from project or config)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args)]
struct InspectArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Deployment ID or live URL to inspect
    id_or_url: String,
    /// Also display all files contained in the deployment manifest
    #[arg(long)]
    files: bool,
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
    /// List all custom domains configured for this project
    #[command(alias = "ls")]
    List,
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
    /// Remove a project from the platform
    #[command(alias = "rm")]
    Remove {
        /// Name or ID of the project to remove
        name: String,
    },
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
}

// ── UI helpers ──────────────────────────────────────────────────

fn is_tty() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

const BRAND: &str = "+-+-+-+-+\n|a|p|p|z|\n+-+-+-+-+";

/// Shown once, at `appz init` — a first-run touch, not decoration on every
/// command. Skipped outside a real terminal so it never lands in piped/CI
/// output.
fn print_brand() {
    if is_tty() {
        println!("{BRAND}");
    }
}

fn intro(msg: &str) {
    if is_tty() {
        let _ = cliclack::intro(msg);
    } else {
        println!("> {msg}");
    }
}

fn outro(msg: &str) {
    if is_tty() {
        let _ = cliclack::outro(msg);
    } else {
        println!("✓ {msg}");
    }
}

fn success(msg: &str) {
    if is_tty() {
        let _ = cliclack::log::success(msg);
    } else {
        println!("ok  {msg}");
    }
}

fn info(msg: &str) {
    if is_tty() {
        let _ = cliclack::log::info(msg);
    } else {
        for line in msg.lines() {
            println!("    {line}");
        }
    }
}

fn step(msg: &str) {
    if is_tty() {
        let _ = cliclack::log::step(msg);
    } else {
        println!("  • {msg}");
    }
}

fn warning(msg: &str) {
    if is_tty() {
        let _ = cliclack::log::warning(msg);
    } else {
        eprintln!("warn {msg}");
    }
}

fn error(msg: &str) -> ! {
    if is_tty() {
        let _ = cliclack::log::error(msg);
    } else {
        eprintln!("fail {msg}");
    }
    std::process::exit(1);
}

fn with_spinner<T>(start: &str, done: &str, f: impl FnOnce() -> T) -> T {
    if !is_tty() {
        println!("  {start}");
        let r = f();
        println!("  {done}");
        return r;
    }
    let sp = cliclack::spinner();
    sp.start(start);
    let r = f();
    sp.stop(done);
    r
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

fn detect_package_manager(root: &Path) -> Option<&'static str> {
    if root.join("pnpm-lock.yaml").exists() {
        Some("pnpm")
    } else if root.join("yarn.lock").exists() {
        Some("yarn")
    } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
        Some("bun")
    } else if root.join("package-lock.json").exists() {
        Some("npm")
    } else {
        None
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

fn do_mise_install(root: &Path) {
    if let Err(e) = appz_core::ensure_mise(root) {
        warning(&e);
        return;
    }
    if let Err(e) = appz_core::trust(root) {
        warning(&format!("mise trust: {e}"));
    }

    with_spinner("installing mise tools...", "mise tools ready", || {
        run_cmd("mise", &["install"], root);
    });
}

fn do_pm_install(root: &Path, toolchains: &[appz_core::DetectedToolchain]) {
    let has_node = toolchains.iter().any(|t| t.slug == "node");
    let has_rust = toolchains.iter().any(|t| t.slug == "rust");

    if has_node {
        let pm = detect_package_manager(root).unwrap_or("npm");
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
    if toolchains.iter().any(|t| t.install_command.is_some()) {
        run_cmd("mise", &["run", "install"], root);
    } else {
        do_pm_install(root, toolchains);
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
            do_mise_install(&canonical);
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
        do_mise_install(&canonical);
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
    if json {
        let canonical = resolve_root(&args.dir);
        let toolchains = do_detect_and_write(&canonical);
        if !args.skip_install {
            do_mise_install(&canonical);
            do_pm_install(&canonical, &toolchains);
        }
        json_out(&toolchains, "ok");
        return;
    }
    intro("appz dev");
    let canonical = resolve_root(&args.dir);

    // Cache hit
    if args.command.is_none()
        && !args.skip_install
        && appz_core::inputs_unchanged(&canonical)
        && let Some(snap) = appz_core::read_latest_snapshot(&canonical)
    {
        let cmds: Vec<String> = snap
            .toolchains
            .iter()
            .filter_map(|t| t.dev_command.clone())
            .collect();
        if !cmds.is_empty() {
            success("inputs unchanged — running cached dev command");
            for cmd in &cmds {
                let parts: Vec<&str> = cmd.split(' ').collect();
                let (prog, args) = parts.split_first().unwrap_or((&"npm", &[]));
                if !run_cmd(prog, args, &canonical) {
                    std::process::exit(1);
                }
            }
            outro("dev server stopped");
            return;
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

    step("starting dev server...");
    for cmd in &cmds {
        let parts: Vec<&str> = cmd.split(' ').collect();
        let (prog, args) = parts.split_first().unwrap_or((&"npm", &[]));
        if !run_cmd(prog, args, &canonical) {
            std::process::exit(1);
        }
    }
    outro("dev server stopped");
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
    let cli = AppzCli::parse();
    let json = cli.json;
    match cli.command {
        AppzCmd::Init(a) => run_init(a, json),
        AppzCmd::Install(a) => run_install(a, json),
        AppzCmd::Build(a) => run_build(a, json),
        AppzCmd::Dev(a) => run_dev(a, json),
        AppzCmd::Test(a) => run_test(a, json),
        AppzCmd::Lint(a) => run_lint(a, json),
        AppzCmd::Format(a) => run_format(a, json),
        AppzCmd::Clean(a) => run_clean(a, json),
        AppzCmd::Doctor(a) => run_doctor_cmd(a, json),
        AppzCmd::DevInstall(a) => dev_install::run(!a.debug, a.dry_run),
        AppzCmd::Mcp => {
            if let Err(e) = mcp::serve() {
                error(&e);
            }
        }
        AppzCmd::Deploy(a) => run_deploy(a, json),
        AppzCmd::Rollback(a) => run_rollback(a, json),
        AppzCmd::Link(a) => run_link(a, json),
        AppzCmd::Alias(a) => run_alias(a, json),
        AppzCmd::List(a) => run_list(a, json),
        AppzCmd::Inspect(a) => run_inspect(a, json),
        AppzCmd::Domains(a) => run_domains(a, json),
        AppzCmd::Projects(a) => run_projects(a, json),
        AppzCmd::Login(a) => run_login(a, json),
        AppzCmd::Logout => run_logout(json),
        AppzCmd::Whoami => run_whoami(json),
        AppzCmd::Env(a) => run_env(a, json),
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

    for hit in &report.hits {
        let mark = if hit.installed { " (installed)" } else { "" };
        step(&format!(
            "{}  {} installs  {}{mark}",
            hit.name,
            skills::format_installs(hit.installs),
            hit.source
        ));
    }

    let installable: Vec<skills::SkillHit> =
        report.hits.into_iter().filter(|h| !h.installed).collect();
    if installable.is_empty() {
        outro("all recommended skills are already installed");
        return;
    }

    let selected: Vec<skills::SkillHit> = if args.yes {
        installable
    } else if !is_tty() {
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

fn run_env(args: EnvArgs, json: bool) {
    let path =
        |global: bool| secrets::vault_path(Path::new("."), global).unwrap_or_else(|e| error(&e));
    match args.command {
        EnvCmd::Set {
            name,
            value,
            global,
        } => match secrets::set(&path(global), &name, value) {
            Ok(()) => success(&format!("stored {name}")),
            Err(e) => error(&e),
        },
        EnvCmd::Ls { global } => match secrets::names(&path(global)) {
            Ok(names) if json => println!("{}", serde_json::to_string(&names).unwrap()),
            Ok(names) if names.is_empty() => info("no secrets stored"),
            Ok(names) => info(&names.join(
                "
",
            )),
            Err(e) => error(&e),
        },
        EnvCmd::Rm { name, global } => match secrets::remove(&path(global), &name) {
            Ok(true) => success(&format!("removed {name}")),
            Ok(false) => error(&format!("{name} is not stored")),
            Err(e) => error(&e),
        },
        EnvCmd::Unlock { global } => match secrets::unlock(&path(global)) {
            Ok((_, true)) => success("vault unlocked"),
            Ok((_, false)) => warning(concat!(
                "vault unlocked for this command only: the OS keyring is unavailable, ",
                "so it will ask again. Set APPZ_VAULT_PASSPHRASE, or pass the platform ",
                "token as an environment variable instead"
            )),
            Err(e) => error(&e),
        },
        EnvCmd::Lock { global } => {
            secrets::lock(&path(global));
            success("vault locked");
        }
        EnvCmd::Pull { file, target } => {
            let root = resolve_root(Path::new("."));
            let project_id = appz_deploy::resolve_project_id_from_dir(&root);
            let token = appz_deploy::resolve_auth_token(None);
            if token.is_none() {
                error("not authenticated. Run `appz login` or set APPZ_TOKEN");
            }
            let api_base = std::env::var("APPZ_API_URL")
                .unwrap_or_else(|_| "https://api.appz.dev".to_string());
            match appz_deploy::pull_cloud_env(
                &api_base,
                &project_id,
                token.as_deref(),
                target.as_deref(),
            ) {
                Ok(vars) => {
                    let kvs: Vec<(String, String)> =
                        vars.into_iter().map(|v| (v.key, v.value)).collect();
                    let formatted = appz_deploy::format_env_file(&kvs);
                    let target_path = root.join(&file);
                    if let Err(e) = std::fs::write(&target_path, formatted) {
                        error(&format!("failed to write {}: {e}", target_path.display()));
                    }
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({ "pulled": kvs.len(), "file": target_path.display().to_string() })
                        );
                    } else {
                        success(&format!(
                            "downloaded {} environment variables to {}",
                            kvs.len(),
                            target_path.display()
                        ));
                    }
                }
                Err(e) => error(&e.to_string()),
            }
        }
        EnvCmd::Push { file, target } => {
            let root = resolve_root(Path::new("."));
            let project_id = appz_deploy::resolve_project_id_from_dir(&root);
            let token = appz_deploy::resolve_auth_token(None);
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
                }
                return;
            }

            let targets =
                target.unwrap_or_else(|| vec!["production".to_string(), "preview".to_string()]);
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
                    }
                }
                Err(e) => error(&e.to_string()),
            }
        }
    }
}

fn run_deploy(args: DeployArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    let all = args.all;
    let req = deploy::DeployRequest {
        target: args.target,
        project: args.project,
        force: args.force,
        prod: args.prod,
        env: args.env,
        build_env: args.build_env,
        dry_run: args.dry_run,
        json,
        install: if args.yes {
            deploy::Consent::Yes
        } else if !json && is_tty() {
            deploy::Consent::Ask
        } else {
            deploy::Consent::Never
        },
        link: if args.yes {
            deploy::Consent::Yes
        } else if !json && is_tty() {
            deploy::Consent::Ask
        } else {
            deploy::Consent::Never
        },
        // Login needs a browser, so it requires a real terminal even with --yes.
        login: if json || !is_tty() {
            deploy::Consent::Never
        } else if args.yes {
            deploy::Consent::Yes
        } else {
            deploy::Consent::Ask
        },
    };
    if all {
        match deploy::run_all(&canonical, req) {
            Ok(outputs) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&outputs).unwrap());
                } else {
                    intro("appz deploy");
                    for output in &outputs {
                        let target = if output.is_preview {
                            "preview"
                        } else {
                            "production"
                        };
                        success(&format!(
                            "{} ({target}): {}",
                            output.provider,
                            output.url.as_deref().unwrap_or("deployed (url unknown)")
                        ));
                    }
                    outro("deploy complete");
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
                intro("appz deploy");
                success(&format!(
                    "dry run: would deploy to {} ({})",
                    output.provider,
                    if output.is_preview {
                        "preview"
                    } else {
                        "production"
                    }
                ));
                if output.unchanged {
                    info(
                        "unchanged since the last deploy: nothing would be redeployed (--force to deploy anyway)",
                    );
                }
                if output.cli_missing {
                    info("cli: not installed (a real deploy installs it with mise)");
                }
                if output.link_needed {
                    info("project: not linked (a real deploy asks to create/link one)");
                }
                for f in &output.setup_files {
                    info(&format!("would create {f} (the project has none)"));
                }
                if let Some(auth) = output.auth {
                    info(match auth {
                        appz_deploy::AuthState::Authenticated => "auth: signed in",
                        appz_deploy::AuthState::NotAuthenticated => {
                            "auth: not signed in (a real deploy would offer to log in)"
                        }
                        appz_deploy::AuthState::Unknown => "auth: could not be checked",
                    });
                }
                outro("nothing was deployed");
            } else {
                intro("appz deploy");
                if output.unchanged {
                    success(&format!(
                        "unchanged since the last deploy to {}; nothing was redeployed (--force to deploy anyway)",
                        output.provider
                    ));
                    if let Some(url) = &output.url {
                        info(url);
                    }
                    outro("done");
                    return;
                }
                for f in &output.setup_files {
                    info(&format!("created {f} (the project had none)"));
                }
                success(&format!("deployed to {}", output.provider));
                if let Some(url) = &output.url {
                    info(url);
                } else {
                    warning(
                        "deploy succeeded but the live URL could not be confirmed from CLI output",
                    );
                }
                outro("deploy complete");
            }
        }
        Err(e) => error(&e),
    }
}

fn run_rollback(args: RollbackArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    match deploy::rollback(&canonical, args.target, &args.version) {
        Ok(output) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                intro("appz rollback");
                success(&format!(
                    "rolled back {} to version {}",
                    output.provider, args.version
                ));
                if let Some(url) = &output.url {
                    info(url);
                }
                outro("rollback complete");
            }
        }
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
                intro("appz link");
                success(&format!("linked project to {provider_name}"));
                outro("link complete");
            }
        }
        Err(e) => error(&e),
    }
}

fn run_alias(args: AliasArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    match deploy::alias(&canonical, args.target, &args.domain) {
        Ok(url) => {
            if json {
                println!("{}", serde_json::json!({ "status": "aliased", "url": url }));
            } else {
                intro("appz alias");
                success(&format!("assigned alias {}", args.domain));
                info(&url);
                outro("alias complete");
            }
        }
        Err(e) => error(&e),
    }
}

fn run_list(args: ListArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    match deploy::list(&canonical, args.target) {
        Ok(deployments) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&deployments).unwrap());
            } else {
                intro("appz list");
                if deployments.is_empty() {
                    info("no deployments found for this project");
                } else {
                    for d in &deployments {
                        let current_marker = if d.is_current { " (current)" } else { "" };
                        let version = d.version_id.as_deref().unwrap_or("-");
                        let url = d.url.as_deref().unwrap_or("-");
                        let age = d
                            .created_at
                            .map(|t| format!("{}", t.format("%Y-%m-%d %H:%M UTC")))
                            .unwrap_or_else(|| "-".to_string());
                        info(&format!(
                            "{:<16}  {:<14}  {:<32}  {} [{:?}{}]",
                            d.id, version, url, age, d.status, current_marker
                        ));
                    }
                }
                outro("done");
            }
        }
        Err(e) => error(&e),
    }
}

fn run_inspect(args: InspectArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    match deploy::inspect(&canonical, args.target, &args.id_or_url) {
        Ok(insp) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&insp).unwrap());
            } else {
                intro("appz inspect");
                step("deployment");
                info(&format!("id:          {}", insp.id));
                info(&format!("project:     {}", insp.project_id));
                info(&format!("version:     {}", insp.version_id));
                info(&format!("status:      {:?}", insp.status));
                info(&format!("target:      {}", insp.target));
                if let Some(created) = insp.created_at {
                    info(&format!(
                        "created:     {}",
                        created.format("%Y-%m-%d %H:%M:%S UTC")
                    ));
                }

                if !insp.hosts.is_empty() {
                    step("hosts");
                    for h in &insp.hosts {
                        info(&format!("  https://{h}"));
                    }
                }

                step("edge storage & cas");
                info(&format!("files:       {}", insp.file_count));
                info(&format!(
                    "size:        {}",
                    appz_core::format_size(insp.total_bytes)
                ));
                if let Some(mkey) = &insp.manifest_key {
                    info(&format!("manifest:    {mkey}"));
                }

                if !insp.redirects.is_empty() || !insp.headers.is_empty() {
                    step("routing rules");
                    for r in &insp.redirects {
                        info(&format!("redirect:    {r}"));
                    }
                    for h in &insp.headers {
                        info(&format!("header:      {h}"));
                    }
                }

                let files_to_show = if args.files {
                    insp.files.as_deref().unwrap_or_default()
                } else {
                    &[]
                };
                if !files_to_show.is_empty() {
                    step("manifest files");
                    for f in files_to_show {
                        let ct = f.content_type.as_deref().unwrap_or("-");
                        let hash_short = if f.hash.len() > 12 {
                            &f.hash[..12]
                        } else {
                            &f.hash
                        };
                        info(&format!(
                            "{:<36}  {:>8}  {:<24}  hash:{}...",
                            f.path,
                            appz_core::format_size(f.size),
                            ct,
                            hash_short
                        ));
                    }
                }
                outro("inspection complete");
            }
        }
        Err(e) => error(&e),
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
                    info(&format!("domain:      {}", dom.domain));
                    info(&format!("status:      {}", dom.display_status()));
                    if let Some(ssl) = &dom.ssl_status {
                        info(&format!("ssl:         {ssl}"));
                    }
                    if let Some(ch) = &dom.challenge
                        && !dom.verified
                    {
                        step("1. Ownership verification (required once):");
                        info(&format!("{:<8}  {:<36}  {}", ch.r#type, ch.name, ch.value));
                    }
                    if dom.cname.is_some() || dom.apex.is_some() {
                        step("2. DNS routing target:");
                        if let Some(cname) = &dom.cname {
                            info(&format!(
                                "{:<8}  {:<36}  {}",
                                "CNAME", cname.name, cname.target
                            ));
                        }
                        if let Some(apex) = &dom.apex {
                            info(&format!("{:<8}  {:<36}  {}", "A", apex.name, apex.ip));
                        }
                    } else if !dom.verification.is_empty() {
                        step("dns configuration records");
                        for v in &dom.verification {
                            let reason = v.reason.as_deref().unwrap_or("-");
                            info(&format!(
                                "{:<8}  {:<32}  {}  ({})",
                                v.r#type.to_uppercase(),
                                v.domain,
                                v.value,
                                reason
                            ));
                        }
                    }
                    step("next steps");
                    info(
                        "1. Configure the DNS records above at your domain registrar/DNS provider",
                    );
                    info(&format!(
                        "2. Run `appz domains verify {}` once configured",
                        dom.domain
                    ));
                    outro("domain added");
                }
            }
            Err(e) => error(&e),
        },
        DomainsCmd::List => match deploy::list_domains(&canonical, args.target) {
            Ok(domains) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&domains).unwrap());
                } else {
                    intro("appz domains");
                    if domains.is_empty() {
                        info("no custom domains configured for this project");
                        info("run `appz domains add <domain>` to connect a custom domain");
                    } else {
                        for d in &domains {
                            let ssl = d.ssl_status.as_deref().unwrap_or("-");
                            let verified_str = if d.verified { "verified" } else { "pending" };
                            info(&format!(
                                "{:<30}  status:{:<12}  ssl:{:<10}  {}",
                                d.domain,
                                d.display_status(),
                                ssl,
                                verified_str
                            ));
                        }
                    }
                    outro("done");
                }
            }
            Err(e) => error(&e),
        },
        DomainsCmd::Verify { domain } => {
            match deploy::verify_domain(&canonical, args.target, &domain) {
                Ok(res) => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&res).unwrap());
                    } else {
                        intro("appz domains verify");
                        info(&format!("domain:      {}", res.domain));
                        info(&format!("status:      {}", res.display_status()));
                        if let Some(ssl) = &res.ssl_status {
                            info(&format!("ssl:         {ssl}"));
                        }
                        if res.verified {
                            success(&format!("{} is verified and active!", res.domain));
                        } else {
                            warning("verification still pending");
                            if let Some(ch) = &res.challenge {
                                step("required ownership challenge");
                                info(&format!("{:<8}  {:<36}  {}", ch.r#type, ch.name, ch.value));
                            }
                            if let Some(cname) = &res.cname {
                                step("routing target");
                                info(&format!(
                                    "{:<8}  {:<36}  {}",
                                    "CNAME", cname.name, cname.target
                                ));
                            }
                            if let Some(apex) = &res.apex {
                                step("apex target");
                                info(&format!("{:<8}  {:<36}  {}", "A", apex.name, apex.ip));
                            }
                            if !res.verification.is_empty() {
                                step("required dns records");
                                for v in &res.verification {
                                    info(&format!(
                                        "{:<8}  {:<32}  {}",
                                        v.r#type.to_uppercase(),
                                        v.domain,
                                        v.value
                                    ));
                                }
                            }
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
                        info("no projects found in active organization");
                        info("run `appz projects add <name>` to create a project");
                    } else {
                        for p in &projects {
                            let fw = p.framework.as_deref().unwrap_or("-");
                            info(&format!(
                                "{:<30}  id:{:<24}  framework:{:<12}",
                                p.name, p.id, fw
                            ));
                        }
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
                    info(&format!("id:   {}", p.id));
                    info(&format!("slug: {}", p.slug));
                    outro("done");
                }
            }
            Err(e) => error(&e),
        },
        ProjectsCmd::Remove { name } => {
            match deploy::remove_project(&canonical, args.target, &name) {
                Ok(_) => {
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({ "status": "removed", "project": name })
                        );
                    } else {
                        intro("appz projects remove");
                        success(&format!("removed project {name}"));
                        outro("done");
                    }
                }
                Err(e) => error(&e),
            }
        }
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
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "start", verify_url])
                    .spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("open").arg(verify_url).spawn();
            }
            #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
            {
                let _ = std::process::Command::new("xdg-open")
                    .arg(verify_url)
                    .spawn();
            }
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
                        "successfully logged in as {}{}",
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
    match token {
        None => {
            if json {
                println!("{}", serde_json::json!({ "authenticated": false }));
            } else {
                info("not logged in. Run `appz login` or set APPZ_TOKEN");
            }
        }
        Some(t) => {
            let stored = appz_deploy::load_stored_auth();
            let api_base = std::env::var("APPZ_API_URL")
                .unwrap_or_else(|_| "https://api.appz.dev".to_string());
            let session_url = format!("{api_base}/api/auth/get-session");
            let session_res = ureq::get(&session_url)
                .set("Authorization", &format!("Bearer {t}"))
                .call();

            let (email, org) = if let Ok(resp) = session_res {
                if let Ok(v) = resp.into_json::<serde_json::Value>() {
                    let email = v
                        .get("user")
                        .and_then(|u| u.get("email"))
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    let org = v
                        .get("session")
                        .and_then(|s| s.get("activeOrganizationId"))
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    (email, org)
                } else {
                    (
                        stored.as_ref().and_then(|s| s.email.clone()),
                        stored.as_ref().and_then(|s| s.organization_id.clone()),
                    )
                }
            } else {
                (
                    stored.as_ref().and_then(|s| s.email.clone()),
                    stored.as_ref().and_then(|s| s.organization_id.clone()),
                )
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
                intro("appz whoami");
                if let Some(e) = email {
                    info(&format!("user:         {e}"));
                } else {
                    info("user:         authenticated via token");
                }
                if let Some(o) = org {
                    info(&format!("organization: {o}"));
                }
                outro("done");
            }
        }
    }
}

fn run_logout(json: bool) {
    match appz_deploy::clear_stored_auth() {
        Ok(()) => {
            if json {
                println!("{}", serde_json::json!({ "status": "logged_out" }));
            } else {
                intro("appz logout");
                success("logged out successfully");
                outro("done");
            }
        }
        Err(e) => error(&format!("failed to clear session token: {e}")),
    }
}
