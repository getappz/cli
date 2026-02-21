//! Select organization/team for project setup
//!
//! Matches Vercel's select-org.ts functionality

use crate::project::{Org, OrgType};
use crate::ClientExt;
use api::Client;
use miette::{miette, Result};
use tracing::instrument;
use ui::prompt::select_with_value;
use ui::status;

/// Select organization/team for project setup
///
/// Prompts user to select which scope (personal account or team) should contain the project.
/// Shows spinner while loading scopes.
#[instrument(skip(client))]
pub async fn select_org(
    client: std::sync::Arc<Client>,
    question: String,
    auto_confirm: bool,
) -> Result<Org> {
    // Show spinner while loading
    status::info("Loading scopes…").map_err(|e| miette!("Failed to display message: {}", e))?;

    // Get user and teams
    let user = client
        .users()
        .get_current()
        .await
        .map_err(|e| miette!("Failed to get user: {}", e))?;

    let teams_response = client
        .teams()
        .list(None, None, None)
        .await
        .map_err(|e| miette!("Failed to list teams: {}", e))?;

    // Build choices: personal account + teams
    let mut choices: Vec<(String, Org)> = Vec::new();

    // Add personal account (user)
    // Use username as ID if id is not available (some API responses don't include id)
    let user_id = user.id.clone().unwrap_or_else(|| user.username.clone());
    let user_org = Org {
        org_type: OrgType::User,
        id: user_id,
        slug: user.username.clone(),
    };
    let user_display_name = user.name.clone().unwrap_or_else(|| user.username.clone());
    choices.push((user_display_name, user_org));

    // Add teams
    for team in teams_response.teams {
        let team_org = Org {
            org_type: OrgType::Team,
            id: team.id.clone(),
            slug: team.slug.clone(),
        };
        let team_display_name = team.name.clone().unwrap_or_else(|| team.slug.clone());
        choices.push((team_display_name, team_org));
    }

    // Get current team ID from client to set default
    let current_team_id = client.get_team_id().await;
    let default_org = current_team_id
        .and_then(|team_id| {
            choices
                .iter()
                .find(|(_, org)| org.id == team_id)
                .map(|(_, org)| org.clone())
        })
        .or_else(|| choices.first().map(|(_, org)| org.clone()));

    if auto_confirm {
        return default_org.ok_or_else(|| miette!("No organization available"));
    }

    // Prompt user to select
    let default_org_ref = default_org.as_ref();
    let selected = select_with_value(question.as_str(), choices, default_org_ref)?;

    Ok(selected)
}
