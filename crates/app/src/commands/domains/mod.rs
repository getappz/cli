//! Domains command module - manage domains.
//!
//! This module provides commands for:
//! - Listing domains
//! - Deleting domains

use crate::session::AppzSession;
use clap::Subcommand;
use starbase::AppResult;

pub mod ls;
pub mod rm;

pub use ls::ls;
pub use rm::rm;

#[derive(Subcommand, Debug, Clone)]
pub enum DomainsCommands {
    /// List all domains
    Ls,
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
        DomainsCommands::Rm { domain, yes } => rm(session, domain, yes).await,
    }
}
