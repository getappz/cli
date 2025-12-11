use crate::auth;
use crate::session::AppzSession;
use miette::Result;
use regex::Regex;
use starbase::AppResult;
use tracing::instrument;
use ui::prompt::text_with_validation;
use ui::status;

/// Validate team slug.
/// Slug must start with a lowercase letter and can contain lowercase letters, numbers, underscores, and hyphens.
fn validate_slug(value: &str) -> Result<Option<String>> {
    let re = Regex::new(r"^[a-z]+[a-z0-9_-]*$")
        .map_err(|e| miette::miette!("Invalid regex pattern: {}", e))?;

    if value.is_empty() {
        return Ok(Some("Team slug cannot be empty".to_string()));
    }

    if !re.is_match(value) {
        return Ok(Some(
            "Team slug must start with a lowercase letter and can only contain lowercase letters, numbers, underscores, and hyphens".to_string()
        ));
    }

    Ok(None)
}

/// Validate team name.
/// Name can contain spaces, letters, numbers, underscores, and hyphens.
fn validate_name(value: &str) -> Result<Option<String>> {
    let re = Regex::new(r"^[ a-zA-Z0-9_-]+$")
        .map_err(|e| miette::miette!("Invalid regex pattern: {}", e))?;

    if value.is_empty() {
        return Ok(Some("Team name cannot be empty".to_string()));
    }

    if !re.is_match(value) {
        return Ok(Some(
            "Team name can only contain letters, numbers, spaces, underscores, and hyphens"
                .to_string(),
        ));
    }

    Ok(None)
}

/// Create a new team with interactive prompts (Vercel-style).
///
/// This command:
/// 1. Prompts for team slug if not provided (with validation)
/// 2. Creates team with slug
/// 3. Prompts for team name (with validation)
/// 4. Updates team with name
/// 5. Sets the newly created team as current team
/// 6. Optionally prompts to invite teammates
///
/// # Arguments
/// * `slug` - Optional team slug (will prompt if not provided)
/// * `name` - Optional team name (will prompt if not provided)
#[instrument(skip_all)]
pub async fn add(session: AppzSession, slug: Option<String>, name: Option<String>) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Step 1: Get or prompt for team slug
    let slug = if let Some(slug) = slug {
        // Validate provided slug
        if let Some(err) = validate_slug(&slug)? {
            return Err(miette::miette!("Invalid slug: {}", err));
        }
        slug
    } else {
        // Interactive prompt for slug
        println!("Pick a team identifier for its URL (e.g.: `appz.dev/acme`)");
        text_with_validation("- Team URL      appz.dev/", None, validate_slug)?
    };

    // Step 2: Create team with slug only
    status::info(&format!("Creating team with slug: {}", slug))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    let mut team = client
        .teams()
        .create(slug.clone(), None)
        .await
        .map_err(|e| miette::miette!("Failed to create team: {}", e))?;

    status::success(&format!("Team created: {}", slug))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    // Step 3: Get or prompt for team name
    let name = if let Some(name) = name {
        // Validate provided name
        if let Some(err) = validate_name(&name)? {
            return Err(miette::miette!("Invalid name: {}", err));
        }
        name
    } else {
        // Interactive prompt for name
        println!("\nPick a display name for your team");
        text_with_validation("- Team Name     ", None, validate_name)?
    };

    // Step 4: Update team with name
    status::info(&format!("Setting team name: {}", name))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    team = client
        .teams()
        .update(&team.id, None, Some(name.clone()), None)
        .await
        .map_err(|e| miette::miette!("Failed to update team name: {}", e))?;

    status::success(&format!("Team name saved: {}", name))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    // Step 5: Update config to set newly created team as current team
    status::info("Saving team configuration...")
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    auth::save_team_id(team.id.clone())
        .map_err(|e| miette::miette!("Failed to save team context: {}", e))?;

    // Set team_id on current API client session
    client.set_team_id(Some(team.id.clone())).await;

    status::success(&format!("Created team '{}' (ID: {})", team.slug, team.id))
        .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

    if let Some(ref team_name) = team.name {
        status::info(&format!("Name: {}", team_name))
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
    }

    // Step 6: Optionally prompt to invite teammates
    println!("\nInvite your teammates! When done, press enter on an empty field");

    // Email validation regex - create once and clone for each validation
    let email_regex = Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$")
        .map_err(|e| miette::miette!("Invalid regex pattern: {}", e))?;

    let mut invited_emails = Vec::new();

    loop {
        // Clone regex for this iteration's closure
        let email_regex = email_regex.clone();
        let validate_email = move |value: &str| -> Result<Option<String>> {
            if value.is_empty() {
                return Ok(None); // Empty is valid (signals end of input)
            }
            if !email_regex.is_match(value.trim()) {
                return Ok(Some("Please enter a valid email address".to_string()));
            }
            Ok(None)
        };

        let email_input = text_with_validation("- Invite User   ", None, validate_email)?;

        let email = email_input.trim().to_string();

        if email.is_empty() {
            break; // User pressed enter on empty field
        }

        // Create invitation
        status::info(&format!("Inviting {}...", email))
            .map_err(|e| miette::miette!("Failed to display message: {}", e))?;

        match client
            .teams()
            .create_invitation(&team.id, email.clone(), None)
            .await
        {
            Ok(_invitation) => {
                status::success(&format!("Invited {}", email))
                    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
                invited_emails.push(email);
            }
            Err(e) => {
                status::info(&format!("Failed to invite {}: {}", email, e))
                    .map_err(|e| miette::miette!("Failed to display message: {}", e))?;
                // Continue to next email instead of failing
            }
        }
    }

    if invited_emails.is_empty() {
        println!("\nNo invitations sent. You can invite teammates later by running: `teams invite <email>`");
    } else {
        println!(
            "\nInvited {} teammate{}",
            invited_emails.len(),
            if invited_emails.len() > 1 { "s" } else { "" }
        );
    }

    Ok(None)
}
