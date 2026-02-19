//! Scaffold a new skill directory with SKILL.md template.

use crate::context::SkillsContext;
use starbase::AppResult;
use starbase_utils::fs;
use std::path::PathBuf;

const SKILL_TEMPLATE: &str = r#"---
name: {name}
description: >
  Describe what this skill does and when to use it.
---

# {name}

## When to use

Describe when an AI agent should invoke this skill.

## How to use

Step-by-step instructions for using this skill.

## Examples

Provide concrete examples.
"#;

/// Create a new skill directory with SKILL.md template.
pub async fn init_cmd(ctx: &SkillsContext, name: Option<String>, output: Option<PathBuf>) -> AppResult {
    let name = name.ok_or_else(|| miette::miette!("Skill name required. Usage: appz skills create <name>"))?;

    // Sanitize name: lowercase, replace spaces/special chars with hyphens
    let sanitized: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let sanitized = sanitized.trim_matches('-');
    let sanitized = sanitized.replace("--", "-");
    if sanitized.is_empty() {
        return Err(miette::miette!("Invalid skill name: '{}'", name));
    }

    let base = output
        .unwrap_or_else(|| ctx.working_dir.clone());
    let skill_dir = base.join(&sanitized);

    if skill_dir.exists() {
        return Err(miette::miette!(
            "Directory already exists: {}. Choose a different name or remove it first.",
            common::user_config::path_for_display(&skill_dir)
        ));
    }

    fs::create_dir_all(&skill_dir)
        .map_err(|e| miette::miette!("Failed to create directory: {}", e))?;

    let content = SKILL_TEMPLATE.replace("{name}", &sanitized);
    let skill_file = skill_dir.join("SKILL.md");
    fs::write_file(&skill_file, &content)
        .map_err(|e| miette::miette!("Failed to write SKILL.md: {}", e))?;

    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!("Created skill: {}", sanitized));
    let _ = ui::layout::indented(&format!("  {}", common::user_config::path_for_display(&skill_file)), 1);
    let _ = ui::layout::blank_line();

    Ok(None)
}
