//! Top-level remove command - Vercel-aligned.
//!
//! Removes deployments (by URL/ID) or projects (by name). Also supports aliases,
//! domains, and teams. Auto-detects resource type from the identifier.

use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::prompt::confirm;
use ui::status;

/// Resource types that can be removed
#[derive(Debug, Clone)]
enum ResourceType {
    Deployment(String), // deployment_id
    Project(String),     // project_id
    Alias(i64),         // alias_id
    Domain(String),     // domain_name
    Team(String),       // team_id
}

/// Remove multiple resources by automatically detecting their types.
///
/// Vercel-aligned: deployments by URL/ID, project by name removes entire project.
/// # Arguments
/// * `resources` - Deployment URLs/IDs, project names, alias IDs, domain names, team IDs
/// * `yes` - Skip confirmation prompt if true
/// * `safe` - When removing project: skip if it has deployments with active preview/production URL
#[instrument(skip_all)]
pub async fn remove(
    session: AppzSession,
    resources: Vec<String>,
    yes: bool,
    safe: bool,
) -> AppResult {
    if resources.is_empty() {
        return Err(miette::miette!(
            "At least one resource identifier is required"
        ));
    }

    let client = session.get_api_client();

    // Group resources by type
    let mut deployments = Vec::new();
    let mut projects = Vec::new();
    let mut aliases = Vec::new();
    let mut domains = Vec::new();
    let mut teams = Vec::new();
    let mut not_found = Vec::new();

    // Detect resource types (deployment first for URL/UUID, then project, alias, domain, team)
    for resource_id in &resources {
        match detect_resource_type(&client, resource_id).await {
            Ok(ResourceType::Deployment(id)) => {
                deployments.push((id.clone(), resource_id.clone()))
            }
            Ok(ResourceType::Project(id)) => projects.push((id.clone(), resource_id.clone())),
            Ok(ResourceType::Alias(id)) => aliases.push((id, resource_id.clone())),
            Ok(ResourceType::Domain(name)) => domains.push((name.clone(), resource_id.clone())),
            Ok(ResourceType::Team(id)) => teams.push((id.clone(), resource_id.clone())),
            Err(_) => not_found.push(resource_id.clone()),
        }
    }

    // --safe: filter out projects that have deployments with active preview/production URL
    if safe && !projects.is_empty() {
        let team_id = client.get_team_id().await;
        let mut filtered = Vec::new();
        for (project_id, orig) in projects {
            let list = client
                .deployments()
                .list(
                    Some(project_id.clone()),
                    Some(50),
                    None,
                    None,
                    team_id.clone(),
                )
                .await;
            let has_active = list
                .as_ref()
                .map(|r| {
                    r.deployments.iter().any(|d| {
                        d.url.as_ref()
                            .is_some_and(|u| !u.is_empty())
                    })
                })
                .unwrap_or(false);
            if !has_active {
                filtered.push((project_id, orig));
            }
        }
        projects = filtered;
    }

    // Report not found resources
    if !not_found.is_empty() {
        eprintln!("Warning: Could not find the following resources:");
        for id in &not_found {
            eprintln!("  - {}", id);
        }
    }

    // Check if we have any resources to remove
    if deployments.is_empty()
        && projects.is_empty()
        && aliases.is_empty()
        && domains.is_empty()
        && teams.is_empty()
    {
        return Err(miette::miette!("No valid resources found to remove"));
    }

    // Show confirmation prompt unless --yes flag is set
    if !yes {
        println!("\nThe following resources will be permanently removed:");

        if !deployments.is_empty() {
            println!("\n  Deployments ({}):", deployments.len());
            for (id, _) in &deployments {
                println!("    - {}", id);
            }
        }

        if !projects.is_empty() {
            println!("\n  Projects ({}):", projects.len());
            for (id, _) in &projects {
                println!("    - {}", id);
            }
        }

        if !aliases.is_empty() {
            println!("\n  Aliases ({}):", aliases.len());
            for (id, _) in &aliases {
                println!("    - {}", id);
            }
        }

        if !domains.is_empty() {
            println!("\n  Domains ({}):", domains.len());
            for (name, _) in &domains {
                println!("    - {}", name);
            }
        }

        if !teams.is_empty() {
            println!("\n  Teams ({}):", teams.len());
            for (id, _) in &teams {
                println!("    - {}", id);
            }
        }

        println!();

        if !confirm("Are you sure? This action cannot be undone.", false)? {
            println!("Canceled");
            return Ok(None);
        }
    }

    // Remove all resources
    let mut errors = Vec::new();
    let mut success_count = 0;

    // Remove deployments (Vercel primary use case)
    for (deployment_id, original_id) in deployments {
        match client.deployments().delete(&deployment_id).await {
            Ok(_) => {
                success_count += 1;
                tracing::info!("Deleted deployment: {}", deployment_id);
            }
            Err(e) => {
                errors.push(format!("Failed to delete deployment '{}': {}", original_id, e));
            }
        }
    }

    // Remove projects
    for (project_id, original_id) in projects {
        match client.projects().delete(&project_id).await {
            Ok(_) => {
                success_count += 1;
                tracing::info!("Deleted project: {}", project_id);
            }
            Err(e) => {
                errors.push(format!("Failed to delete project '{}': {}", original_id, e));
            }
        }
    }

    // Remove aliases
    for (alias_id, original_id) in aliases {
        match client.aliases().delete(&alias_id.to_string()).await {
            Ok(_) => {
                success_count += 1;
                tracing::info!("Deleted alias: {}", alias_id);
            }
            Err(e) => {
                errors.push(format!("Failed to delete alias '{}': {}", original_id, e));
            }
        }
    }

    // Remove domains
    let team_id = client.get_team_id().await;
    for (domain_name, _) in domains {
        match client.domains().delete(&domain_name, team_id.clone()).await {
            Ok(_) => {
                success_count += 1;
                tracing::info!("Deleted domain: {}", domain_name);
            }
            Err(e) => {
                errors.push(format!("Failed to delete domain '{}': {}", domain_name, e));
            }
        }
    }

    // Remove teams
    for (team_id, original_id) in teams {
        match client.teams().delete(&team_id).await {
            Ok(_) => {
                success_count += 1;
                tracing::info!("Deleted team: {}", team_id);
            }
            Err(e) => {
                errors.push(format!("Failed to delete team '{}': {}", original_id, e));
            }
        }
    }

    // Display results
    if success_count > 0 {
        status::success(&format!(
            "Successfully removed {} resource(s)",
            success_count
        ))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
    }

    if !errors.is_empty() {
        eprintln!("\nErrors occurred while removing some resources:");
        for error in &errors {
            eprintln!("  {}", error);
        }
        return Err(miette::miette!("Some resources could not be removed"));
    }

    Ok(None)
}

/// Check if identifier looks like a deployment URL or UUID (try deployment first).
fn looks_like_deployment(identifier: &str) -> bool {
    identifier.starts_with("http://") || identifier.starts_with("https://")
        || uuid_like(identifier)
}

/// Simple UUID format check (8-4-4-4-12 hex).
fn uuid_like(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts[0].len() == 8
        && parts[1].len() == 4
        && parts[2].len() == 4
        && parts[3].len() == 4
        && parts[4].len() == 12
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Detect the resource type by attempting to resolve the identifier.
///
/// Vercel-aligned: deployment (URL/UUID) first, then project → alias → domain → team
async fn detect_resource_type(
    client: &api::Client,
    identifier: &str,
) -> Result<ResourceType, miette::Error> {
    // Try deployment first when identifier looks like URL or UUID (Vercel primary use case)
    if looks_like_deployment(identifier) {
        if let Ok(deployment) = client.deployments().get(identifier).await {
            return Ok(ResourceType::Deployment(deployment.id));
        }
    }

    // Try as project
    if let Ok(project) = client.projects().get(identifier).await {
        if let Some(id) = project.id {
            return Ok(ResourceType::Project(id));
        }
    }

    // Try as alias
    if let Ok(alias) = client.aliases().get(identifier).await {
        return Ok(ResourceType::Alias(alias.id));
    }

    // Try as domain - check domains list first before assuming
    // (We'll check domains list later in the function)

    // Try as team
    if let Ok(team) = client.teams().get(identifier).await {
        return Ok(ResourceType::Team(team.id));
    }

    // If all attempts fail, try listing to find by slug/name
    // Try projects list
    if let Ok(projects_response) = client.projects().list(None, None, None).await {
        for project in projects_response.projects {
            let matches_id = project
                .id
                .as_ref()
                .map(|id| id == identifier)
                .unwrap_or(false);
            let matches_slug = project
                .slug
                .as_ref()
                .map(|slug| slug == identifier)
                .unwrap_or(false);
            if matches_id || matches_slug {
                if let Some(id) = project.id {
                    return Ok(ResourceType::Project(id));
                }
            }
        }
    }

    // Try aliases list
    if let Ok(aliases_response) = client.aliases().list(None, None, None, None, None).await {
        for alias in aliases_response.aliases {
            if alias.id.to_string() == identifier || alias.alias == identifier {
                return Ok(ResourceType::Alias(alias.id));
            }
        }
    }

    // Try teams list
    if let Ok(teams_response) = client.teams().list(None, None, None).await {
        for team in teams_response.teams {
            if team.id == identifier || team.slug == identifier {
                return Ok(ResourceType::Team(team.id));
            }
        }
    }

    // Try domains list (check before team list since domain names might match team slugs)
    if let Ok(domains_response) = client.domains().list(None, None, None, None).await {
        for domain in domains_response.domains {
            if domain.name == identifier {
                return Ok(ResourceType::Domain(domain.name));
            }
        }
    }

    Err(miette::miette!("Resource '{}' not found", identifier))
}
