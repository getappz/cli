//! appz pack: unified AI context builder (config-driven + imperative).

use crate::session::AppzSession;
use clap::{Args, Subcommand};
use std::path::PathBuf;
use ui::layout;
use ui::status;

/// Default ignore patterns for imperative pack.
const DEFAULT_LLM_IGNORE: &[&str] = &[
    "node_modules", ".git", ".appz", "dist", "build", "target", "out",
    ".next", ".nuxt", ".turbo", "coverage", ".cache", "__pycache__", "*.pyc",
    "venv", ".venv", ".env", ".env.*", "*.lock", "package-lock.json",
    "yarn.lock", "pnpm-lock.yaml",
];

#[derive(Subcommand, Debug, Clone)]
pub enum PackSubcommand {
    /// List cached pack outputs (imperative mode)
    #[command(visible_alias = "list")]
    Ls {
        #[arg(long)]
        verbose: bool,
    },
    /// Remove cached pack outputs
    #[command(visible_alias = "remove")]
    Rm {
        hashes: Vec<String>,
        #[arg(long)]
        all: bool,
    },
    /// Initialize pack config (pack.config.json, repomix.config.json)
    Init,
    /// Add a GitHub repo to pack config
    Add {
        /// Repository: owner/repo or https://github.com/owner/repo
        repo: String,
    },
    /// Remove pack scripts and rules from project
    Uninstall,
}

#[derive(Args, Debug, Clone)]
pub struct PackRunOpts {
    #[arg(long)]
    pub workdir: Option<PathBuf>,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, default_value = "markdown")]
    pub style: String,
    #[arg(long)]
    pub include: Vec<String>,
    #[arg(short = 'i', long)]
    pub ignore: Vec<String>,
    #[arg(long)]
    pub compress: bool,
    #[arg(long, default_value_t = true)]
    pub remove_comments: bool,
    #[arg(long, default_value_t = true)]
    pub remove_empty_lines: bool,
    #[arg(long)]
    pub split_output: Option<String>,
    #[arg(long)]
    pub instruction: Option<PathBuf>,
    #[arg(long)]
    pub header: Option<String>,
    #[arg(long)]
    pub copy: bool,
    #[arg(long)]
    pub stdout: bool,
    #[arg(short)]
    pub strings: Vec<String>,
    #[arg(long)]
    pub exclude_strings: Vec<String>,
    #[arg(long)]
    pub staged: bool,
    #[arg(long)]
    pub dirty: bool,
    #[arg(long)]
    pub diff: bool,
    #[arg(long, default_value = "main")]
    pub diff_base: String,
    #[arg(long)]
    pub bundle: Option<String>,
    #[arg(long)]
    pub template: Option<String>,
    #[arg(long, short = 'W')]
    pub workspace: Option<String>,
    #[arg(long)]
    pub workspaces: bool,
    #[arg(long)]
    pub list_templates: bool,
    #[arg(long)]
    pub no_cache: bool,
    /// Config file path (for config-driven mode)
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    /// Override output directory (config-driven mode)
    #[arg(long)]
    pub output_path: Option<PathBuf>,
    /// Suppress non-error output (config-driven mode)
    #[arg(long)]
    pub silent: bool,
}

pub async fn run(session: AppzSession, subcommand: Option<PackSubcommand>, run_opts: PackRunOpts) -> starbase::AppResult {
    let cwd = session
        .working_dir
        .canonicalize()
        .map_err(|e| miette::miette!("Invalid workdir: {}", e))?;

    // Subcommands
    if let Some(cmd) = subcommand {
        match cmd {
            PackSubcommand::Ls { verbose } => {
                let entries = code_mix::list_cached(verbose).map_err(|e| miette::miette!("{}", e.0))?;
                let _ = layout::section_title("Pack cache");
                let _ = layout::blank_line();
                if entries.is_empty() {
                    let _ = layout::indented("No cached pack outputs. Run `appz pack` first.", 1);
                    return Ok(None);
                }
                for e in &entries {
                    let workdir = e.workdir.as_deref().unwrap_or("—");
                    let style = e.style.as_deref().unwrap_or("—");
                    let file_count = e.file_count.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
                    let ws = e.workspace.as_deref().unwrap_or("—");
                    let hash_short = e.content_hash.get(..12).unwrap_or(&e.content_hash);
                    let _ = layout::indented(
                        &format!("{}  {}  {} files  ws:{}  {}", workdir, style, file_count, ws, hash_short),
                        1,
                    );
                }
                let _ = layout::blank_line();
                let _ = layout::indented("Use: appz pack rm <content_hash>", 1);
                return Ok(None);
            }
            PackSubcommand::Rm { hashes, all } => {
                if !all && hashes.is_empty() {
                    return Err(miette::miette!("Specify content hash(es) or use --all"));
                }
                code_mix::remove_cached(&hashes, all).map_err(|e| miette::miette!("{}", e.0))?;
                let _ = status::success("Cache entries removed.");
                return Ok(None);
            }
            PackSubcommand::Init => {
                hypermix::init(&cwd).map_err(|e| miette::miette!("{}", e))?;
                let _ = status::success("Pack initialized. Edit pack.config.json and run `appz pack`.");
                return Ok(None);
            }
            PackSubcommand::Add { repo } => {
                hypermix::add_repo(&cwd, run_opts.config.as_deref(), &repo)
                    .map_err(|e| miette::miette!("{}", e))?;
                let _ = status::success(&format!("Added {} to config.", repo));
                return Ok(None);
            }
            PackSubcommand::Uninstall => {
                hypermix::uninstall(&cwd).map_err(|e| miette::miette!("{}", e))?;
                let _ = status::success("Pack uninstalled.");
                return Ok(None);
            }
        }
    }

    // Run: config-driven or imperative
    let config_path = run_opts.config.as_deref();
    let has_config = match config_path {
        Some(p) => p.exists(),
        None => ["pack.config.json", "pack.config.jsonc", "hypermix.config.json", "hypermix.config.jsonc"]
            .iter()
            .any(|n| cwd.join(n).exists()),
    };

    if has_config {
        hypermix::run_config(&cwd, config_path)
            .await
            .map_err(|e| miette::miette!("{}", e))?;
        let _ = status::success("Pack complete (config-driven).");
    } else {
        // Imperative mode: use code_mix
        let list_templates = run_opts.list_templates;
        let workspaces = run_opts.workspaces;
        if workspaces {
            let Some(ws_list) = code_mix::workspace::detect_monorepo(&cwd).await.map_err(|e| miette::miette!("{}", e.0))?
            else {
                let _ = layout::indented("No monorepo detected.", 1);
                return Ok(None);
            };
            let _ = layout::section_title("Workspaces");
            for ws in &ws_list {
                let _ = layout::indented(&format!("{} ({})", ws.name, ws.relative_path), 1);
            }
            return Ok(None);
        }
        if list_templates {
            let _ = layout::section_title("Built-in templates");
            for name in code_mix::templates::list() {
                let _ = layout::indented(name, 1);
            }
            return Ok(None);
        }

        let workdir = run_opts.workdir.unwrap_or(cwd.clone());
        let workdir = workdir.canonicalize().map_err(|e| miette::miette!("Invalid workdir: {}", e))?;

        let ignore: Vec<String> = DEFAULT_LLM_IGNORE
            .iter()
            .map(|s| (*s).to_string())
            .chain(run_opts.ignore)
            .collect();

        let options = code_mix::PackOptions {
            workdir,
            output: run_opts.output,
            style: Some(run_opts.style),
            include: run_opts.include,
            ignore,
            compress: run_opts.compress,
            remove_comments: run_opts.remove_comments,
            remove_empty_lines: run_opts.remove_empty_lines,
            split_output: run_opts.split_output,
            instruction: run_opts.instruction,
            header: run_opts.header,
            copy: run_opts.copy,
            stdout: run_opts.stdout,
            strings: run_opts.strings,
            exclude_strings: run_opts.exclude_strings,
            staged: run_opts.staged,
            dirty: run_opts.dirty,
            diff: run_opts.diff,
            diff_base: Some(run_opts.diff_base),
            bundle: run_opts.bundle,
            template: run_opts.template,
            workspace: run_opts.workspace,
            no_cache: run_opts.no_cache,
            ..Default::default()
        };

        let workdir_path = options.workdir.clone();
        code_mix::pack(&workdir_path, options)
            .await
            .map_err(|e| miette::miette!("Pack failed: {}", e.0))?;
        let _ = status::success("Pack complete.");
    }

    Ok(None)
}
