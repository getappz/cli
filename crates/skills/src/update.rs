//! Update outdated skills. Reinstalls skills with updates via add flow.

use crate::add;
use crate::context::SkillsContext;
use crate::skill_lock;
use starbase::AppResult;

/// Update skills that have newer versions available.
pub async fn update(ctx: &SkillsContext) -> AppResult {
    let _ = ui::layout::blank_line();
    let _ = ui::status::info("Checking for skill updates...");
    let _ = ui::layout::blank_line();

    let lock = skill_lock::read_skill_lock(ctx);
    let token = skill_lock::get_github_token();

    let mut to_update: Vec<(String, String, String)> = Vec::new(); // (name, source, source_url)

    for (name, entry) in &lock.skills {
        if entry.source_type != "github"
            || entry.skill_folder_hash.is_empty()
            || entry.skill_path.is_none()
        {
            continue;
        }
        let skill_path = entry.skill_path.as_deref().unwrap_or("");
        match skill_lock::fetch_skill_folder_hash(&entry.source, skill_path, token.as_deref()).await
        {
            Some(latest) if latest != entry.skill_folder_hash => {
                let mut install_url = entry.source_url.replace(".git", "").trim_end_matches('/').to_string();
                let folder = skill_path
                    .strip_suffix("/SKILL.md")
                    .unwrap_or(skill_path)
                    .trim_end_matches('/')
                    .to_string();
                if !folder.is_empty() {
                    install_url.push_str("/tree/main/");
                    install_url.push_str(&folder);
                }
                to_update.push((name.clone(), entry.source.clone(), install_url));
            }
            _ => {}
        }
    }

    if to_update.is_empty() {
        let _ = ui::status::success("All skills are up to date");
        let _ = ui::layout::blank_line();
        return Ok(None);
    }

    let _ = ui::status::info(&format!("Found {} update(s)", to_update.len()));
    let _ = ui::layout::blank_line();

    let mut success = 0;
    let mut fail = 0;

    for (name, _source, install_url) in &to_update {
        let _ = ui::status::info(&format!("Updating {}...", name));
        match add::add(
            ctx,
            install_url.clone(),
            true,  // global
            false, // project
            true,  // yes
            Some(name.clone()),
            false, // list_only
            &[],
            false, // all
            false, // full_depth
            None,  // skill_filters_override
            true,  // no_save (update doesn't touch skills.json)
        )
        .await
        {
            Ok(_) => {
                success += 1;
                let _ = ui::status::success(&format!("Updated {}", name));
            }
            Err(e) => {
                fail += 1;
                let _ = ui::status::warning(&format!("Failed to update {}: {}", name, e));
            }
        }
    }

    let _ = ui::layout::blank_line();
    if success > 0 {
        let _ = ui::status::success(&format!("Updated {} skill(s)", success));
    }
    if fail > 0 {
        let _ = ui::status::warning(&format!("Failed to update {} skill(s)", fail));
    }
    let _ = ui::layout::blank_line();

    Ok(Some(if fail > 0 { 1 } else { 0 }))
}
