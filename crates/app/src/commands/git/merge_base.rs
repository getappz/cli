//! Merge base — Superpowers: finishing-a-development-branch

use crate::session::AppzSession;
use git2::Repository;
use starbase::AppResult;

pub async fn run(session: AppzSession, branch: String) -> AppResult {
    let repo = Repository::discover(&session.working_dir)
        .map_err(|e| miette::miette!("Not a git repository: {}", e))?;

    let head = repo
        .head()
        .map_err(|e| miette::miette!("No HEAD: {}", e))?
        .target()
        .ok_or_else(|| miette::miette!("HEAD is not a direct reference"))?;

    let branches: Vec<&str> = if branch == "main" {
        vec!["main", "master"]
    } else if branch == "master" {
        vec!["master", "main"]
    } else {
        vec![branch.as_str()]
    };

    for b in &branches {
        let branch_ref = match repo.find_branch(b, git2::BranchType::Local) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let branch_oid = match branch_ref.get().target() {
            Some(oid) => oid,
            None => continue,
        };

        if let Ok(merge_base) = repo.merge_base(head, branch_oid) {
            println!("{}", merge_base);
            return Ok(None);
        }
    }

    Err(miette::miette!(
        "Could not find merge base with {:?}. Ensure the branch exists.",
        branches
    ))
}
