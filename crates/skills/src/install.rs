//! Install skills from skills.json (declarative workflow).

use crate::add;
use crate::config::{self, ReadSkillsConfigOptions};
use crate::context::SkillsContext;
use crate::gitignore;
use starbase::AppResult;

/// Install all skills defined in skills.json.
pub async fn install(
    ctx: &SkillsContext,
    global: bool,
    agent: Vec<String>,
    yes: bool,
) -> AppResult {
    let _ = ui::layout::blank_line();

    let config_path = config::find_skills_config(&ctx.working_dir);
    let Some(_path) = config_path else {
        let _ = ui::status::warning("No skills.json found.");
        let _ = ui::layout::blank_line();
        let _ = ui::layout::indented(
            "Get started by adding a skill source:",
            1,
        );
        let _ = ui::layout::indented(
            "appz skills add vercel-labs/skills",
            2,
        );
        let _ = ui::layout::blank_line();
        return Ok(None);
    };

    let result = config::read_skills_config(
        ReadSkillsConfigOptions {
            cwd: None,
            create_if_not_exists: false,
        },
        &ctx.working_dir,
    )
    .map_err(|e| miette::miette!("Failed to read skills.json: {}", e))?;

    let skills = &result.config.skills;
    if skills.is_empty() {
        let _ = ui::status::info("No skills defined in skills.json.");
        let _ = ui::layout::blank_line();
        return Ok(None);
    }

    if let Err(e) = gitignore::add_gitignore_entries(&ctx.working_dir, &[".agents"], true) {
        let _ = ui::status::warning(&format!("Could not update .gitignore: {}", e));
    }

    let total = skills.len();
    let _ = ui::status::info(&format!(
        "Installing {} skill source{} from skills.json...",
        total,
        if total == 1 { "" } else { "s" }
    ));
    let _ = ui::layout::blank_line();

    let project = !global;
    let agents = if agent.is_empty() {
        vec!["claude-code".to_string()]
    } else {
        agent
    };

    for (i, entry) in skills.iter().enumerate() {
        let prefix = if total > 1 {
            format!("[{}/{}] ", i + 1, total)
        } else {
            String::new()
        };
        let skills_desc = entry
            .skills
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| s.join(", "))
            .unwrap_or_else(|| "all skills".to_string());
        let _ = ui::status::info(&format!("{}Installing {} ({})", prefix, entry.source, skills_desc));

        let skill_filters = entry
            .skills
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned();

        if let Err(e) = add::add(
            ctx,
            entry.source.clone(),
            global,
            project,
            yes,
            skill_filters.as_ref().and_then(|s| s.first().cloned()),
            false,
            &agents,
            false,
            false,
            skill_filters,
            true,  // no_save: skills already in config
            false, // code
            false, // compress
            None,  // directories
            None,  // workdir
            None,  // name
        )
        .await
        {
            let _ = ui::status::warning(&format!("Failed to install {}: {}", entry.source, e));
        }
    }

    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!(
        "Done. Installed skills from {} source{}.",
        total,
        if total == 1 { "" } else { "s" }
    ));

    Ok(None)
}
