//! Projects command module - manage projects.
//!
//! This module provides commands for:
//! - Listing projects
//! - Creating projects
//! - Deleting projects

use crate::session::AppzSession;
use api::Client;
use clap::Subcommand;
use starbase::AppResult;

pub mod add;
pub mod ls;
pub mod rm;

pub use add::add;
pub use ls::ls;
pub use rm::rm;

/// Resolve project identifier (ID or slug) to project ID.
///
/// This function first attempts to fetch the project directly by ID.
/// If that fails, it lists all projects and searches by slug.
///
/// # Arguments
/// * `client` - The API client to use for requests
/// * `project_identifier` - Project ID or slug to resolve
///
/// # Returns
/// The project ID if found, otherwise an error
pub async fn resolve_project_id(
    client: &Client,
    project_identifier: &str,
) -> Result<String, miette::Error> {
    // Try to get project directly by ID first (faster if it's already an ID)
    if let Ok(project) = client.projects().get(project_identifier).await {
        if let Some(id) = project.id {
            return Ok(id);
        }
    }

    // If that fails, list projects and find by slug
    let projects_response = client
        .projects()
        .list(None, None, None)
        .await
        .map_err(|e| miette::miette!("Failed to list projects: {}", e))?;

    for project in projects_response.projects {
        let matches_id = project
            .id
            .as_ref()
            .map(|id| id == project_identifier)
            .unwrap_or(false);
        let matches_slug = project
            .slug
            .as_ref()
            .map(|slug| slug == project_identifier)
            .unwrap_or(false);

        if matches_id || matches_slug {
            if let Some(id) = project.id {
                return Ok(id);
            }
        }
    }

    Err(miette::miette!(
        "Project '{}' not found",
        project_identifier
    ))
}

#[derive(Subcommand, Debug, Clone)]
pub enum ProjectsCommands {
    /// List all projects
    Ls,
    /// Create a new project
    Add {
        /// Project slug (unique identifier)
        slug: String,
        /// Project name (optional)
        #[arg(short, long)]
        name: Option<String>,
        /// Team ID or slug (optional)
        #[arg(short, long)]
        team: Option<String>,
    },
    /// Delete a project
    Rm {
        /// Project ID or slug
        project: String,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

/// Route projects subcommands to their respective handlers.
pub async fn run(session: AppzSession, command: ProjectsCommands) -> AppResult {
    match command {
        ProjectsCommands::Ls => ls(session).await,
        ProjectsCommands::Add { slug, name, team } => add(session, slug, name, team).await,
        ProjectsCommands::Rm { project, yes } => rm(session, project, yes).await,
    }
}
