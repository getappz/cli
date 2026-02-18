//! Remove a skill by name from agent skills dirs.

use crate::agents::{self, SkillDir};
use crate::context::SkillsContext;
use crate::skill_lock;
use starbase::AppResult;
use starbase_utils::fs as starbase_fs;
use std::path::PathBuf;
use ui::prompt::confirm;

/// Remove a skill by name. Searches agent dirs; removes from first match.
/// When global_only or project_only is set, only searches/removes from that scope.
pub async fn remove(
    ctx: &SkillsContext,
    name: Option<String>,
    yes: bool,
    global_only: bool,
    project_only: bool,
    agent: &[String],
    all: bool,
) -> AppResult {
    if all {
        return remove_all(ctx, yes, global_only, project_only, agent).await;
    }

    let name = name.ok_or_else(|| miette::miette!("Skill name required. Use --all to remove all skills."))?;

    let dirs = agents::skill_dirs_for_list_remove(
        agent,
        ctx.working_dir.as_path(),
        ctx.user_appz_dir.as_deref(),
        global_only,
        project_only,
    );

    let mut to_remove: Vec<(PathBuf, String)> = Vec::new();
    for SkillDir { path, scope } in dirs {
        if let Some(skill_path) = find_skill_by_name(&path, &name) {
            to_remove.push((skill_path, scope));
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
            let _ = ui::layout::indented(
                &format!("{} ({})", common::user_config::path_for_display(path), scope),
                1,
            );
        }
        let _ = ui::layout::blank_line();
        if !confirm("Continue?", false)? {
            let _ = ui::status::info("Canceled.");
            return Ok(None);
        }
    }

    for (path, scope) in &to_remove {
        starbase_fs::remove_dir_all(path)
            .map_err(|e| miette::miette!("Failed to remove skill: {}", e))?;
        if scope == "global" || scope == "appz (global)" {
            let _ = skill_lock::remove_skill_from_lock(ctx, name.as_str());
        }
        let _ = ui::status::success(&format!("Removed skill '{}' ({})", name, scope));
    }

    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!("Done. Skill '{}' removed.", name));

    Ok(None)
}

async fn remove_all(
    ctx: &SkillsContext,
    yes: bool,
    global_only: bool,
    project_only: bool,
    agent: &[String],
) -> AppResult {
    let mut to_remove: Vec<(String, PathBuf, String)> = Vec::new();

    let dirs = agents::skill_dirs_for_list_remove(
        agent,
        ctx.working_dir.as_path(),
        ctx.user_appz_dir.as_deref(),
        global_only,
        project_only,
    );

    for SkillDir { path, scope } in dirs {
        collect_skill_names(&path, &scope, &mut to_remove);
    }

    if to_remove.is_empty() {
        let _ = ui::status::info("No skills to remove.");
        return Ok(None);
    }

    if !yes {
        let _ = ui::layout::blank_line();
        let _ = ui::layout::section_title("Remove all skills");
        let _ = ui::status::info("The following skill(s) will be removed:");
        for (name, _path, scope) in &to_remove {
            let _ = ui::layout::indented(
                &format!("{} ({})", name, scope),
                1,
            );
        }
        let _ = ui::layout::blank_line();
        if !confirm("Continue? This will remove all listed skills.", false)? {
            let _ = ui::status::info("Canceled.");
            return Ok(None);
        }
    }

    for (name, path, scope) in &to_remove {
        starbase_fs::remove_dir_all(path)
            .map_err(|e| miette::miette!("Failed to remove skill: {}", e))?;
        if scope == "global" || scope == "appz (global)" {
            let _ = skill_lock::remove_skill_from_lock(ctx, name.as_str());
        }
        let _ = ui::status::success(&format!("Removed skill '{}' ({})", name, scope));
    }

    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!("Done. {} skill(s) removed.", to_remove.len()));

    Ok(None)
}

fn collect_skill_names(
    root: &std::path::Path,
    scope: &str,
    out: &mut Vec<(String, PathBuf, String)>,
) {
    let Ok(entries) = starbase_fs::read_dir(root) else {
        return;
    };
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                if let Some(name) = path.file_name() {
                    out.push((name.to_string_lossy().to_string(), path, scope.to_string()));
                }
            } else {
                collect_skill_names(&path, scope, out);
            }
        }
    }
}

fn find_skill_by_name(root: &std::path::Path, name: &str) -> Option<PathBuf> {
    let entries = starbase_fs::read_dir(root).ok()?;
    for entry in entries {
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
