//! Worktree commands — Superpowers: using-git-worktrees

use crate::commands::git::WorktreeCommands;
use crate::session::AppzSession;
use git2::{WorktreeAddOptions, Repository};
use starbase::AppResult;
use std::path::PathBuf;

pub async fn run(session: AppzSession, command: WorktreeCommands) -> AppResult {
    match command {
        WorktreeCommands::Create {
            branch,
            dir,
            setup,
            verify,
        } => create(session, branch, dir, setup, verify).await,
        WorktreeCommands::Remove { path } => remove(session, path).await,
        WorktreeCommands::List => list(session).await,
    }
}

async fn create(
    session: AppzSession,
    branch: String,
    dir: Option<PathBuf>,
    setup: bool,
    verify: bool,
) -> AppResult {
    let cwd = session.working_dir.clone();

    // 1. Resolve worktree directory
    let worktree_dir = match &dir {
        Some(d) => {
            if d.is_absolute() {
                d.clone()
            } else {
                cwd.join(d)
            }
        }
        None => {
            // Superpowers priority: .worktrees, worktrees
            let dot = cwd.join(".worktrees").join(&branch);
            let plain = cwd.join("worktrees").join(&branch);
            if cwd.join(".worktrees").exists() {
                dot
            } else if cwd.join("worktrees").exists() {
                plain
            } else {
                // Default to .worktrees
                dot
            }
        }
    };

    let parent = worktree_dir
        .parent()
        .ok_or_else(|| miette::miette!("Invalid worktree path"))?;

    let repo = Repository::discover(&cwd)
        .map_err(|e| miette::miette!("Not a git repository: {}", e))?;

    // 2. Verify parent is gitignored (for project-local .worktrees or worktrees)
    if parent.starts_with(&cwd) {
        let check_path = parent.strip_prefix(&cwd).unwrap_or(parent);
        let ignored = repo
            .is_path_ignored(check_path)
            .unwrap_or(false);

        if !ignored {
            let _ = ui::status::warning(&format!(
                "Directory {:?} is not gitignored. Add it to .gitignore to prevent committing worktree contents.",
                check_path
            ));
        }
    }

    // 3. Create branch from HEAD and add worktree
    let head = repo
        .head()
        .map_err(|e| miette::miette!("No HEAD: {}", e))?
        .peel_to_commit()
        .map_err(|e| miette::miette!("HEAD is not a commit: {}", e))?;

    repo.branch(&branch, &head, false)
        .map_err(|e| miette::miette!("Failed to create branch {}: {}", branch, e))?;

    let branch_ref = repo
        .find_reference(&format!("refs/heads/{}", branch))
        .map_err(|e| miette::miette!("Failed to find branch ref: {}", e))?;

    let mut opts = WorktreeAddOptions::new();
    opts.reference(Some(&branch_ref));

    repo.worktree(&branch, &worktree_dir, Some(&opts))
        .map_err(|e| miette::miette!("git worktree add failed: {}", e))?;

    let _ = ui::status::success(&format!(
        "Worktree created at {}",
        worktree_dir.display()
    ));

    if setup {
        run_project_setup(&worktree_dir).await?;
    }

    if verify {
        crate::verify::run_tests(&worktree_dir).await?;
    }

    Ok(None)
}

async fn run_project_setup(workdir: &PathBuf) -> AppResult {
    if workdir.join("package.json").exists() {
        ui::status::info("Running npm install...");
        let status = tokio::process::Command::new("npm")
            .arg("install")
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette::miette!("npm install failed: {}", e))?;
        if !status.success() {
            return Err(miette::miette!("npm install failed"));
        }
    }
    if workdir.join("Cargo.toml").exists() {
        ui::status::info("Running cargo build...");
        let status = tokio::process::Command::new("cargo")
            .arg("build")
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette::miette!("cargo build failed: {}", e))?;
        if !status.success() {
            return Err(miette::miette!("cargo build failed"));
        }
    }
    if workdir.join("requirements.txt").exists() {
        ui::status::info("Running pip install...");
        let status = tokio::process::Command::new("pip")
            .args(["install", "-r", "requirements.txt"])
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette::miette!("pip install failed: {}", e))?;
        if !status.success() {
            return Err(miette::miette!("pip install failed"));
        }
    }
    if workdir.join("go.mod").exists() {
        ui::status::info("Running go mod download...");
        let status = tokio::process::Command::new("go")
            .args(["mod", "download"])
            .current_dir(workdir)
            .status()
            .await
            .map_err(|e| miette::miette!("go mod download failed: {}", e))?;
        if !status.success() {
            return Err(miette::miette!("go mod download failed"));
        }
    }
    Ok(None)
}

async fn remove(session: AppzSession, path: Option<PathBuf>) -> AppResult {
    let cwd = session.working_dir.clone();
    let repo = Repository::discover(&cwd)
        .map_err(|e| miette::miette!("Not a git repository: {}", e))?;

    let target = match path {
        Some(p) => {
            if p.is_absolute() {
                p
            } else {
                cwd.join(p)
            }
        }
        None => {
            let head = repo
                .head()
                .map_err(|e| miette::miette!("Could not determine current branch: {}", e))?;
            let branch = head
                .shorthand()
                .ok_or_else(|| miette::miette!("Could not get branch name"))?
                .to_string();

            let names = repo.worktrees().map_err(|e| miette::miette!("git worktree list failed: {}", e))?;
            let canonical_target = std::fs::canonicalize(&cwd)
                .unwrap_or(cwd.clone());

            for name in names.iter().flatten() {
                if let Ok(wt) = repo.find_worktree(name) {
                    if wt.name() == Some(branch.as_str()) {
                        return prune_worktree(&wt).await;
                    }
                    if let Ok(wt_path) = wt.path().canonicalize() {
                        if wt_path == canonical_target {
                            return prune_worktree(&wt).await;
                        }
                    }
                }
            }

            return Err(miette::miette!(
                "No worktree found for branch {:?}. Specify path explicitly.",
                branch
            ));
        }
    };

    let names = repo.worktrees().map_err(|e| miette::miette!("git worktree list failed: {}", e))?;
    let canonical_target = target.canonicalize()
        .or_else(|_| target.to_path_buf().canonicalize())
        .unwrap_or(target.clone());

    for name in names.iter().flatten() {
        if let Ok(wt) = repo.find_worktree(name) {
            if let Ok(wt_path) = wt.path().canonicalize() {
                if wt_path == canonical_target {
                    return prune_worktree(&wt).await;
                }
            }
        }
    }

    Err(miette::miette!("No worktree found at {}", target.display()))
}


async fn prune_worktree(wt: &git2::Worktree) -> AppResult {
    let path = wt.path().to_path_buf();
    wt.prune(None)
        .map_err(|e| miette::miette!("git worktree remove failed: {}", e))?;
    let _ = ui::status::success(&format!("Removed worktree at {}", path.display()));
    Ok(None)
}

async fn list(session: AppzSession) -> AppResult {
    let repo = Repository::discover(&session.working_dir)
        .map_err(|e| miette::miette!("Not a git repository: {}", e))?;

    if let Some(workdir) = repo.workdir() {
        println!("{}  [{}]", workdir.display(), "main");
    }

    let names = repo.worktrees().map_err(|e| miette::miette!("git worktree list failed: {}", e))?;
    for name in names.iter().flatten() {
        if let Ok(wt) = repo.find_worktree(name) {
            let branch = wt.name().unwrap_or(name);
            println!("{}  [{}]", wt.path().display(), branch);
        }
    }
    Ok(None)
}
