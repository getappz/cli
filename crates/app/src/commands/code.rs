//! Pack codebase for AI context via code-mix (Repomix + pre-filters).

use crate::session::AppzSession;
use clap::{Args, Subcommand};
use std::path::PathBuf;
use ui::layout;
use ui::status;

/// Default ignore patterns optimized for LLM context (build artifacts, deps, caches).
const DEFAULT_LLM_IGNORE: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    "target",
    "out",
    ".next",
    ".nuxt",
    ".turbo",
    "coverage",
    ".cache",
    "__pycache__",
    "*.pyc",
    "venv",
    ".venv",
    ".env",
    ".env.*",
    "*.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
];

/// List cached pack outputs.
#[derive(Subcommand, Debug, Clone)]
pub enum PackSubcommand {
    /// List cached pack outputs
    #[command(visible_alias = "list")]
    Ls {
        /// Show content hash and size
        #[arg(long)]
        verbose: bool,
    },
    /// Remove cached pack outputs
    #[command(visible_alias = "remove")]
    Rm {
        /// Content hash(es) to remove (from pack ls)
        hashes: Vec<String>,
        /// Remove all cached entries
        #[arg(long)]
        all: bool,
    },
}

#[derive(Args, Debug, Clone)]
pub struct PackRunOpts {
    /// Working directory (default: current)
    #[arg(long)]
    pub workdir: Option<PathBuf>,
    /// Output file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Output style: markdown, xml, json, plain (default: markdown for LLM)
    #[arg(long, default_value = "markdown")]
    pub style: String,
    /// Include only files matching glob patterns
    #[arg(long)]
    pub include: Vec<String>,
    /// Exclude files matching glob patterns (baseline: build artifacts, deps, caches)
    #[arg(short = 'i', long)]
    pub ignore: Vec<String>,
    /// Extract essential structure with Tree-sitter (compress)
    #[arg(long)]
    pub compress: bool,
    /// Remove comments from code (default: true for LLM)
    #[arg(long, default_value_t = true)]
    pub remove_comments: bool,
    /// Remove empty lines (default: true for LLM)
    #[arg(long, default_value_t = true)]
    pub remove_empty_lines: bool,
    /// Split output into chunks (e.g. 500kb, 1mb)
    #[arg(long)]
    pub split_output: Option<String>,
    /// Instruction file path to prepend to output
    #[arg(long)]
    pub instruction: Option<PathBuf>,
    /// Custom header text
    #[arg(long)]
    pub header: Option<String>,
    /// Copy output to clipboard
    #[arg(long)]
    pub copy: bool,
    /// Write to stdout instead of file
    #[arg(long)]
    pub stdout: bool,
    /// Include only files containing text (use multiple times, requires rg)
    #[arg(short)]
    pub strings: Vec<String>,
    /// Exclude files containing text
    #[arg(long)]
    pub exclude_strings: Vec<String>,
    /// Include only git staged files
    #[arg(long)]
    pub staged: bool,
    /// Include only modified/untracked files
    #[arg(long)]
    pub dirty: bool,
    /// Include only files changed from base branch
    #[arg(long)]
    pub diff: bool,
    /// Base branch for --diff
    #[arg(long, default_value = "main")]
    pub diff_base: String,
    /// Load saved bundle (skip TUI, use saved file list)
    #[arg(long)]
    pub bundle: Option<String>,
    /// Use built-in prompt template (review, tests, refactor, etc.)
    #[arg(long)]
    pub template: Option<String>,
    /// List available built-in templates
    #[arg(long)]
    pub list_templates: bool,
    /// Select monorepo workspace (e.g. @org/pkg)
    #[arg(long, short = 'W')]
    pub workspace: Option<String>,
    /// List detected workspaces (monorepo only)
    #[arg(long)]
    pub workspaces: bool,
    /// Disable SHA-256 output cache
    #[arg(long)]
    pub no_cache: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CodeCommands {
    /// Pack codebase for AI context (Repomix + pre-filters)
    Pack {
        #[command(subcommand)]
        subcommand: Option<PackSubcommand>,
        #[command(flatten)]
        run_opts: PackRunOpts,
    },
}

pub async fn run(session: AppzSession, command: CodeCommands) -> starbase::AppResult {
    match command {
        CodeCommands::Pack { subcommand, run_opts } => {
            if let Some(PackSubcommand::Ls { verbose }) = subcommand {
                let entries = code_mix::list_cached(verbose)
                    .map_err(|e| miette::miette!("{}", e.0))?;
                let _ = layout::section_title("Pack cache");
                let _ = layout::blank_line();
                if entries.is_empty() {
                    let _ = layout::indented("No cached pack outputs.", 1);
                    return Ok(None);
                }
                for e in &entries {
                    let workdir = e.workdir.as_deref().map(|p| {
                        std::path::Path::new(p)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(p)
                    }).unwrap_or("—");
                    let style = e.style.as_deref().unwrap_or("—");
                    let file_count = e.file_count.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
                    let ws = e.workspace.as_deref().unwrap_or("—");
                    let size_str = e
                        .size_bytes
                        .map(|s| format!("  {} bytes", s))
                        .unwrap_or_default();
                    let hash_short = if e.content_hash.len() >= 12 {
                        &e.content_hash[..12]
                    } else {
                        &e.content_hash
                    };
                    let _ = layout::indented(
                        &format!("{}  {}  {} files  ws:{}  {}{}", workdir, style, file_count, ws, hash_short, size_str),
                        1,
                    );
                }
                let _ = layout::blank_line();
                let _ = layout::indented("Use: appz code pack rm <content_hash>", 1);
                return Ok(None);
            }
            if let Some(PackSubcommand::Rm { hashes, all }) = subcommand {
                if !all && hashes.is_empty() {
                    return Err(miette::miette!(
                        "Specify content hash(es) or use --all to remove all cached entries"
                    ));
                }
                code_mix::remove_cached(&hashes, all).map_err(|e| miette::miette!("{}", e.0))?;
                let _ = status::success("Cache entries removed.");
                return Ok(None);
            }

            let workdir = run_opts
                .workdir
                .clone()
                .unwrap_or_else(|| session.working_dir.clone());
            let workdir = workdir
                .canonicalize()
                .map_err(|e| miette::miette!("Invalid workdir: {}", e))?;

            let list_templates = run_opts.list_templates;
            let workspaces = run_opts.workspaces;
            if workspaces {
                let Some(workspaces_list) = code_mix::workspace::detect_monorepo(&workdir).await
                    .map_err(|e| miette::miette!("{}", e.0))?
                else {
                    let _ = layout::indented("No monorepo detected.", 1);
                    let _ = layout::indented(
                        "Supported: pnpm-workspace.yaml, package.json workspaces",
                        1,
                    );
                    return Ok(None);
                };
                let _ = layout::section_title("Workspaces");
                let _ = layout::blank_line();
                for ws in &workspaces_list {
                    let _ = layout::indented(
                        &format!("{} ({})", ws.name, ws.relative_path),
                        1,
                    );
                }
                let _ = layout::blank_line();
                let _ = layout::indented(
                    "Use with: appz code pack --workspace <name>",
                    1,
                );
                return Ok(None);
            }
            if list_templates {
                let _ = layout::section_title("Built-in templates");
                let _ = layout::blank_line();
                for name in code_mix::templates::list() {
                    let _ = layout::indented(name, 1);
                }
                let _ = layout::blank_line();
                let _ = layout::indented("Use with: appz code pack --template <name>", 1);
                return Ok(None);
            }

            let _ = status::info(&format!("Packing codebase at {}", workdir.display()));
            let _ = layout::blank_line();

            let ignore: Vec<String> = DEFAULT_LLM_IGNORE
                .iter()
                .map(|s| (*s).to_string())
                .chain(run_opts.ignore.iter().cloned())
                .collect();

            let options = code_mix::PackOptions {
                workdir: workdir.clone(),
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

            code_mix::pack(&workdir, options)
                .await
                .map_err(|e| miette::miette!("Pack failed: {}", e.0))?;

            let _ = layout::blank_line();
            let _ = status::success("Pack complete.");
        }
    }

    Ok(None)
}
