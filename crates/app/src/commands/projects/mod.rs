//! Projects command module - manage projects (Vercel-aligned).
//!
//! Subcommands: list/ls | add | inspect | remove/rm
//! Default: list when no subcommand provided

use crate::session::AppzSession;
use api::Client;
use clap::Subcommand;
use starbase::AppResult;

pub mod add;
pub mod inspect;
pub mod ls;
pub mod rm;

pub use add::add;
pub use inspect::inspect;
pub use ls::ls;
pub use rm::rm;

/// Map API errors to user-friendly messages for list projects.
pub fn user_friendly_list_projects_error(e: &api::ApiError) -> String {
    use api::ApiError;
    match e {
        ApiError::Unauthorized(msg) => format!("Couldn't list projects. {}", msg),
        ApiError::ApiError { code: 503, .. } => {
            "Couldn't list projects. The service is temporarily unavailable. Please try again in a few moments.".to_string()
        }
        ApiError::ApiError { code, message } => {
            format!("Couldn't list projects. {} (code: {})", message, code)
        }
        _ => format!("Couldn't list projects. {}", e),
    }
}

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
        .map_err(|e| miette::miette!("{}", user_friendly_list_projects_error(&e)))?;

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
    #[command(alias = "list")]
    Ls,
    /// Add a new project
    Add {
        /// Project name (used to derive slug, e.g. "my-project" or "My Project")
        name: String,
        /// Team ID or slug (optional)
        #[arg(short = 'T', long)]
        team: Option<String>,
    },
    /// Display information about a project
    Inspect {
        /// Project name or ID (optional – uses linked project from CWD if omitted)
        name: Option<String>,
        /// Skip confirmation when linking
        #[arg(long)]
        yes: bool,
    },
    /// Delete a project
    #[command(alias = "remove")]
    Rm {
        /// Project name or ID to delete
        name: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
        /// Skip removal if project has deployments with active preview/production URL
        #[arg(long, short = 's')]
        safe: bool,
    },
}

/// Derive slug from project name (Vercel-style: lowercase, alphanumeric + hyphen/underscore)
fn name_to_slug(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Route projects subcommands to their respective handlers.
pub async fn run(session: AppzSession, command: ProjectsCommands) -> AppResult {
    match command {
        ProjectsCommands::Ls => ls(session).await,
        ProjectsCommands::Add { name, team } => {
            let slug = name_to_slug(&name);
            let display_name = if slug != name.trim() {
                Some(name.trim().to_string())
            } else {
                None
            };
            add(session, slug, display_name, team).await
        }
        ProjectsCommands::Inspect { name, yes } => inspect(session, name, yes).await,
        ProjectsCommands::Rm { name, yes, safe } => rm(session, name, yes, safe).await,
    }
}
