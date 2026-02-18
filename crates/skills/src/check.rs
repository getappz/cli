//! Check installed skills for updates. Uses lock file and GitHub Trees API.

use crate::context::SkillsContext;
use crate::skill_lock;
use starbase::AppResult;

/// Check which installed skills have updates available.
pub async fn check(ctx: &SkillsContext) -> AppResult {
    let lock = skill_lock::read_skill_lock(ctx);
    let skill_names: Vec<String> = lock.skills.keys().cloned().collect();

    if skill_names.is_empty() {
        let _ = ui::status::info("No skills tracked in lock file.");
        let _ = ui::layout::indented(
            "Install skills with: appz skills add <owner/repo> or appz skills add <url>",
            1,
        );
        return Ok(None);
    }

    let token = skill_lock::get_github_token();
    let mut updates: Vec<(String, String)> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut checked = 0u32;

    for name in &skill_names {
        let Some(entry) = lock.skills.get(name) else {
            continue;
        };
        if entry.source_type != "github"
            || entry.skill_folder_hash.is_empty()
            || entry.skill_path.is_none()
        {
            continue;
        }
        let skill_path = entry.skill_path.as_deref().unwrap_or("");
        checked += 1;
        match skill_lock::fetch_skill_folder_hash(
            &entry.source,
            skill_path,
            token.as_deref(),
        )
        .await
        {
            Some(latest) if latest != entry.skill_folder_hash => {
                updates.push((name.clone(), entry.source.clone()));
            }
            Some(_) => {}
            None => {
                errors.push((name.clone(), "Could not fetch from GitHub".to_string()));
            }
        }
    }

    let _ = ui::layout::blank_line();
    let _ = ui::layout::section_title("Check for updates");
    let _ = ui::layout::blank_line();

    if checked == 0 {
        let _ = ui::status::info("No GitHub skills to check (or lock file has no folder hashes).");
        let _ = ui::layout::indented(
            "Reinstall skills from GitHub to enable update tracking.",
            1,
        );
        return Ok(None);
    }

    if updates.is_empty() {
        let _ = ui::status::success("All skills are up to date");
    } else {
        let _ = ui::status::info(&format!("{} update(s) available:", updates.len()));
        let _ = ui::layout::blank_line();
        for (name, source) in &updates {
            let _ = ui::layout::indented(&format!("{} (source: {})", name, source), 1);
        }
        let _ = ui::layout::blank_line();
        let _ = ui::layout::indented("Run `appz skills update` to update all skills.", 1);
    }

    if !errors.is_empty() {
        let _ = ui::layout::blank_line();
        let _ = ui::status::warning(&format!(
            "Could not check {} skill(s) (may need reinstall)",
            errors.len()
        ));
    }

    let _ = ui::layout::blank_line();

    Ok(if updates.is_empty() { None } else { Some(1) })
}
