//! Review prepare — Superpowers: requesting-code-review

use crate::commands::git::ReviewCommands;
use crate::session::AppzSession;
use git2::Repository;
use starbase::AppResult;

pub async fn run(session: AppzSession, command: ReviewCommands) -> AppResult {
    match command {
        ReviewCommands::Prepare { base } => prepare(session, base).await,
    }
}

async fn prepare(session: AppzSession, base: Option<String>) -> AppResult {
    let repo = Repository::discover(&session.working_dir)
        .map_err(|e| miette::miette!("Not a git repository: {}", e))?;

    let head_obj = repo
        .revparse_single("HEAD")
        .map_err(|e| miette::miette!("git rev-parse HEAD failed: {}", e))?;
    let head_sha = head_obj.id().to_string();

    let base_sha = match &base {
        Some(b) => {
            repo.revparse_single(b)
                .map_err(|e| miette::miette!("Invalid base ref {}: {}", b, e))?
                .id()
                .to_string()
        }
        None => {
            if let Ok(obj) = repo.revparse_single("HEAD~1") {
                obj.id().to_string()
            } else {
                let head_oid = head_obj.id();
                let base_ref = repo
                    .find_branch("main", git2::BranchType::Local)
                    .ok()
                    .and_then(|b| b.get().target())
                    .or_else(|| {
                        repo.find_branch("master", git2::BranchType::Local)
                            .ok()
                            .and_then(|b| b.get().target())
                    })
                    .ok_or_else(|| miette::miette!("Could not find main or master branch"))?;
                repo.merge_base(head_oid, base_ref)
                    .map_err(|e| miette::miette!("Could not determine base commit: {}", e))?
                    .to_string()
            }
        }
    };

    println!("BASE_SHA={}", base_sha);
    println!("HEAD_SHA={}", head_sha);
    println!();
    println!("## Code Review Template");
    println!();
    println!("WHAT_WAS_IMPLEMENTED: [describe changes]");
    println!("PLAN_OR_REQUIREMENTS: [reference plan/requirements]");
    println!("BASE_SHA: {}", base_sha);
    println!("HEAD_SHA: {}", head_sha);
    println!("DESCRIPTION: [brief summary]");

    Ok(None)
}
