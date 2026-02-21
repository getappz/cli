//! Git tooling from Superpowers workflow (worktrees, branch finish, review prepare).
//!
//! Subcommands: worktree | check-ignore | merge-base | branch | review
//! Uses git2 crate for all git operations.

use crate::session::AppzSession;
use clap::Subcommand;
use git2::Repository;
use starbase::AppResult;
use std::path::PathBuf;

mod branch;
mod merge_base;
mod review;
mod worktree;

#[derive(Subcommand, Debug, Clone)]
pub enum GitCommands {
    /// Manage worktrees (create, remove, list) — Superpowers: using-git-worktrees
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },
    /// Verify a path is gitignored — Superpowers: worktree safety check
    #[command(name = "check-ignore")]
    CheckIgnore {
        /// Path to check (e.g. .worktrees or worktrees)
        path: PathBuf,
    },
    /// Find merge base with main/master — Superpowers: finishing-a-development-branch
    #[command(name = "merge-base")]
    MergeBase {
        /// Branch to find merge base with (default: try main then master)
        #[arg(default_value = "main")]
        branch: String,
    },
    /// Branch lifecycle — Superpowers: finishing-a-development-branch
    Branch {
        #[command(subcommand)]
        command: BranchCommands,
    },
    /// Prepare code review context — Superpowers: requesting-code-review
    Review {
        #[command(subcommand)]
        command: ReviewCommands,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum WorktreeCommands {
    /// Create worktree with branch, run setup, verify baseline
    Create {
        /// Branch name (creates new branch)
        branch: String,
        /// Worktree directory (.worktrees or worktrees, or custom path)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Run project setup (npm install, cargo build, etc.)
        #[arg(long, default_value = "true")]
        setup: bool,
        /// Run tests to verify clean baseline
        #[arg(long, default_value = "true")]
        verify: bool,
    },
    /// Remove a worktree
    Remove {
        /// Worktree path (from `git worktree list`)
        path: Option<PathBuf>,
    },
    /// List worktrees
    List,
}

#[derive(Subcommand, Debug, Clone)]
pub enum BranchCommands {
    /// Finish development branch: merge, PR, keep, or discard
    Finish {
        /// Action: merge | pr | keep | discard
        #[arg(value_enum)]
        action: BranchFinishAction,
        /// Skip confirmation for destructive actions
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum BranchFinishAction {
    /// Merge to base branch locally
    Merge,
    /// Push and create PR
    Pr,
    /// Keep branch as-is
    Keep,
    /// Discard branch and worktree
    Discard,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ReviewCommands {
    /// Output BASE_SHA, HEAD_SHA and template for code review
    Prepare {
        /// Base commit (default: HEAD~1 or origin/main)
        #[arg(long)]
        base: Option<String>,
    },
}

pub async fn run(session: AppzSession, command: GitCommands) -> AppResult {
    match command {
        GitCommands::Worktree { command } => worktree::run(session, command).await,
        GitCommands::CheckIgnore { path } => check_ignore(session, path).await,
        GitCommands::MergeBase { branch } => merge_base::run(session, branch).await,
        GitCommands::Branch { command } => branch::run(session, command).await,
        GitCommands::Review { command } => review::run(session, command).await,
    }
}

async fn check_ignore(session: AppzSession, path: PathBuf) -> AppResult {
    let repo = Repository::discover(&session.working_dir)
        .map_err(|e| miette::miette!("Not a git repository: {}", e))?;

    let path_for_ignore = if let Some(workdir) = repo.workdir() {
        path.strip_prefix(workdir).unwrap_or(&path)
    } else {
        path.as_path()
    };

    let ignored = repo
        .is_path_ignored(path_for_ignore)
        .map_err(|e| miette::miette!("git check-ignore failed: {}", e))?;

    if ignored {
        println!("✓ {} is gitignored", path.display());
    } else {
        println!("✗ {} is NOT gitignored", path.display());
        println!("  Add it to .gitignore to prevent accidentally committing worktree contents.");
        return Err(miette::miette!("Path not ignored"));
    }

    Ok(None)
}
