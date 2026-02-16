//! Add (install) a skill from GitHub, URL, or local path.
//!
//! When installing globally, if the current directory has agent dirs (e.g. `.cursor`, `.claude`),
//! creates symlinks so the installed skill is visible to those agents (e.g. `.cursor/skills/<name>`).

use crate::session::AppzSession;
use init::sources::git::{download_git, parse_git_source};
use starbase::AppResult;
use std::path::{Path, PathBuf};
use starbase_utils::fs as starbase_fs;

/// Agent config dirs that may contain a `skills/` subdir. If present in cwd, we symlink installed skills there.
const AGENT_DIRS: &[&str] = &[".cursor", ".claude"];

/// Install a skill from the given source.
pub async fn add(
    session: AppzSession,
    source: String,
    global: bool,
    project: bool,
    yes: bool,
    skill_filter: Option<String>,
) -> AppResult {
    let _ = ui::layout::blank_line();
    let target_dir = resolve_target_dir(&session, global, project)?;
    starbase_fs::create_dir_all(&target_dir)
        .map_err(|e| miette::miette!("Failed to create skills directory: {}", e))?;

    let source_dir = if is_git_source(&source) {
        // If URL points to a single skill and it's already installed, skip download
        let use_existing = try_existing_skill_from_git_url(&source, &target_dir, skill_filter.as_deref());
        if let Some(existing_path) = use_existing {
            let _ = ui::status::info("Skill already installed; skipping download.");
            existing_path
        } else {
            let _ = ui::status::info(&format!("Downloading skill from {}...", source));
            download_git(&source, None, None, !session.cli.verbose, Some("Downloading skill...")).await.map_err(|e| {
                let msg = e.to_string();
                miette::miette!(
                    "Could not download skill from \"{}\".\n\n{}\n\nCheck your network connection and that the repository exists. For GitHub you can use: owner/repo or a full URL (e.g. .../tree/main/path/to/skill).",
                    source,
                    msg
                )
            })?
        }
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

    let _ = ui::status::info(&format!(
        "Found {} skill(s). Installing to {}",
        to_install.len(),
        common::user_config::path_for_display(&target_dir)
    ));
    let _ = ui::layout::blank_line();

    let show_install_spinner = to_install.len() > 1 && !session.cli.verbose;
    let mut install_spinner: Option<ui::progress::SpinnerHandle> = if show_install_spinner {
        Some(ui::progress::spinner("Installing skills..."))
    } else {
        None
    };

    for (name, path) in &to_install {
        let dest = target_dir.join(name);
        if path.as_path() == dest.as_path() {
            let _ = ui::status::success(&format!("Already installed: {}", name));
            continue;
        }
        if dest.exists() && !yes {
            install_spinner = None;
            let overwrite = inquire::Confirm::new(&format!(
                "Skill '{}' already exists. Overwrite?",
                name
            ))
            .with_default(false)
            .prompt()
            .map_err(|e| miette::miette!("Prompt failed: {}", e))?;
            if !overwrite {
                if show_install_spinner {
                    install_spinner = Some(ui::progress::spinner("Installing skills..."));
                }
                continue;
            }
            if show_install_spinner {
                install_spinner = Some(ui::progress::spinner("Installing skills..."));
            }
        }
        copy_skill_dir(path, &dest)?;
        let _ = ui::status::success(&format!("Installed skill: {}", name));
    }

    // When installing globally, symlink into project agent dirs (.cursor, .claude) if present
    if !project {
        link_skills_into_agent_dirs(&session.working_dir, &target_dir, &to_install)?;
    }

    let _ = ui::layout::blank_line();
    let _ = ui::status::success(&format!(
        "Done. {} skill(s) installed to {}",
        to_install.len(),
        common::user_config::path_for_display(&target_dir)
    ));

    Ok(None)
}

/// For each agent dir (e.g. .cursor, .claude) present in cwd, create skills subdir and symlink each installed skill.
fn link_skills_into_agent_dirs(
    cwd: &Path,
    installed_skills_dir: &Path,
    installed: &[(String, PathBuf)],
) -> Result<(), miette::Report> {
    for &agent_dir_name in AGENT_DIRS {
        let agent_dir = cwd.join(agent_dir_name);
        if !agent_dir.is_dir() {
            continue;
        }
        let skills_dir = agent_dir.join("skills");
        starbase_fs::create_dir_all(&skills_dir)
            .map_err(|e| miette::miette!("Failed to create {}: {}", skills_dir.display(), e))?;

        for (name, _) in installed {
            let dest = installed_skills_dir.join(name);
            let link_path = skills_dir.join(name);
            if !dest.exists() {
                continue;
            }
            if let Err(e) = create_skill_symlink(&dest, &link_path) {
                let _ = ui::status::warning(&format!(
                    "Could not link {} into {}: {}",
                    name,
                    agent_dir_name,
                    e
                ));
            } else {
                let _ = ui::status::success(&format!(
                    "Linked {} into {}/skills/",
                    name,
                    agent_dir_name
                ));
            }
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn create_skill_symlink(target: &Path, link_path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    if link_path.exists() {
        let _ = starbase_fs::remove_file(link_path);
        let _ = starbase_fs::remove_dir_all(link_path);
    }
    let target_canon = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());
    symlink(target_canon, link_path)
}

#[cfg(target_os = "windows")]
fn create_skill_symlink(target: &Path, link_path: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::symlink_dir;
    if link_path.exists() {
        let _ = starbase_fs::remove_file(link_path);
        let _ = starbase_fs::remove_dir_all(link_path);
    }
    let target_canon = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());
    symlink_dir(target_canon, link_path)
}

/// If the git URL points to a single skill and that skill is already installed at target_dir, return its path so we skip download.
fn try_existing_skill_from_git_url(
    source: &str,
    target_dir: &Path,
    skill_filter: Option<&str>,
) -> Option<PathBuf> {
    let parsed = parse_git_source(source).ok()?;
    let subfolder = parsed.subfolder.as_deref()?;
    let name = subfolder.split('/').last()?.to_string();
    if name.is_empty() {
        return None;
    }
    if let Some(filter) = skill_filter {
        if !name.eq_ignore_ascii_case(filter) {
            return None;
        }
    }
    let existing = target_dir.join(&name);
    if existing.is_dir() && existing.join("SKILL.md").exists() {
        Some(existing)
    } else {
        None
    }
}

fn resolve_target_dir(session: &AppzSession, _global: bool, project: bool) -> Result<PathBuf, miette::Report> {
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
/// If root itself contains SKILL.md (e.g. when URL points at a single skill folder), it is included.
fn find_skill_dirs(root: &Path) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();
    if root.join("SKILL.md").exists() {
        if let Some(name) = root.file_name() {
            results.push((name.to_string_lossy().to_string(), root.to_path_buf()));
        }
    }
    let Ok(entries) = starbase_fs::read_dir(root) else {
        return results;
    };
    for entry in entries {
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

    let entries = starbase_fs::read_dir(src)
        .map_err(|e| miette::miette!("Failed to read source: {}", e))?;
    for entry in entries {
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
