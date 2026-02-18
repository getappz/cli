//! List installed skills from agent skills dirs (~/.appz/skills, .cursor/skills, etc.).

use crate::agents::{self, SkillDir};
use crate::context::SkillsContext;
use serde::Deserialize;
use starbase::AppResult;
use starbase_utils::fs;
use std::path::{Path, PathBuf};

/// List skills from agent project and global directories.
pub async fn list(
    ctx: &SkillsContext,
    global_only: bool,
    project_only: bool,
    agent: &[String],
) -> AppResult {
    let mut skills: Vec<(String, String, PathBuf, String)> = Vec::new();

    let dirs = agents::skill_dirs_for_list_remove(
        agent,
        ctx.working_dir.as_path(),
        ctx.user_appz_dir.as_deref(),
        global_only,
        project_only,
    );

    for SkillDir { path, scope } in dirs {
        collect_skills_from_dir(&path, scope, &mut skills);
    }

    if skills.is_empty() {
        ui::empty::display(
            "No skills installed",
            Some("Run `appz skills add <source>` to add a skill (e.g. owner/repo or a GitHub URL)."),
        )?;
        return Ok(None);
    }

    // Deduplicate by name (project overrides global)
    let mut seen = std::collections::HashSet::new();
    let mut displayed = Vec::new();
    for (name, desc, path, scope) in skills {
        let key = name.to_lowercase();
        if !seen.contains(&key) {
            seen.insert(key);
            displayed.push((name, desc, path, scope));
        }
    }

    let _ = ui::layout::blank_line();
    let _ = ui::layout::section_title("Installed skills");
    let _ = ui::layout::blank_line();
    for (name, description, path, scope) in displayed {
        let _ = ui::status::info(&format!("{} ({})", name, scope));
        let _ = ui::layout::indented(&description, 1);
        let _ = ui::layout::indented(&common::user_config::path_for_display(&path), 1);
        let _ = ui::layout::blank_line();
    }

    Ok(None)
}

/// Recursively collect skills (directories containing SKILL.md) from a directory.
fn collect_skills_from_dir(
    dir: &Path,
    scope: impl AsRef<str>,
    out: &mut Vec<(String, String, PathBuf, String)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                if let Ok(content) = fs::read_file(&skill_file) {
                    if let Ok((name, desc)) = parse_skill_frontmatter(&content) {
                        out.push((name, desc, path, scope.as_ref().to_string()));
                    }
                }
            } else {
                collect_skills_from_dir(&path, scope.as_ref(), out);
            }
        }
    }
}

#[derive(Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
}

fn parse_skill_frontmatter(content: &str) -> Result<(String, String), miette::Report> {
    let content = content.trim_start();
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| miette::miette!("No YAML frontmatter found"))?;
    let rest = rest.trim_start_matches(|c| c == '\n' || c == '\r');
    let end = rest
        .find("\n---")
        .or_else(|| rest.find("\r\n---"))
        .ok_or_else(|| miette::miette!("No closing --- in frontmatter"))?;
    let yaml = rest[..end].trim();
    let parsed: SkillFrontmatter = serde_yaml::from_str(yaml)
        .map_err(|e| miette::miette!("Invalid frontmatter: {}", e))?;
    Ok((parsed.name, parsed.description))
}
