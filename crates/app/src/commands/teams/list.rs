use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;
use ui::{format, pagination, table};

/// List all teams the user has access to.
///
/// Displays teams in a table format with ID, slug, name, and creation timestamp.
/// The currently active team (if set) is highlighted in the output.
#[instrument(skip_all)]
pub async fn list(session: AppzSession) -> AppResult {
    // Get authenticated API client from session
    let client = session.get_api_client();

    // Get current team_id to highlight active team
    let current_team_id = client.get_team_id().await;

    // List teams
    let teams_response = client
        .teams()
        .list(None, None, None)
        .await
        .map_err(|e| miette::miette!("Failed to list teams: {}", e))?;

    if teams_response.teams.is_empty() {
        ui::empty::display(
            "No teams found",
            Some("Try creating a team first with 'teams add'"),
        )?;
        return Ok(None);
    }

    // Sort teams: active team first, then others
    let mut teams = teams_response.teams;
    if let Some(ref active_team_id) = current_team_id {
        teams.sort_by(|a, b| {
            let a_is_active = &a.id == active_team_id;
            let b_is_active = &b.id == active_team_id;
            match (a_is_active, b_is_active) {
                (true, false) => std::cmp::Ordering::Less, // a comes first
                (false, true) => std::cmp::Ordering::Greater, // b comes first
                _ => std::cmp::Ordering::Equal,            // maintain relative order
            }
        });
    }

    // Prepare table data
    let headers = vec!["ID", "Slug", "Name", "Created"];
    let mut rows = Vec::new();

    for team in &teams {
        let team_id = team.id.clone();
        let slug = team.slug.clone();
        let name = team.name.as_deref().unwrap_or("N/A");

        // Highlight current active team
        let display_id = if current_team_id
            .as_ref()
            .map(|id| id == &team_id)
            .unwrap_or(false)
        {
            format!("{} (active)", team_id)
        } else {
            team_id
        };

        // Format timestamp (handle both seconds and milliseconds)
        let created = team
            .createdAt
            .map(format::timestamp_auto)
            .unwrap_or_else(|| "N/A".to_string());

        rows.push(vec![display_id, slug, name.to_string(), created]);
    }

    // Display table with professional formatting
    table::display(&headers, &rows, Some("Teams"))?;

    // Display pagination info if available
    if let Some(ref pag) = teams_response.pagination {
        let pagination_info = pagination::PaginationInfo::new(pag.count, pag.next, pag.prev);
        pagination::display(&pagination_info)?;
    }

    Ok(None)
}
