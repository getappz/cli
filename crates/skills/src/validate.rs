//! Validate SKILL.md structure (frontmatter + body).
//!
//! Ported from aistudio/backend/server.py validate_skill_name and validate_skill.

use crate::context::SkillsContext;
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;
use serde::Serialize;
use starbase::AppResult;
use starbase_utils::fs;
use std::path::{Path, PathBuf};

/// Result of validating a skill.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Deserialize)]
struct SkillFrontmatter {
    name: String,
    description: String,
    #[serde(default)]
    compatibility: Option<String>,
}

/// Validate a skill by path. Returns ValidationResult and the skill name for display.
pub fn validate_skill_at_path(path: &Path) -> Result<(ValidationResult, String), miette::Report> {
    let (skill_file, skill_name) = resolve_skill_path(path)?;
    let content = fs::read_file(&skill_file)
        .map_err(|e| miette::miette!("Failed to read {}: {}", skill_file.display(), e))?;
    let (frontmatter, body) = parse_frontmatter_and_body(&content)?;
    let result = validate_skill(&frontmatter, &body);
    Ok((result, skill_name))
}

fn resolve_skill_path(path: &Path) -> Result<(PathBuf, String), miette::Report> {
    let path = path
        .canonicalize()
        .map_err(|e| miette::miette!("Path not found: {} - {}", path.display(), e))?;
    let (skill_file, name) = if path.ends_with("SKILL.md") {
        let parent = path.parent().ok_or_else(|| miette::miette!("Invalid path"))?;
        let name = parent
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "skill".to_string());
        (path, name)
    } else if path.join("SKILL.md").exists() {
        let skill_file = path.join("SKILL.md");
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "skill".to_string());
        (skill_file, name)
    } else {
        return Err(miette::miette!(
            "No SKILL.md found at {}",
            path.display()
        )
        .into());
    };
    Ok((skill_file, name))
}

fn parse_frontmatter_and_body(content: &str) -> Result<(SkillFrontmatter, String), miette::Report> {
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
    let body = rest[end + 4..].trim_start().to_string();
    let frontmatter: SkillFrontmatter =
        serde_yaml::from_str(yaml).map_err(|e| miette::miette!("Invalid frontmatter: {}", e))?;
    Ok((frontmatter, body))
}

fn validate_skill_name(name: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if name.is_empty() {
        errors.push("Name is required".to_string());
        return errors;
    }
    if name.len() > 64 {
        errors.push("Name must be 64 characters or less".to_string());
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^[a-z][a-z0-9-]*[a-z0-9]$|^[a-z]$").unwrap());
    if !re.is_match(name) {
        errors.push("Name must contain only lowercase letters, numbers, and hyphens. Must start with a letter and not end with a hyphen".to_string());
    }
    if name.contains("--") {
        errors.push("Name cannot contain consecutive hyphens".to_string());
    }
    if name.starts_with('-') || name.ends_with('-') {
        errors.push("Name cannot start or end with a hyphen".to_string());
    }
    errors
}

fn validate_skill(frontmatter: &SkillFrontmatter, body: &str) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    errors.extend(validate_skill_name(&frontmatter.name));

    if frontmatter.description.is_empty() {
        errors.push("Description is required".to_string());
    } else if frontmatter.description.len() > 1024 {
        errors.push("Description must be 1024 characters or less".to_string());
    } else if frontmatter.description.len() < 20 {
        warnings.push("Description should be more detailed for better discoverability".to_string());
    }

    if let Some(ref compat) = frontmatter.compatibility {
        if compat.len() > 500 {
            errors.push("Compatibility must be 500 characters or less".to_string());
        }
    }

    let body_trimmed = body.trim();
    if body_trimmed.len() < 10 {
        warnings.push("Body content is very short. Consider adding detailed instructions".to_string());
    }
    if body.len() > 50000 {
        warnings.push("Body content is very long. Consider splitting into reference files".to_string());
    }

    let recommended = ["## when to use", "## how to", "## example"];
    let body_lower = body.to_lowercase();
    let found = recommended.iter().filter(|s| body_lower.contains(*s)).count();
    if found == 0 {
        warnings.push("Consider adding sections like 'When to use', 'How to', or 'Examples'".to_string());
    }

    ValidationResult {
        is_valid: errors.is_empty(),
        errors,
        warnings,
    }
}

/// Collect (name, path) of skills from project and user dirs. Deduplicates by name (project wins).
fn collect_skill_paths(ctx: &SkillsContext) -> Vec<(String, PathBuf)> {
    let mut skills: Vec<(String, PathBuf)> = Vec::new();

    let project_dir = ctx.working_dir.join(".agents").join("skills");
    if project_dir.exists() {
        collect_skills_from_dir(&project_dir, &mut skills);
    }
    if let Some(ref appz_dir) = ctx.user_appz_dir {
        let user_dir = appz_dir.join("skills");
        if user_dir.exists() {
            collect_skills_from_dir(&user_dir, &mut skills);
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for (name, path) in skills {
        let key = name.to_lowercase();
        if !seen.contains(&key) {
            seen.insert(key);
            result.push((name, path));
        }
    }
    result
}

fn collect_skills_from_dir(dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                if let Ok(content) = fs::read_file(&skill_file) {
                    if let Ok((name, _)) = parse_name_desc(&content) {
                        out.push((name, path));
                    }
                }
            } else {
                collect_skills_from_dir(&path, out);
            }
        }
    }
}

fn parse_name_desc(content: &str) -> Result<(String, String), miette::Report> {
    let content = content.trim_start();
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| miette::miette!("No YAML frontmatter"))?;
    let rest = rest.trim_start_matches(|c| c == '\n' || c == '\r');
    let end = rest
        .find("\n---")
        .or_else(|| rest.find("\r\n---"))
        .ok_or_else(|| miette::miette!("No closing ---"))?;
    let yaml = rest[..end].trim();
    #[derive(Deserialize)]
    struct Fm {
        name: String,
        description: String,
    }
    let fm: Fm = serde_yaml::from_str(yaml).map_err(|e| miette::miette!("Invalid YAML: {}", e))?;
    Ok((fm.name, fm.description))
}

/// CLI entry point for `appz skills validate [PATH]`.
pub async fn validate(
    ctx: &SkillsContext,
    path: Option<PathBuf>,
    strict: bool,
    json_output: bool,
) -> AppResult {
    let skills_to_validate: Vec<(String, PathBuf)> = if let Some(ref p) = path {
        let (skill_file, name) = resolve_skill_path(p)?;
        vec![(name, skill_file.parent().unwrap_or(p).to_path_buf())]
    } else {
        let collected = collect_skill_paths(ctx);
        if collected.is_empty() {
            if json_output {
                println!("[]");
            } else {
                ui::empty::display(
                    "No skills to validate",
                    Some("Run `appz skills add <source>` to add skills, or pass a path."),
                )?;
            }
            return Ok(None);
        }
        collected
    };

    let mut all_results: Vec<(String, PathBuf, ValidationResult)> = Vec::new();
    let show_spinner = skills_to_validate.len() > 1 && !json_output;
    let _validate_spinner = if show_spinner {
        Some(ui::progress::spinner("Validating skills..."))
    } else {
        None
    };
    for (name, skill_dir) in &skills_to_validate {
        let skill_file = skill_dir.join("SKILL.md");
        if !skill_file.exists() {
            continue;
        }
        let content = match fs::read_file(&skill_file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let (frontmatter, body) = match parse_frontmatter_and_body(&content) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let result = validate_skill(&frontmatter, &body);
        all_results.push((name.clone(), skill_dir.clone(), result));
    }

    if json_output {
        let output: Vec<(&str, &ValidationResult)> = all_results
            .iter()
            .map(|(n, _, r)| (n.as_str(), r))
            .collect();
        if output.len() == 1 {
            let json = serde_json::to_string_pretty(output[0].1)
                .map_err(|e| miette::miette!("JSON serialize: {}", e))?;
            println!("{}", json);
        } else {
            let vec: Vec<&ValidationResult> = all_results.iter().map(|(_, _, r)| r).collect();
            let json = serde_json::to_string_pretty(&vec)
                .map_err(|e| miette::miette!("JSON serialize: {}", e))?;
            println!("{}", json);
        }
        let has_invalid = all_results.iter().any(|(_, _, r)| {
            !r.is_valid || (strict && !r.warnings.is_empty())
        });
        return Ok(if has_invalid { Some(1) } else { None });
    }

    let _ = ui::layout::blank_line();
    let mut invalid_count = 0usize;

    for (name, path, result) in &all_results {
        let _ = ui::layout::section_title(name);
        let _ = ui::status::info(&common::user_config::path_for_display(&path.join("SKILL.md")));

        for err in &result.errors {
            let _ = ui::status::error(&format!("  {}", err));
        }
        for warn in &result.warnings {
            let _ = ui::status::warning(&format!("  {}", warn));
        }

        if result.is_valid && (result.warnings.is_empty() || !strict) {
            let _ = ui::status::success("Validation passed");
        } else {
            invalid_count += 1;
            if !result.is_valid {
                let _ = ui::status::error("Validation failed");
            } else if strict && !result.warnings.is_empty() {
                let _ = ui::status::error("Validation failed (strict: warnings treated as errors)");
            }
        }
        let _ = ui::layout::blank_line();
    }

    let total = all_results.len();
    if invalid_count == 0 {
        let _ = ui::status::success(&format!(
            "All {} skill{} valid",
            total,
            if total == 1 { "" } else { "s" }
        ));
    } else {
        let _ = ui::status::error(&format!(
            "{} of {} skill{} invalid",
            invalid_count,
            total,
            if total == 1 { "" } else { "s" }
        ));
    }

    Ok(if invalid_count > 0 { Some(1) } else { None })
}
