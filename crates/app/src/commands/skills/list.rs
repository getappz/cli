//! List installed skills from ~/.appz/skills and project .agents/skills.

use crate::session::AppzSession;
use serde::Deserialize;
use starbase::AppResult;
use std::path::{Path, PathBuf};

/// List skills from user global and project directories.
pub async fn list(session: AppzSession) -> AppResult {
    let mut skills: Vec<(String, String, PathBuf, &'static str)> = Vec::new();

    // Project skills (.agents/skills)
    let project_dir = session.working_dir.join(".agents").join("skills");
    if project_dir.exists() {
        collect_skills_from_dir(&project_dir, "project", &mut skills);
    }

    // User global skills (~/.appz/skills)
    if let Some(appz_dir) = common::user_config::user_appz_dir() {
        let user_dir = appz_dir.join("skills");
        if user_dir.exists() {
            collect_skills_from_dir(&user_dir, "global", &mut skills);
        }
    }

    if skills.is_empty() {
        let _ = ui::status::warning("No skills installed. Run `appz skills add <source>` to add skills.");
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

    println!("\nInstalled skills:\n");
    for (name, description, path, scope) in displayed {
        println!("  {} ({})", name, scope);
        println!("    {}", description);
        println!("    {}", path.display());
        println!();
    }

    Ok(None)
}

/// Recursively collect skills (directories containing SKILL.md) from a directory.
fn collect_skills_from_dir(
    dir: &Path,
    scope: &'static str,
    out: &mut Vec<(String, String, PathBuf, &'static str)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&skill_file) {
                    if let Ok((name, desc)) = parse_skill_frontmatter(&content) {
                        out.push((name, desc, path, scope));
                    }
                }
            } else {
                collect_skills_from_dir(&path, scope, out);
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
