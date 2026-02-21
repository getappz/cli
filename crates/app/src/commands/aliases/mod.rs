//! Aliases command module - manage aliases.
//!
//! This module provides commands for:
//! - Listing aliases
//! - Setting (creating) aliases
//! - Deleting aliases

use crate::session::AppzSession;
use crate::ClientExt;
use api::Client;
use clap::Subcommand;
use starbase::AppResult;

pub mod ls;
pub mod rm;
pub mod set;

pub use ls::ls;
pub use rm::rm;
pub use set::set;

/// Resolve alias identifier (ID or alias string) to alias ID.
///
/// This function first attempts to fetch the alias directly by ID or alias string.
/// If that fails, it lists all aliases and searches by alias string.
///
/// # Arguments
/// * `client` - The API client to use for requests
/// * `alias_identifier` - Alias ID or alias string to resolve
///
/// # Returns
/// The alias ID if found, otherwise an error
pub async fn resolve_alias_id(
    client: std::sync::Arc<Client>,
    alias_identifier: String,
) -> Result<i64, miette::Error> {
    // Try to get alias directly by ID or alias string first (faster if it's already an ID)
    if let Ok(alias) = client.aliases().get(&alias_identifier).await {
        return Ok(alias.id);
    }

    // If that fails, list aliases and find by alias string
    let aliases_response = client
        .aliases()
        .list(None, None, None, None, None)
        .await
        .map_err(|e| miette::miette!("Failed to list aliases: {}", e))?;

    for alias in aliases_response.aliases {
        if alias.id.to_string() == alias_identifier || alias.alias == alias_identifier {
            return Ok(alias.id);
        }
    }

    Err(miette::miette!("Alias '{}' not found", alias_identifier))
}

#[derive(Subcommand, Debug, Clone)]
pub enum AliasesCommands {
    /// List all aliases
    Ls {
        /// Maximum number of aliases to return (default: 20, max: 100)
        #[arg(long, value_name = "N")]
        limit: Option<i64>,
    },
    /// Assign a custom domain to a deployment
    Set {
        /// Deployment URL or ID
        deployment: String,
        /// Custom domain (do not include https://)
        #[arg(value_name = "custom-domain")]
        custom_domain: String,
    },
    /// Delete an alias
    Rm {
        /// Alias ID or alias string
        alias: String,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

/// Route aliases subcommands to their respective handlers.
pub async fn run(session: AppzSession, command: AliasesCommands) -> AppResult {
    match command {
        AliasesCommands::Ls { limit } => ls(session, limit).await,
        AliasesCommands::Set {
            deployment,
            custom_domain,
        } => set(session, deployment, custom_domain).await,
        AliasesCommands::Rm { alias, yes } => rm(session, alias, yes).await,
    }
}
