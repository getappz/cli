//! skills.json config management.
//!
//! Find, read, update, and add skill sources. Schema compatible with skillman.

use serde::{Deserialize, Serialize};
use starbase_utils::fs;
use std::path::{Path, PathBuf};

const SKILLS_JSON: &str = "skills.json";
const SKILLS_SCHEMA: &str = "https://unpkg.com/skillman/skills_schema.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    pub skills: Vec<SkillSource>,

    /// Project analysis from detect (optional, ignored by install).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillSource {
    pub source: String,

    /// Specific skills to install (empty/omitted = all).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct SkillsConfigResult {
    pub config: SkillsConfig,
    pub path: PathBuf,
}

/// Find skills.json by traversing up from cwd.
pub fn find_skills_config<P: AsRef<Path>>(cwd: P) -> Option<PathBuf> {
    let cwd = cwd.as_ref();
    let mut dir = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

    loop {
        let candidate = dir.join(SKILLS_JSON);
        if candidate.exists() {
            return Some(candidate);
        }
        let parent = dir.parent()?;
        if parent == dir {
            break;
        }
        dir = parent.to_path_buf();
    }

    let root = dir;
    let root_candidate = root.join(SKILLS_JSON);
    if root_candidate.exists() {
        return Some(root_candidate);
    }

    None
}

#[derive(Clone, Debug, Default)]
pub struct ReadSkillsConfigOptions {
    pub cwd: Option<PathBuf>,
    pub create_if_not_exists: bool,
}

#[derive(Clone, Debug, Default)]
pub struct UpdateSkillsConfigOptions {
    pub cwd: Option<PathBuf>,
    pub create_if_not_exists: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AddSkillOptions {
    pub cwd: Option<PathBuf>,
    pub create_if_not_exists: bool,
}

fn default_config() -> SkillsConfig {
    SkillsConfig {
        schema: Some(SKILLS_SCHEMA.to_string()),
        skills: vec![],
        detected: None,
    }
}

fn assert_skills_config(value: &mut serde_json::Value) -> Result<SkillsConfig, miette::Report> {
    let obj = value.as_object_mut().ok_or_else(|| {
        miette::miette!("Invalid skills.json: expected object")
    })?;

    if !obj.contains_key("skills") {
        return Err(miette::miette!("Invalid skills.json: missing 'skills' key.").into());
    }

    if !obj.contains_key("$schema") {
        obj.insert(
            "$schema".to_string(),
            serde_json::Value::String(SKILLS_SCHEMA.to_string()),
        );
    }

    serde_json::from_value(serde_json::Value::Object(std::mem::take(obj)))
        .map_err(|e| miette::miette!("Invalid skills.json: {}", e).into())
}

/// Read and validate skills.json.
pub fn read_skills_config(
    opts: ReadSkillsConfigOptions,
    working_dir: &Path,
) -> Result<SkillsConfigResult, miette::Report> {
    let cwd = opts
        .cwd
        .unwrap_or_else(|| working_dir.to_path_buf())
        .canonicalize()
        .unwrap_or_else(|_| working_dir.to_path_buf());

    let skills_path = find_skills_config(&cwd);

    let (config, path) = match skills_path {
        Some(p) => {
            let content = fs::read_file(&p)
                .map_err(|e| miette::miette!("Failed to read skills.json: {}", e))?;
            let mut parsed: serde_json::Value = serde_json::from_str(&content)
                .map_err(|e| miette::miette!("Invalid JSON in skills.json: {}", e))?;
            let config = assert_skills_config(&mut parsed)?;
            (config, p)
        }
        None => {
            if opts.create_if_not_exists {
                let path = cwd.join(SKILLS_JSON);
                let config = default_config();
                let content = serde_json::to_string_pretty(&config)
                    .map_err(|e| miette::miette!("Failed to serialize config: {}", e))?;
                fs::write_file(&path, format!("{}\n", content))
                    .map_err(|e| miette::miette!("Failed to write skills.json: {}", e))?;
                (config, path)
            } else {
                return Err(miette::miette!(
                    "skills.json not found in current directory or any parent directory."
                )
                .into());
            }
        }
    };

    Ok(SkillsConfigResult { config, path })
}

/// Update skills.json with a callback.
pub fn update_skills_config<F>(
    opts: UpdateSkillsConfigOptions,
    working_dir: &Path,
    updater: F,
) -> Result<SkillsConfigResult, miette::Report>
where
    F: FnOnce(&mut SkillsConfig),
{
    let opts_read = ReadSkillsConfigOptions {
        cwd: opts.cwd.clone(),
        create_if_not_exists: opts.create_if_not_exists,
    };
    let mut result = read_skills_config(opts_read, working_dir)?;
    updater(&mut result.config);

    if result.config.schema.is_none() {
        result.config.schema = Some(SKILLS_SCHEMA.to_string());
    }

    let content = serde_json::to_string_pretty(&result.config)
        .map_err(|e| miette::miette!("Failed to serialize config: {}", e))?;
    fs::write_file(&result.path, format!("{}\n", content))
        .map_err(|e| miette::miette!("Failed to write skills.json: {}", e))?;

    Ok(result)
}

/// Add a skill source to skills.json. Merges with existing entry if present.
pub fn add_skill_to_config(
    source: String,
    skills: Vec<String>,
    opts: AddSkillOptions,
    working_dir: &Path,
) -> Result<SkillsConfigResult, miette::Report> {
    let create = opts.create_if_not_exists;
    update_skills_config(
        UpdateSkillsConfigOptions {
            cwd: opts.cwd,
            create_if_not_exists: create,
        },
        working_dir,
        |config| {
            let entry = config.skills.iter_mut().find(|s| s.source == source);

            if let Some(entry) = entry {
                if skills.is_empty() || entry.skills.as_ref().map(|s| s.is_empty()).unwrap_or(true)
                {
                    entry.skills = Some(vec![]);
                } else {
                    let mut merged: std::collections::HashSet<String> = entry
                        .skills
                        .as_ref()
                        .map(|s| s.iter().cloned().collect())
                        .unwrap_or_default();
                    for skill in &skills {
                        merged.insert(skill.clone());
                    }
                    entry.skills = Some(merged.into_iter().collect());
                }
            } else {
                config.skills.push(SkillSource {
                    source,
                    skills: if skills.is_empty() {
                        None
                    } else {
                        Some(skills)
                    },
                });
            }
        },
    )
}
