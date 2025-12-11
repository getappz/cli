//! Teams command module - manage teams and invitations.
//!
//! This module provides commands for:
//! - Listing teams
//! - Creating teams
//! - Inviting users to teams

use crate::session::AppzSession;
use api::Client;
use clap::Subcommand;
use starbase::AppResult;

pub mod add;
pub mod invite;
pub mod list;
pub mod rm;
pub mod switch;

pub use add::add;
pub use invite::invite;
pub use list::list;
pub use rm::rm;
pub use switch::switch;

/// Resolve team identifier (ID or slug) to team ID.
///
/// This function first attempts to fetch the team directly by ID.
/// If that fails, it lists all teams and searches by slug.
///
/// # Arguments
/// * `client` - The API client to use for requests
/// * `team_identifier` - Team ID or slug to resolve
///
/// # Returns
/// The team ID if found, otherwise an error
pub async fn resolve_team_id(
    client: &Client,
    team_identifier: &str,
) -> Result<String, miette::Error> {
    // Try to get team directly by ID first (faster if it's already an ID)
    if let Ok(team) = client.teams().get(team_identifier).await {
        return Ok(team.id);
    }

    // If that fails, list teams and find by slug
    let teams_response = client
        .teams()
        .list(None, None, None)
        .await
        .map_err(|e| miette::miette!("Failed to list teams: {}", e))?;

    for team in teams_response.teams {
        if team.id == team_identifier || team.slug == team_identifier {
            return Ok(team.id);
        }
    }

    Err(miette::miette!("Team '{}' not found", team_identifier))
}

#[derive(Subcommand, Debug, Clone)]
pub enum TeamsCommands {
    /// List all teams
    Ls,
    /// Create a new team
    Add {
        /// Team slug (unique identifier, optional - will prompt if not provided)
        slug: Option<String>,
        /// Team name (optional - will prompt if not provided)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Invite a user to the current team
    Invite {
        /// Email address of the user to invite
        email: String,
        /// Role ID (optional)
        #[arg(short, long)]
        role: Option<i64>,
    },
    /// Delete a team
    Rm {
        /// Team ID or slug
        team: String,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Switch to a different team
    Switch {
        /// Team ID or slug
        team: String,
    },
}

/// Route teams subcommands to their respective handlers.
pub async fn run(session: AppzSession, command: TeamsCommands) -> AppResult {
    match command {
        TeamsCommands::Ls => list(session).await,
        TeamsCommands::Add { slug, name } => add(session, slug, name).await,
        TeamsCommands::Invite { email, role } => invite(session, email, role).await,
        TeamsCommands::Rm { team, yes } => rm(session, team, yes).await,
        TeamsCommands::Switch { team } => switch(session, team).await,
    }
}
