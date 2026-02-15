//! Agent Skills discovery and management.
//!
//! Follows the [Agent Skills specification](https://agentskills.io/specification):
//! skills are directories containing `SKILL.md` with YAML frontmatter (name, description)
//! and Markdown instructions.

mod tool;

use sandbox::scoped_fs::ScopedFs;
use std::path::PathBuf;

use crate::error::{AiError, AiResult};

pub use tool::create_load_skill_tool;

/// Metadata extracted from a skill's SKILL.md frontmatter.
#[derive(Debug, Clone)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    /// Path to the skill directory (absolute for whitelisted, or stored for relative).
    pub path: PathBuf,
    /// Whether this skill is under a whitelisted (allowed) path vs sandbox root.
    pub is_allowed: bool,
}

/// A skill directory to scan: either relative to sandbox root or an absolute whitelisted path.
#[derive(Debug, Clone)]
pub enum SkillDir {
    /// Path relative to sandbox root (e.g. `.agents/skills`).
    Relative(PathBuf),
    /// Absolute path under a whitelisted directory (e.g. `~/.appz/skills`).
    Allowed(PathBuf),
}

/// Discover skills from the given directories.
///
/// Scans each directory for subdirectories containing `SKILL.md`, parses frontmatter,
/// and returns metadata. First skill with a given name wins (project overrides user globals).
pub fn discover_skills(
    fs: &ScopedFs,
    directories: &[SkillDir],
) -> AiResult<Vec<SkillMetadata>> {
    let mut skills = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    for dir in directories {
        let entries = match dir {
            SkillDir::Relative(rel) => {
                if !fs.exists(rel) {
                    continue;
                }
                fs.list_dir(rel).map_err(|e| AiError::SkillError {
                    reason: format!("Failed to list {}: {}", rel.display(), e),
                })?
            }
            SkillDir::Allowed(abs) => fs.list_dir_allowed(abs).map_err(|e| AiError::SkillError {
                reason: format!("Failed to list {}: {}", abs.display(), e),
            })?,
        };

        for entry in entries {
            if !entry.is_dir {
                continue;
            }
            let skill_path = match dir {
                SkillDir::Relative(rel) => rel.join(&entry.name),
                SkillDir::Allowed(_) => PathBuf::from(entry.abs_path.clone()),
            };
            let skill_file = match dir {
                SkillDir::Relative(rel) => rel.join(&entry.name).join("SKILL.md"),
                SkillDir::Allowed(_) => entry.abs_path.join("SKILL.md"),
            };

            let content = match dir {
                SkillDir::Relative(_) => fs.read_to_string(&skill_file),
                SkillDir::Allowed(_) => fs.read_allowed(&skill_file),
            };

            let content = match content {
                Ok(c) => c,
                Err(_) => continue,
            };

            let meta = match parse_frontmatter(&content) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if seen_names.contains(meta.name.to_lowercase().as_str()) {
                continue;
            }
            seen_names.insert(meta.name.to_lowercase());

            let path = match dir {
                SkillDir::Relative(_) => fs.resolve(&skill_path).unwrap_or(skill_path),
                SkillDir::Allowed(_) => entry.abs_path,
            };

            skills.push(SkillMetadata {
                name: meta.name,
                description: meta.description,
                path,
                is_allowed: matches!(dir, SkillDir::Allowed(_)),
            });
        }
    }

    Ok(skills)
}

/// Frontmatter parsed from SKILL.md.
#[derive(Debug)]
struct SkillFrontmatter {
    name: String,
    description: String,
}

/// Parse YAML frontmatter from SKILL.md content.
fn parse_frontmatter(content: &str) -> AiResult<SkillFrontmatter> {
    let content = content.trim_start();
    let rest = content.strip_prefix("---").ok_or_else(|| AiError::SkillError {
        reason: "No valid YAML frontmatter (--- ... ---) found".to_string(),
    })?;
    let rest = rest.trim_start_matches(|c| c == '\n' || c == '\r');
    let end_marker = rest.find("\n---").or_else(|| rest.find("\r\n---"));
    let yaml = end_marker
        .map(|i| rest[..i].trim())
        .unwrap_or(rest.trim());

    #[derive(serde::Deserialize)]
    struct Frontmatter {
        name: String,
        description: String,
    }

    let parsed: Frontmatter = serde_yaml::from_str(yaml).map_err(|e| AiError::SkillError {
        reason: format!("Invalid frontmatter YAML: {}", e),
    })?;

    Ok(SkillFrontmatter {
        name: parsed.name,
        description: parsed.description,
    })
}

/// Strip the frontmatter block from SKILL.md and return the Markdown body.
pub fn strip_frontmatter(content: &str) -> String {
    if let Some(rest) = content.strip_prefix("---") {
        if let Some(idx) = rest.find("\n---") {
            return rest[idx + 4..].trim_start().to_string();
        }
    }
    content.trim().to_string()
}

/// Build the skills section for the system prompt.
pub fn build_skills_prompt(skills: &[SkillMetadata]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let list: String = skills
        .iter()
        .map(|s| format!("- {}: {}", s.name, s.description))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r##"
## Skills

Use the loadSkill tool when the user's request would benefit from specialized instructions.

Available skills:
{}
"##,
        list
    )
}
