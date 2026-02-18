//! Skill lock file for update tracking.
//!
//! Located at ~/.appz/.skill-lock.json. Schema v3 mirrors skills.sh reference.

use crate::context::SkillsContext;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use starbase_utils::fs;
use std::collections::HashMap;
use std::path::PathBuf;

const LOCK_FILE: &str = ".skill-lock.json";
const CURRENT_VERSION: u32 = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillLockEntry {
    /// Normalized source identifier (e.g., "owner/repo", "mintlify/bun.com")
    pub source: String,
    /// Provider/source type (e.g., "github", "mintlify", "huggingface", "local")
    pub source_type: String,
    /// Original URL used to install (for re-fetching updates)
    pub source_url: String,
    /// Subpath within the source repo, if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_path: Option<String>,
    /// GitHub tree SHA for the skill folder (fetched via Trees API)
    #[serde(default)]
    pub skill_folder_hash: String,
    /// ISO timestamp when first installed
    pub installed_at: String,
    /// ISO timestamp when last updated
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DismissedPrompts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find_skills_prompt: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SkillLockFile {
    pub version: u32,
    pub skills: HashMap<String, SkillLockEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dismissed: Option<DismissedPrompts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_selected_agents: Option<Vec<String>>,
}

/// Path to the global skill lock file (~/.appz/.skill-lock.json).
pub fn lock_path(ctx: &SkillsContext) -> Option<PathBuf> {
    ctx.user_appz_dir.as_ref().map(|d| d.join(LOCK_FILE))
}

/// Read the lock file. Returns empty structure if missing or invalid.
pub fn read_skill_lock(ctx: &SkillsContext) -> SkillLockFile {
    let Some(path) = lock_path(ctx) else {
        return create_empty_lock();
    };
    if !path.exists() {
        return create_empty_lock();
    }
    match fs::read_file(&path) {
        Ok(content) => match serde_json::from_str::<SkillLockFile>(&content) {
            Ok(lock) => {
                if lock.version < CURRENT_VERSION {
                    return create_empty_lock();
                }
                lock
            }
            Err(_) => create_empty_lock(),
        },
        Err(_) => create_empty_lock(),
    }
}

fn create_empty_lock() -> SkillLockFile {
    SkillLockFile {
        version: CURRENT_VERSION,
        skills: HashMap::new(),
        dismissed: None,
        last_selected_agents: None,
    }
}

/// Write the lock file.
pub fn write_skill_lock(ctx: &SkillsContext, lock: &SkillLockFile) -> Result<(), miette::Report> {
    let Some(appz_dir) = &ctx.user_appz_dir else {
        return Err(miette::miette!("Could not determine home directory").into());
    };
    starbase_utils::fs::create_dir_all(appz_dir)
        .map_err(|e| miette::miette!("Failed to create appz dir: {}", e))?;
    let path = appz_dir.join(LOCK_FILE);
    let content = serde_json::to_string_pretty(lock)
        .map_err(|e| miette::miette!("Failed to serialize lock: {}", e))?;
    fs::write_file(&path, &content)
        .map_err(|e| miette::miette!("Failed to write lock file: {}", e))?;
    Ok(())
}

/// Add or update a skill in the lock file.
pub fn add_skill_to_lock(
    ctx: &SkillsContext,
    skill_name: String,
    entry: AddSkillLockInput,
) -> Result<(), miette::Report> {
    let mut lock = read_skill_lock(ctx);
    let now = Utc::now().to_rfc3339();
    let existing = lock.skills.get(&skill_name);

    lock.skills.insert(
        skill_name,
        SkillLockEntry {
            source: entry.source,
            source_type: entry.source_type,
            source_url: entry.source_url,
            skill_path: entry.skill_path,
            skill_folder_hash: entry.skill_folder_hash.unwrap_or_default(),
            installed_at: existing
                .map(|e| e.installed_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
        },
    );
    write_skill_lock(ctx, &lock)
}

#[derive(Clone, Debug)]
pub struct AddSkillLockInput {
    pub source: String,
    pub source_type: String,
    pub source_url: String,
    pub skill_path: Option<String>,
    pub skill_folder_hash: Option<String>,
}

/// Remove a skill from the lock file.
pub fn remove_skill_from_lock(ctx: &SkillsContext, skill_name: &str) -> Result<bool, miette::Report> {
    let mut lock = read_skill_lock(ctx);
    if lock.skills.remove(skill_name).is_some() {
        write_skill_lock(ctx, &lock)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Get all locked skills.
pub fn get_all_locked_skills(ctx: &SkillsContext) -> HashMap<String, SkillLockEntry> {
    read_skill_lock(ctx).skills
}

/// Get skills grouped by source for batch operations.
pub fn get_skills_by_source(
    ctx: &SkillsContext,
) -> HashMap<String, Vec<(String, SkillLockEntry)>> {
    let lock = read_skill_lock(ctx);
    let mut by_source: HashMap<String, Vec<(String, SkillLockEntry)>> = HashMap::new();
    for (name, entry) in lock.skills {
        by_source
            .entry(entry.source.clone())
            .or_default()
            .push((name, entry));
    }
    by_source
}

/// Fetch the tree SHA (folder hash) for a skill folder via GitHub Trees API.
pub async fn fetch_skill_folder_hash(
    owner_repo: &str,
    skill_path: &str,
    token: Option<&str>,
) -> Option<String> {
    let mut folder_path = skill_path.replace('\\', "/");
    if folder_path.ends_with("/SKILL.md") {
        folder_path = folder_path[..folder_path.len() - 9].to_string();
    } else if folder_path.ends_with("SKILL.md") {
        folder_path = folder_path[..folder_path.len() - 8].to_string();
    }
    if folder_path.ends_with('/') {
        folder_path.pop();
    }

    for branch in ["main", "master"] {
        let url = format!(
            "https://api.github.com/repos/{}/git/trees/{}?recursive=1",
            owner_repo, branch
        );
        let client = reqwest::Client::new();
        let mut req = client.get(&url).header(
            "Accept",
            "application/vnd.github.v3+json",
        );
        if let Some(t) = token {
            req = req.header("Authorization", format!("Bearer {}", t));
        }
        if let Ok(res) = req.send().await {
            if !res.status().is_success() {
                continue;
            }
            if let Ok(data) = res.json::<serde_json::Value>().await {
                let sha = data.get("sha")?.as_str()?.to_string();
                if folder_path.is_empty() {
                    return Some(sha);
                }
                let tree = data.get("tree")?.as_array()?;
                for entry in tree {
                    let path = entry.get("path")?.as_str()?;
                    let typ = entry.get("type")?.as_str()?;
                    if typ == "tree" && path == folder_path {
                        return entry.get("sha").and_then(|s| s.as_str()).map(String::from);
                    }
                }
            }
        }
    }
    None
}

/// Get GitHub token from env or `gh auth token`.
pub fn get_github_token() -> Option<String> {
    if let Ok(t) = std::env::var("GITHUB_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    if let Ok(t) = std::env::var("GH_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let output = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .ok()?;
    if output.status.success() {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }
    None
}
