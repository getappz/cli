//! Branch finish — Superpowers: finishing-a-development-branch

use crate::commands::git::{BranchCommands, BranchFinishAction};
use crate::session::AppzSession;
use git2::Repository;
use starbase::AppResult;

pub async fn run(session: AppzSession, command: BranchCommands) -> AppResult {
    match command {
        BranchCommands::Finish { action, yes } => finish(session, action, yes).await,
    }
}

async fn finish(
    session: AppzSession,
    action: BranchFinishAction,
    yes: bool,
) -> AppResult {
    let cwd = session.working_dir.clone();

    let repo = Repository::discover(&cwd)
        .map_err(|e| miette::miette!("Not a git repository: {}", e))?;

    let repo_root = repo
        .commondir()
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| cwd.clone());

    let head = repo
        .head()
        .map_err(|e| miette::miette!("Could not determine current branch: {}", e))?;
    let branch = head
        .shorthand()
        .ok_or_else(|| miette::miette!("Could not get branch name"))?
        .to_string();

    let base = ["main", "master"]
        .iter()
        .find(|b| repo.find_branch(b, git2::BranchType::Local).is_ok())
        .map(|s| (*s).to_string())
        .unwrap_or_else(|| "main".to_string());

    match action {
        BranchFinishAction::Merge => {
            let _ = ui::status::info(&format!("Merging {} into {}", branch, base));
            run_git(&repo_root, &["checkout", &base])?;
            run_git(&repo_root, &["pull"])?;
            run_git(&repo_root, &["merge", &branch])?;
            run_git(&repo_root, &["branch", "-d", &branch])?;
            cleanup_worktree_if_needed(&repo_root, &branch).await?;
            ui::status::success("Merge complete");
        }
        BranchFinishAction::Pr => {
            let _ = ui::status::info(&format!("Pushing {} and creating PR", branch));
            run_git(&repo_root, &["push", "-u", "origin", &branch])?;
            if which::which("gh").is_ok() {
                let mut cmd = std::process::Command::new("gh");
                cmd.args(["pr", "create"])
                    .current_dir(&repo_root)
                    .status()
                    .map_err(|e| miette::miette!("gh pr create failed: {}", e))?;
                ui::status::success("PR created");
            } else {
                ui::status::info("Push complete. Create PR manually (gh CLI not found).");
            }
        }
        BranchFinishAction::Keep => {
            let _ = ui::status::success(&format!("Keeping branch {}. Worktree preserved.", branch));
        }
        BranchFinishAction::Discard => {
            if !yes {
                return Err(miette::miette!(
                    "Discard requires confirmation. Use --yes to confirm."
                ));
            }
            let _ = ui::status::info(&format!("Discarding branch {}", branch));
            run_git(&repo_root, &["checkout", &base])?;
            run_git(&repo_root, &["branch", "-D", &branch])?;
            cleanup_worktree_if_needed(&repo_root, &branch).await?;
            ui::status::success("Branch discarded");
        }
    }

    Ok(None)
}

fn run_git(cwd: &std::path::Path, args: &[&str]) -> Result<(), miette::Report> {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|e| miette::miette!("git {} failed: {}", args.join(" "), e))?;
    if !status.success() {
        return Err(miette::miette!("git {} failed", args.join(" ")));
    }
    Ok(())
}

async fn cleanup_worktree_if_needed(cwd: &std::path::Path, branch: &str) -> AppResult {
    let output = std::process::Command::new("git")
        .args(["worktree", "list"])
        .current_dir(cwd)
        .output()
        .map_err(|e| miette::miette!("git worktree list failed: {}", e))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains(&format!("[{}]", branch)) {
            let path = line.split_whitespace().next().unwrap_or("");
            if !path.is_empty() {
                let status = std::process::Command::new("git")
                    .args(["worktree", "remove", path])
                    .current_dir(cwd)
                    .status()
                    .map_err(|e| miette::miette!("git worktree remove failed: {}", e))?;
                if status.success() {
                    let _ = ui::status::info(&format!("Removed worktree at {}", path));
                }
            }
            break;
        }
    }

    Ok(None)
}
