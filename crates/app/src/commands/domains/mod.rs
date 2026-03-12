//! Domains command module - manage domains (Vercel parity).
//!
//! This module provides commands for:
//! - Listing domains
//! - Adding a domain to a project
//! - Deleting domains

use crate::session::AppzSession;
use clap::Subcommand;
use starbase::AppResult;

pub mod add;
pub mod ls;
pub mod rm;

pub use add::add;
pub use ls::ls;
pub use rm::rm;

#[derive(Subcommand, Debug, Clone)]
pub enum DomainsCommands {
    /// List all domains
    Ls,
    /// Add a domain to a project (Vercel parity: domains add <domain> [project])
    Add {
        /// Domain name (do not include https://)
        domain: String,
        /// Project name or ID (uses linked project if omitted)
        project: Option<String>,
        /// Target environment (production, preview, staging) [default: production]
        #[arg(long, short = 'e')]
        environment: Option<String>,
        /// Team ID or slug
        #[arg(short = 'T', long)]
        team: Option<String>,
    },
    /// Delete a domain
    Rm {
        /// Domain name
        domain: String,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

/// Route domains subcommands to their respective handlers.
pub async fn run(session: AppzSession, command: DomainsCommands) -> AppResult {
    match command {
        DomainsCommands::Ls => ls(session).await,
        DomainsCommands::Add {
            domain,
            project,
            environment,
            team,
        } => {
            let env = environment.unwrap_or_else(|| "production".to_string());
            add(session, domain, project, env, team).await
        }
        DomainsCommands::Rm { domain, yes } => rm(session, domain, yes).await,
    }
}
