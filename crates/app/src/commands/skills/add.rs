//! Add (install) a skill from GitHub, URL, or local path.

use crate::session::AppzSession;
use init::sources::git::download_git;
use starbase::AppResult;
use std::path::{Path, PathBuf};
use starbase_utils::fs as starbase_fs;

/// Install a skill from the given source.
pub async fn add(
    session: AppzSession,
    source: String,
    global: bool,
    project: bool,
    yes: bool,
    skill_filter: Option<String>,
) -> AppResult {
    let target_dir = resolve_target_dir(&session, global, project)?;
    starbase_fs::create_dir_all(&target_dir)
        .map_err(|e| miette::miette!("Failed to create skills directory: {}", e))?;

    let source_dir = if is_git_source(&source) {
        download_git(&source, None, None, !session.cli.verbose).await
            .map_err(|e| miette::miette!("Failed to download skill: {}", e))?
    } else if is_local_path(&source) {
        let cwd = session.working_dir.as_path();
        let path = if source.starts_with('/') || (source.len() >= 2 && &source[1..2] == ":") {
            PathBuf::from(&source)
        } else {
            cwd.join(&source)
        };
        path.canonicalize()
            .map_err(|e| miette::miette!("Local path not found: {} - {}", source, e))?
    } else {
        return Err(miette::miette!(
            "Invalid source: '{}'. Use owner/repo, https://..., or ./local-path",
            source
        )
        .into());
    };

    let skill_dirs = find_skill_dirs(&source_dir);

    if skill_dirs.is_empty() {
        return Err(miette::miette!(
            "No skills found (no SKILL.md in subdirectories)"
        )
        .into());
    }

    let to_install: Vec<_> = if let Some(ref name) = skill_filter {
        skill_dirs
            .into_iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .collect()
    } else {
        skill_dirs
    };

    if to_install.is_empty() {
        return Err(miette::miette!(
            "No matching skill '{}' found",
            skill_filter.as_deref().unwrap_or("")
        )
        .into());
    }

    for (name, path) in &to_install {
        let dest = target_dir.join(name);
        if dest.exists() && !yes {
            let overwrite = inquire::Confirm::new(&format!(
                "Skill '{}' already exists. Overwrite?",
                name
            ))
            .with_default(false)
            .prompt()
            .map_err(|e| miette::miette!("Prompt failed: {}", e))?;
            if !overwrite {
                continue;
            }
        }
        copy_skill_dir(path, &dest)?;
        let _ = ui::status::success(&format!("Installed skill: {}", name));
    }

    Ok(None)
}

fn resolve_target_dir(session: &AppzSession, global: bool, project: bool) -> Result<PathBuf, miette::Report> {
    if project {
        let dir = session.working_dir.join(".agents").join("skills");
        Ok(dir)
    } else {
        let appz_dir = common::user_config::user_appz_dir()
            .ok_or_else(|| miette::miette!("Could not determine home directory"))?;
        Ok(appz_dir.join("skills"))
    }
}

fn is_git_source(s: &str) -> bool {
    if s.starts_with("https://") || s.starts_with("http://") {
        let lower = s.to_lowercase();
        return lower.contains("github.com")
            || lower.contains("gitlab.com")
            || lower.contains("bitbucket.org");
    }
    if s.contains('/') && !s.starts_with("./") && !s.starts_with("../") {
        let parts: Vec<&str> = s.split('/').collect();
        return parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty();
    }
    false
}

fn is_local_path(s: &str) -> bool {
    s.starts_with("./") || s.starts_with("../") || s.starts_with('/')
        || (s.len() >= 2 && s.chars().nth(1) == Some(':') && !s.contains("github.com") && !s.contains("gitlab.com"))
}

/// Find directories containing SKILL.md (returns (skill_name, path)).
fn find_skill_dirs(root: &Path) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return results;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                if let Some(name) = path.file_name() {
                    results.push((name.to_string_lossy().to_string(), path));
                }
            } else {
                let nested = find_skill_dirs(&path);
                results.extend(nested);
            }
        }
    }
    results
}

fn copy_skill_dir(src: &Path, dest: &Path) -> Result<(), miette::Report> {
    if dest.exists() {
        starbase_fs::remove_dir_all(dest)
            .map_err(|e| miette::miette!("Failed to remove existing skill: {}", e))?;
    }
    starbase_fs::create_dir_all(dest)
        .map_err(|e| miette::miette!("Failed to create directory: {}", e))?;

    let entries = std::fs::read_dir(src)
        .map_err(|e| miette::miette!("Failed to read source: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap();
        let dest_path = dest.join(name);
        if path.is_dir() {
            copy_skill_dir(&path, &dest_path)?;
        } else {
            starbase_fs::copy_file(&path, &dest_path)
                .map_err(|e| miette::miette!("Failed to copy file: {}", e))?;
        }
    }
    Ok(())
}
