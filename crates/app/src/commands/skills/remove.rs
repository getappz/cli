//! Remove a skill by name from ~/.appz/skills or project .agents/skills.

use crate::session::AppzSession;
use starbase::AppResult;
use starbase_utils::fs as starbase_fs;
use std::path::PathBuf;
use ui::prompt::confirm;

/// Remove a skill by name. Searches project then global; removes from first match.
pub async fn remove(session: AppzSession, name: String, yes: bool) -> AppResult {
    let project_dir = session.working_dir.join(".agents").join("skills");
    let global_dir = common::user_config::user_appz_dir().map(|d| d.join("skills"));

    let mut to_remove: Vec<(PathBuf, &'static str)> = Vec::new();

    if project_dir.exists() {
        if let Some(path) = find_skill_by_name(&project_dir, &name) {
            to_remove.push((path, "project"));
        }
    }
    if let Some(ref dir) = global_dir {
        if dir.exists() {
            if let Some(path) = find_skill_by_name(dir, &name) {
                to_remove.push((path, "global"));
            }
        }
    }

    if to_remove.is_empty() {
        return Err(miette::miette!("Skill '{}' not found", name).into());
    }

    if !yes {
        println!("\nThe following skill(s) will be removed:");
        for (path, scope) in &to_remove {
            println!("  - {} ({})", path.display(), scope);
        }
        if !confirm("Continue?", false)? {
            return Ok(None);
        }
    }

    for (path, scope) in &to_remove {
        starbase_fs::remove_dir_all(&path)
            .map_err(|e| miette::miette!("Failed to remove skill: {}", e))?;
        let _ = ui::status::success(&format!("Removed skill '{}' ({})", name, scope));
    }

    Ok(None)
}

fn find_skill_by_name(root: &std::path::Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                if let Some(dir_name) = path.file_name() {
                    if dir_name.to_string_lossy().eq_ignore_ascii_case(name) {
                        return Some(path);
                    }
                }
            }
            if let Some(found) = find_skill_by_name(&path, name) {
                return Some(found);
            }
        }
    }
    None
}
