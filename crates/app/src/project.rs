//! Project context management module.
//!
//! This module provides functionality for linking directories to projects,
//! storing project configuration, and resolving project context from
//! multiple sources (environment variables, `.appz/project.json`).

mod config;
mod connect_git;
mod edit_project_settings;
mod humanize_path;
mod input_project;
mod input_root_directory;
mod select_org;

pub use config::read_config_async;
pub use connect_git::connect_git_repository;
pub use edit_project_settings::edit_project_settings;
pub use humanize_path::humanize_path;
pub use input_project::{input_project, Either};
pub use input_root_directory::input_root_directory;
pub use select_org::select_org;

use crate::commands::projects::user_friendly_list_projects_error;
use api::models::Project;
use api::Client;
use miette::{miette, Result};
use serde::{Deserialize, Serialize};
use starbase_utils::{fs, json};
use std::path::{Path, PathBuf};

/// Organization/Team type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrgType {
    User,
    Team,
}

/// Organization/Team information
#[derive(Debug, Clone)]
pub struct Org {
    /// Organization type (user or team)
    pub org_type: OrgType,
    /// Organization ID
    pub id: String,
    /// Organization slug
    pub slug: String,
}

impl std::fmt::Display for Org {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.slug)
    }
}

/// Project link information stored in `.appz/project.json`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLink {
    /// Project ID
    pub project_id: String,
    /// Team/Organization ID
    pub team_id: String,
    /// Project name
    #[serde(default)]
    pub project_name: Option<String>,
}

/// Project settings stored in `.appz/project.json`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectSettings {
    /// Build command
    #[serde(default)]
    pub build_command: Option<String>,
    /// Development command
    #[serde(default)]
    pub dev_command: Option<String>,
    /// Install command
    #[serde(default)]
    pub install_command: Option<String>,
    /// Output directory
    #[serde(default)]
    pub output_directory: Option<String>,
    /// Root directory (for monorepos)
    #[serde(default)]
    pub root_directory: Option<String>,
    /// Framework identifier
    #[serde(default)]
    pub framework: Option<String>,
    /// Node version
    #[serde(default)]
    pub node_version: Option<String>,
    /// Command for ignoring build step
    #[serde(default, rename = "command_for_ignoring_build_step")]
    pub command_for_ignoring_build_step: Option<String>,
}

/// Complete project context containing link and settings
#[derive(Debug, Clone)]
pub struct ProjectContext {
    /// Project link information
    pub link: ProjectLink,
    /// Project settings
    pub settings: ProjectSettings,
    /// Full project object from API (if loaded)
    pub project: Option<Project>,
}

/// Project link and settings combined (what's stored in project.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLinkAndSettings {
    /// Project link
    #[serde(flatten)]
    pub link: ProjectLink,
    /// Project settings
    #[serde(default)]
    pub settings: ProjectSettings,
}

/// Directory name for project configuration
pub const APPZ_DIR: &str = ".appz";
/// Project configuration file name
pub const PROJECT_JSON: &str = "project.json";

/// Get the `.appz` directory path for a given directory
pub fn get_appz_directory(cwd: &Path) -> PathBuf {
    cwd.join(APPZ_DIR)
}

/// Read project link from `.appz/project.json` (sync version for non-async contexts)
pub fn read_project_link(cwd: &Path) -> Result<Option<ProjectLinkAndSettings>> {
    let appz_dir = get_appz_directory(cwd);
    let project_json = appz_dir.join(PROJECT_JSON);

    if !project_json.exists() {
        return Ok(None);
    }

    let link_and_settings: ProjectLinkAndSettings = json::read_file(&project_json)
        .map_err(|e| miette!("Failed to read/parse {}: {}", project_json.display(), e))?;

    Ok(Some(link_and_settings))
}

/// Read project link from `.appz/project.json` (async version)
pub async fn read_project_link_async(cwd: &Path) -> Result<Option<ProjectLinkAndSettings>> {
    let appz_dir = get_appz_directory(cwd);
    let project_json = appz_dir.join(PROJECT_JSON);

    // Use spawn_blocking for file I/O in async context (following workspace rules)
    let project_json_clone = project_json.clone();
    let exists = tokio::task::spawn_blocking(move || project_json_clone.exists())
        .await
        .map_err(|e| miette!("Failed to check file existence: {}", e))?;

    if !exists {
        return Ok(None);
    }

    let project_json_clone = project_json.clone();
    let link_and_settings = tokio::task::spawn_blocking(move || {
        json::read_file(&project_json_clone).map_err(|e| {
            miette!(
                "Failed to read/parse {}: {}",
                project_json_clone.display(),
                e
            )
        })
    })
    .await
    .map_err(|e| miette!("Failed to read file: {}", e))??;

    Ok(Some(link_and_settings))
}

/// Write project link and settings to `.appz/project.json` (sync version)
pub fn write_project_link(cwd: &Path, link_and_settings: &ProjectLinkAndSettings) -> Result<()> {
    let appz_dir = get_appz_directory(cwd);

    // Create .appz directory if it doesn't exist
    fs::create_dir_all(&appz_dir)
        .map_err(|e| miette!("Failed to create {} directory: {}", appz_dir.display(), e))?;

    let project_json = appz_dir.join(PROJECT_JSON);
    json::write_file(&project_json, link_and_settings, true)
        .map_err(|e| miette!("Failed to write {}: {}", project_json.display(), e))?;

    Ok(())
}

/// Write project link and settings to `.appz/project.json` (async version)
pub async fn write_project_link_async(
    cwd: &Path,
    link_and_settings: &ProjectLinkAndSettings,
) -> Result<()> {
    let appz_dir = get_appz_directory(cwd);
    let project_json = appz_dir.join(PROJECT_JSON);

    // Use spawn_blocking for file I/O in async context (following workspace rules)
    let appz_dir_clone = appz_dir.clone();
    let project_json_clone = project_json.clone();
    let link_and_settings_clone = link_and_settings.clone();

    tokio::task::spawn_blocking(move || {
        // Create .appz directory if it doesn't exist
        fs::create_dir_all(&appz_dir_clone).map_err(|e| {
            miette!(
                "Failed to create {} directory: {}",
                appz_dir_clone.display(),
                e
            )
        })?;

        json::write_file(&project_json_clone, &link_and_settings_clone, true)
            .map_err(|e| miette!("Failed to write {}: {}", project_json_clone.display(), e))?;

        Ok::<(), miette::Error>(())
    })
    .await
    .map_err(|e| miette!("Failed to write file: {}", e))??;

    Ok(())
}

/// Remove project link (delete `.appz/project.json`) (sync version)
pub fn remove_project_link(cwd: &Path) -> Result<()> {
    let appz_dir = get_appz_directory(cwd);
    let project_json = appz_dir.join(PROJECT_JSON);

    if project_json.exists() {
        fs::remove_file(&project_json)
            .map_err(|e| miette!("Failed to remove {}: {}", project_json.display(), e))?;
    }

    Ok(())
}

/// Check if project is linked
pub fn is_project_linked(cwd: &Path) -> bool {
    read_project_link(cwd).ok().flatten().is_some()
}

/// Get project context from environment variables
pub fn get_project_context_from_env() -> Option<ProjectLink> {
    let project_id = std::env::var("APPZ_PROJECT_ID").ok()?;
    let team_id = std::env::var("APPZ_TEAM_ID").ok()?;

    Some(ProjectLink {
        project_id,
        team_id,
        project_name: None,
    })
}

/// Warning message shown when project is unavailable
const PROJECT_UNAVAILABLE_MESSAGE: &str =
    "Your Project was either deleted, transferred to a new Team, or you don't have access to it anymore.";

/// Check if an API error indicates that a project was deleted, transferred, or access was revoked
fn is_project_unavailable_error(err: &api::error::ApiError) -> bool {
    use api::error::ApiError;
    matches!(
        err,
        ApiError::NotFound(_) | ApiError::Forbidden(_) | ApiError::Unauthorized(_)
    )
}

/// Fetch a project with team context management
///
/// This helper function:
/// - Sets team context before fetching
/// - Resets team context after fetching (using a guard pattern)
/// - Handles errors gracefully
async fn fetch_project_with_team_context(
    client: &Client,
    project_id: &str,
    team_id: &str,
) -> Result<Option<api::models::Project>, api::error::ApiError> {
    // Save previous team scope so we restore it after (preserves user's --scope when switching teams)
    let previous_team_id = client.get_team_id().await;

    let had_team_context = !team_id.is_empty();
    if had_team_context {
        client.set_team_id(Some(team_id.to_string())).await;
    }

    // Fetch project (errors will be handled by caller)
    let result = client.projects().get(project_id).await;

    // Restore previous team scope (preserves explicit --scope, env, or app switch)
    client.set_team_id(previous_team_id).await;

    result.map(Some).or_else(|e| {
        if is_project_unavailable_error(&e) {
            Ok(None)
        } else {
            Err(e)
        }
    })
}

/// Resolve and validate a project link
///
/// This helper function handles the common pattern of:
/// - Fetching a project with team context
/// - Handling unavailable projects (deleted/transferred/no access)
/// - Showing appropriate warnings
/// - Returning the appropriate ProjectContext or None
async fn resolve_and_validate_link(
    client: &Client,
    link: ProjectLink,
    settings: ProjectSettings,
) -> Result<Option<ProjectContext>> {
    use ui::status;

    match fetch_project_with_team_context(client, &link.project_id, &link.team_id).await {
        Ok(Some(project)) => Ok(Some(ProjectContext {
            link,
            settings,
            project: Some(project),
        })),
        Ok(None) => {
            // Project was deleted, transferred, or access revoked
            let _ = status::warning(PROJECT_UNAVAILABLE_MESSAGE);
            Ok(None)
        }
        Err(_) => {
            // Other errors - still return context but without project
            Ok(Some(ProjectContext {
                link,
                settings,
                project: None,
            }))
        }
    }
}

/// Resolve project context from multiple sources
/// Priority: 1. Env vars, 2. .appz/project.json, 3. None
///
/// This function gracefully handles cases where:
/// - Project was deleted
/// - Project was transferred to a different team
/// - User lost access to the project
///
/// In these cases, it shows a warning message and returns None (not linked),
/// allowing the user to re-link the project.
pub async fn resolve_project_context(
    client: &Client,
    cwd: &Path,
) -> Result<Option<ProjectContext>> {
    // First check environment variables
    if let Some(link) = get_project_context_from_env() {
        return resolve_and_validate_link(client, link, ProjectSettings::default()).await;
    }

    // Then check .appz/project.json (use async version in async context)
    if let Some(link_and_settings) = read_project_link_async(cwd).await? {
        return resolve_and_validate_link(
            client,
            link_and_settings.link,
            link_and_settings.settings,
        )
        .await;
    }

    Ok(None)
}

/// Link a project interactively (used by both link command and ensure_project_link)
///
/// This function handles the interactive linking process:
/// 1. Resolves team ID first (needed for both linking and creating)
/// 2. If project_id is provided, links to that project
/// 3. If no project_id, lists projects and uses first one, or creates new one if none exist
/// 4. Creates and writes the project link
/// 5. Returns the project context
pub async fn link_project_interactive(
    client: &Client,
    cwd: &Path,
    project_id: Option<String>,
    team_id: Option<String>,
) -> Result<ProjectContext> {
    use crate::commands::projects;
    use crate::commands::teams;
    use ui::status;

    // Resolve team ID first (needed for both linking and creating projects)
    let team_id = if let Some(ref team_identifier) = team_id {
        Some(teams::resolve_team_id(client, team_identifier).await?)
    } else {
        // Try to get team from client
        client.get_team_id().await
    };

    let team_id = team_id.ok_or_else(|| {
        miette!("Team ID is required. Use --team flag or set APPZ_TEAM_ID environment variable.")
    })?;

    // Resolve project identifier
    let (project_id, project_obj) = if let Some(ref project_identifier) = project_id {
        // Use provided project identifier
        let resolved_id = projects::resolve_project_id(client, project_identifier).await?;
        let project = client
            .projects()
            .get(&resolved_id)
            .await
            .map_err(|e| miette!("Failed to get project: {}", e))?;
        (resolved_id, project)
    } else {
        // List projects and let user select, or create new one if none exist
        let projects_response = client
            .projects()
            .list(None, None, None)
            .await
            .map_err(|e| miette!("{}", user_friendly_list_projects_error(&e)))?;

        if projects_response.projects.is_empty() {
            // No projects exist - create a new one
            // Use directory name as project slug
            let dir_name = cwd
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project")
                .to_string();

            // Create slug from directory name (simple slugification)
            // Convert to lowercase, replace non-alphanumeric with dashes, collapse multiple dashes
            let mut slug = String::new();
            let mut last_was_dash = false;
            for c in dir_name.chars() {
                if c.is_alphanumeric() {
                    slug.push(c.to_ascii_lowercase());
                    last_was_dash = false;
                } else if !last_was_dash {
                    slug.push('-');
                    last_was_dash = true;
                }
            }

            // Remove leading/trailing dashes
            let slug = slug.trim_matches('-');
            let slug = if slug.is_empty() {
                "project".to_string()
            } else {
                slug.to_string()
            };

            status::info(&format!(
                "No projects found. Creating new project '{}'...",
                slug
            ))
            .map_err(|e| miette!("Failed to display message: {}", e))?;

            let project = client
                .projects()
                .create(slug.clone(), Some(dir_name.clone()), Some(team_id.clone()))
                .await
                .map_err(|e| miette!("Failed to create project: {}", e))?;

            let project_id = project
                .id
                .clone()
                .ok_or_else(|| miette!("Created project but no ID returned"))?;

            status::success(&format!(
                "Created project '{}' (ID: {})",
                project.slug.as_deref().unwrap_or("unknown"),
                project_id
            ))
            .map_err(|e| miette!("Failed to display message: {}", e))?;

            (project_id, project)
        } else {
            // Use the first project (can be enhanced with interactive selection)
            let first_project = projects_response
                .projects
                .first()
                .ok_or_else(|| miette!("No project ID found"))?;

            let project_id = first_project
                .id
                .clone()
                .ok_or_else(|| miette!("Project has no ID"))?;

            // Get full project details
            let project = client
                .projects()
                .get(&project_id)
                .await
                .map_err(|e| miette!("Failed to get project: {}", e))?;

            (project_id, project)
        }
    };

    // Create project link
    let project_link = ProjectLink {
        project_id: project_id.clone(),
        team_id: team_id.clone(),
        project_name: project_obj.name.clone(),
    };

    // Create default settings (can be enhanced with interactive editing)
    let settings = ProjectSettings::default();

    // Write project link
    let link_and_settings = ProjectLinkAndSettings {
        link: project_link.clone(),
        settings: settings.clone(),
    };

    write_project_link_async(cwd, &link_and_settings).await?;

    // Display success message
    let project_name = project_link
        .project_name
        .as_deref()
        .or(project_obj.slug.as_deref())
        .unwrap_or("project");

    status::success(&format!(
        "Linked to project '{}' (ID: {})",
        project_name, project_id
    ))
    .map_err(|e| miette!("Failed to display message: {}", e))?;

    status::info("Created .appz/project.json")
        .map_err(|e| miette!("Failed to display message: {}", e))?;

    // Try to fetch project from API for context
    let project = client.projects().get(&project_id).await.ok();

    Ok(ProjectContext {
        link: project_link,
        settings,
        project,
    })
}

/// Setup and link project - main Vercel-style flow
///
/// This function implements the complete Vercel project setup and linking flow:
/// 1. Initial setup prompt
/// 2. Select organization/team
/// 3. Project selection/creation
/// 4. If linking: link directly
/// 5. If creating: root directory, framework detection, settings editing, create project, link, git connection
#[tracing::instrument(skip(client, cwd))]
pub async fn setup_and_link(
    client: &Client,
    cwd: &Path,
    auto_confirm: bool,
    setup_msg: Option<&str>,
    project_name: Option<&str>,
) -> Result<ProjectContext> {
    use ui::prompt::confirm;

    use ui::status;

    let setup_msg = setup_msg.unwrap_or("Set up");
    let detected_project_name = project_name
        .map(|s| s.to_string())
        .or_else(|| {
            cwd.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "project".to_string());

    // Check if TTY (interactive terminal)
    let is_tty = atty::is(atty::Stream::Stdout);
    if !is_tty && !auto_confirm {
        return Err(miette!(
            "Command requires confirmation. Use --yes flag to confirm in non-interactive mode."
        ));
    }

    // Initial setup prompt
    let humanized_path = humanize_path(cwd);
    let should_start_setup =
        auto_confirm || confirm(&format!("{} \"{}\"?", setup_msg, humanized_path), true)?;

    if !should_start_setup {
        tracing::info!("Canceled. Project not set up.");
        return Err(miette!("Project setup canceled"));
    }

    // Select organization/team
    let org = select_org(
        client,
        "Which scope should contain your project?",
        auto_confirm,
    )
    .await?;

    // Set team ID on client if team was selected
    if org.org_type == OrgType::Team {
        client.set_team_id(Some(org.id.clone())).await;
    } else {
        client.set_team_id(None).await;
    }

    // Project selection/creation
    let project_result = input_project(client, &org, &detected_project_name, auto_confirm).await?;

    match project_result {
        Either::Left(project) => {
            // Linking to existing project
            let project_id = project
                .id
                .as_ref()
                .ok_or_else(|| miette!("Project has no ID"))?;

            // Create project link
            let project_link = ProjectLink {
                project_id: project_id.clone(),
                team_id: org.id.clone(),
                project_name: project.name.clone(),
            };

            let settings = ProjectSettings::default();
            let link_and_settings = ProjectLinkAndSettings {
                link: project_link.clone(),
                settings: settings.clone(),
            };

            write_project_link_async(cwd, &link_and_settings).await?;

            let project_name_display = project
                .name
                .as_ref()
                .or(project.slug.as_ref())
                .unwrap_or(project_id);

            status::success(&format!(
                "Linked to project '{}' (ID: {})",
                project_name_display, project_id
            ))
            .map_err(|e| miette!("Failed to display message: {}", e))?;

            return Ok(ProjectContext {
                link: project_link,
                settings,
                project: Some(project),
            });
        }
        Either::Right(new_project_name) => {
            // Creating new project
            let root_directory = if !auto_confirm {
                input_root_directory(cwd, auto_confirm).await?
            } else {
                None
            };

            // Validate root directory if provided (use spawn_blocking for file I/O)
            if let Some(ref root_dir) = root_directory {
                let root_path = cwd.join(root_dir);
                let root_path_clone = root_path.clone();
                let exists = tokio::task::spawn_blocking(move || root_path_clone.exists())
                    .await
                    .map_err(|e| miette!("Failed to check file existence: {}", e))?;

                if !exists {
                    return Err(miette!(
                        "Root directory does not exist: {}",
                        root_path.display()
                    ));
                }

                let root_path_clone = root_path.clone();
                let is_dir = tokio::task::spawn_blocking(move || root_path_clone.is_dir())
                    .await
                    .map_err(|e| miette!("Failed to check if path is directory: {}", e))?;

                if !is_dir {
                    return Err(miette!(
                        "Root directory is not a directory: {}",
                        root_path.display()
                    ));
                }
            }

            let path_with_root = root_directory
                .as_ref()
                .map(|rd| cwd.join(rd))
                .unwrap_or_else(|| cwd.to_path_buf());

            // Read local config if exists (use async version in async context)
            let local_config = read_config_async(&path_with_root).await.ok().flatten();

            // Detect framework and edit settings
            // Always use framework detection for zero-config projects
            let mut settings =
                edit_project_settings(&path_with_root, local_config.as_ref(), auto_confirm).await?;

            // Additional settings prompt
            if !auto_confirm {
                let change_additional =
                    confirm("Do you want to change additional project settings?", false)?;
                if change_additional {
                    // TODO: Implement additional settings editing (vercelAuth, etc.)
                    // For now, skip
                }
            }

            // Add root directory to settings if provided
            if let Some(rd) = root_directory {
                settings.root_directory = Some(rd);
            }

            // Create project
            let slug = new_project_name
                .to_lowercase()
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' {
                        c
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .trim_matches('-')
                .to_string();

            let team_id_for_create = if org.org_type == OrgType::Team {
                Some(org.id.clone())
            } else {
                None
            };

            let created_project = client
                .projects()
                .create(
                    slug.clone(),
                    Some(new_project_name.clone()),
                    team_id_for_create,
                )
                .await
                .map_err(|e| miette!("Failed to create project: {}", e))?;

            let project_id = created_project
                .id
                .as_ref()
                .ok_or_else(|| miette!("Created project but no ID returned"))?;

            // Create project link
            let project_link = ProjectLink {
                project_id: project_id.clone(),
                team_id: org.id.clone(),
                project_name: Some(new_project_name.clone()),
            };

            let link_and_settings = ProjectLinkAndSettings {
                link: project_link.clone(),
                settings: settings.clone(),
            };

            write_project_link_async(cwd, &link_and_settings).await?;

            status::success(&format!(
                "Linked to project '{}' (ID: {})",
                new_project_name, project_id
            ))
            .map_err(|e| miette!("Failed to display message: {}", e))?;

            // Connect git repository
            connect_git_repository(cwd, auto_confirm).await.ok(); // Ignore errors for git connection

            Ok(ProjectContext {
                link: project_link,
                settings,
                project: Some(created_project),
            })
        }
    }
}

/// Ensure project is linked, automatically linking if necessary
#[tracing::instrument(skip(client, cwd))]
pub async fn ensure_project_link(
    _command_name: &str,
    client: &Client,
    cwd: &Path,
    auto_confirm: bool,
) -> Result<ProjectContext> {
    // First try to resolve existing link
    if let Some(ctx) = resolve_project_context(client, cwd).await? {
        return Ok(ctx);
    }

    // If not linked, use setup_and_link to link automatically
    setup_and_link(client, cwd, auto_confirm, None, None).await
}
