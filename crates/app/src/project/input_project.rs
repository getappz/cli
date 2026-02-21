//! Input project selection/creation
//!
//! Matches Vercel's input-project.ts functionality

use crate::commands::projects::resolve_project_id;
use crate::project::Org;
use crate::ClientExt;
use api::models::Project;
use api::Client;
use miette::{miette, Result};
use tracing::instrument;
use ui::prompt::confirm;
use ui::status;

/// Slugify a string (simple implementation)
fn slugify(input: &str) -> String {
    input
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
        .to_string()
}

/// Input project selection or creation
///
/// Returns either:
/// - `Project` if linking to existing project
/// - `String` (project name) if creating new project
#[instrument(skip(client, org))]
pub async fn input_project(
    client: std::sync::Arc<Client>,
    org: &Org,
    detected_project_name: String,
    auto_confirm: bool,
) -> Result<Either<Project, String>> {
    let slugified_name = slugify(&detected_project_name);

    // Show spinner while searching
    status::info("Searching for existing projects…")
        .map_err(|e| miette!("Failed to display message: {}", e))?;

    // Attempt to auto-detect a project to link
    let mut detected_project: Option<Project> = None;

    // Try to find project by name or slug
    if let Ok(project_id) = resolve_project_id(client.clone(), detected_project_name.clone()).await {
        if let Ok(project) = client.projects().get(&project_id).await {
            // Check if project belongs to this org
            if project.teamId.as_ref() == Some(&org.id)
                || (org.org_type == crate::project::OrgType::User && project.teamId.is_none())
            {
                detected_project = Some(project);
            }
        }
    }

    // Also try slugified name
    if detected_project.is_none() && slugified_name != detected_project_name {
        if let Ok(project_id) = resolve_project_id(client.clone(), slugified_name.clone()).await {
            if let Ok(project) = client.projects().get(&project_id).await {
                if project.teamId.as_ref() == Some(&org.id)
                    || (org.org_type == crate::project::OrgType::User && project.teamId.is_none())
                {
                    detected_project = Some(project);
                }
            }
        }
    }

    if auto_confirm {
        if let Some(project) = detected_project {
            return Ok(Either::Left(project));
        }
        return Ok(Either::Right(detected_project_name));
    }

    let should_link_project: bool;

    if let Some(ref project) = detected_project {
        // Auto-detected a project to link
        let project_name = project
            .name
            .as_deref()
            .or(project.slug.as_deref())
            .or(project.id.as_deref())
            .unwrap_or("unknown");

        let link_message = format!(
            "Found project \"{}/{}\". Link to it?",
            org.slug, project_name
        );

        if confirm(&link_message, true)? {
            return Ok(Either::Left(project.clone()));
        }

        // User doesn't want to link the auto-detected project
        should_link_project = confirm("Link to different existing project?", true)?;
    } else {
        // Did not auto-detect a project to link
        should_link_project = confirm("Link to existing project?", false)?;
    }

    if should_link_project {
        // User wants to link a project
        // Use a loop with manual validation since async validators are complex
        let project_name = loop {
            use ui::prompt::prompt;
            let input = prompt("What's the name of your existing project?", None)?;

            if input.is_empty() {
                tracing::warn!("Project name cannot be empty");
                continue;
            }

            match resolve_project_id(client.clone(), input.clone()).await {
                Ok(project_id) => {
                    match client.projects().get(&project_id).await {
                        Ok(project) => {
                            // Check if project belongs to this org
                            if project.teamId.as_ref() == Some(&org.id)
                                || (org.org_type == crate::project::OrgType::User
                                    && project.teamId.is_none())
                            {
                                break input;
                            } else {
                                tracing::warn!("Project not found in this scope");
                                continue;
                            }
                        }
                        Err(e) => {
                            tracing::debug!("Project not found: {}", e);
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Project not found: {}", e);
                    continue;
                }
            }
        };

        let project_id = resolve_project_id(client.clone(), project_name.clone()).await?;
        let project = client
            .projects()
            .get(&project_id)
            .await
            .map_err(|e| miette!("Failed to get project: {}", e))?;

        return Ok(Either::Left(project));
    }

    // User wants to create a new project
    // Use a loop with manual validation
    let project_name = loop {
        use ui::prompt::prompt;
        let default_name: Option<&str> = if detected_project.is_none() {
            Some(&slugified_name)
        } else {
            None
        };
        let input = prompt("What's your project's name?", default_name)?;

        if input.is_empty() {
            tracing::warn!("Project name cannot be empty");
            continue;
        }

        // Check if project already exists
        match resolve_project_id(client.clone(), input.clone()).await {
            Ok(_) => {
                tracing::warn!("Project already exists");
                continue;
            }
            Err(_) => {
                // Project doesn't exist, which is good
                break input;
            }
        }
    };

    Ok(Either::Right(project_name))
}

/// Either type for project selection result
#[derive(Debug, Clone)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}
