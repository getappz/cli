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
        let _ = ui::status::error(&format!("Skill '{}' not found", name));
        return Err(miette::miette!(
            "Skill '{}' not found in project or global skills. Use `appz skills list` to see installed skills.",
            name
        )
        .into());
    }

    if !yes {
        let _ = ui::layout::blank_line();
        let _ = ui::layout::section_title("Remove skill");
        let _ = ui::status::info("The following skill(s) will be removed:");
        for (path, scope) in &to_remove {
            let _ = ui::layout::indented(&format!("{} ({})", path.display(), scope), 1);
        }
        let _ = ui::layout::blank_line();
        if !confirm("Continue?", false)? {
            let _ = ui::status::info("Canceled.");
            return Ok(None);
        }
    }

    for (path, scope) in &to_remove {
        starbase_fs::remove_dir_all(&path)
            .map_err(|e| miette::miette!("Failed to remove skill: {}", e))?;
        let _ = ui::status::success(&format!("Removed skill '{}' ({})", name, scope));
    }

    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!("Done. Skill '{}' removed.", name));

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
