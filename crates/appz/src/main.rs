use clap::{Args, Parser, Subcommand};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use appz_core::{detect_toolchains, generate_claude_md, run_doctor};

mod agents;
mod deploy;
mod dev_install;
mod init_source;
mod mcp;
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
    /// Diagnose project stack, config, and suggestions
    Doctor(DoctorArgs),
    /// Build the checkout and install it over the running `appz` binary
    DevInstall(DevInstallArgs),
    /// Run a stdio MCP server exposing appz to AI agents
    Mcp,
    /// Deploy to a hosting platform (drives that platform's own CLI)
    Deploy(DeployArgs),
    /// Discover and install agent skills relevant to this project
    Skills(SkillsArgs),
    /// Manage AI agents — list, doctor, install, uninstall, update, refresh
    Agents(AgentArgs),
    /// Self-update to the latest (or a specific) release
    Update(UpdateArgs),
}

#[derive(Args)]
struct SkillsArgs {
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
    /// Install a skill for a specific agent
    Install {
        /// Agent to install for (e.g. cursor, claude-code)
        #[arg(long)]
        agent: Option<String>,
        /// Name of the skill to install
        skill: String,
        /// Install without prompting
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Uninstall a skill
    Uninstall {
        /// Name of the skill to remove
        skill: String,
    },
    /// Update installed skills to latest versions
    Update,
    /// Re-detect agents and list installed skills
    Refresh,
}

#[derive(Args)]
struct DeployArgs {
    #[arg(default_value = ".")]
    dir: PathBuf,
    /// Provider slug (e.g. vercel, netlify). Uses appz.jsonc's deploy.default if omitted.
    #[arg(long)]
    target: Option<String>,
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

    with_spinner("writing project state...", "state written", || {
        appz_core::write_state(root, &toolchains);
    });

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
            do_pm_install(&canonical, &toolchains);
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
    let toolchains = {
        if !args.skip_install {
            do_full_install(&canonical)
        } else {
            do_detect_and_write(&canonical)
        }
    };

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
        AppzCmd::Doctor(a) => run_doctor_cmd(a, json),
        AppzCmd::DevInstall(a) => dev_install::run(!a.debug, a.dry_run),
        AppzCmd::Mcp => {
            if let Err(e) = mcp::serve() {
                error(&e);
            }
        }
        AppzCmd::Deploy(a) => run_deploy(a, json),
        AppzCmd::Skills(a) => run_skills(a, json),
        AppzCmd::Agents(a) => run_agents(a, json),
        AppzCmd::Update(a) => update::run(a.version, a.check, a.quiet),
    }
}

fn run_agents(args: AgentArgs, json: bool) {
    let cmd = agents::AgentCommand {
        kind: match args.action {
            AgentAction::List => agents::AgentSubcommand::List,
            AgentAction::Doctor => agents::AgentSubcommand::Doctor,
            AgentAction::Install { .. } => agents::AgentSubcommand::Install,
            AgentAction::Uninstall { .. } => agents::AgentSubcommand::Uninstall,
            AgentAction::Update => agents::AgentSubcommand::Update,
            AgentAction::Refresh => agents::AgentSubcommand::Refresh,
        },
        dir: args.dir,
        agent: match &args.action {
            AgentAction::Install { agent, .. } => agent.clone(),
            _ => None,
        },
        skill_name: match &args.action {
            AgentAction::Install { skill, .. } => Some(skill.clone()),
            AgentAction::Uninstall { skill } => Some(skill.clone()),
            _ => None,
        },
        yes: match &args.action {
            AgentAction::Install { yes, .. } => *yes,
            _ => false,
        },
        json,
    };
    agents::run(&cmd);
}

fn run_skills(args: SkillsArgs, json: bool) {
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

fn run_deploy(args: DeployArgs, json: bool) {
    let canonical = resolve_root(&args.dir);
    match deploy::run(
        &canonical,
        deploy::DeployRequest {
            target: args.target,
            prod: args.prod,
            env: args.env,
            build_env: args.build_env,
            dry_run: args.dry_run,
            json,
        },
    ) {
        Ok(output) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                intro("appz deploy");
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
