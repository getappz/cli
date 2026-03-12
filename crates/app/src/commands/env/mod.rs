//! Environment variables command module (Vercel-aligned).
//!
//! Subcommands: ls | add | rm | pull

use crate::session::AppzSession;
use clap::Subcommand;
use starbase::AppResult;

pub mod add;
pub mod ls;
pub mod pull;
pub mod rm;

pub use add::add;
pub use ls::ls;
pub use pull::{default_env_filename, pull_env};
pub use rm::rm;

#[derive(Subcommand, Debug, Clone)]
pub enum EnvCommands {
    /// List environment variables
    #[command(alias = "list")]
    Ls {
        /// Filter by target (production, preview, development)
        #[arg(long, short)]
        target: Option<String>,
    },
    /// Add an environment variable
    Add {
        /// Variable name
        key: String,
        /// Variable value (prompted if omitted)
        value: Option<String>,
        /// Target environment (production, preview, development)
        #[arg(default_value = "production")]
        target: String,
        /// Overwrite if exists
        #[arg(long)]
        force: bool,
    },
    /// Remove an environment variable
    #[command(alias = "remove")]
    Rm {
        /// Variable name
        key: String,
        /// Target environment (production, preview, development)
        #[arg(long, short)]
        target: Option<String>,
        /// Skip confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Pull env vars and write to .env.local
    Pull {
        /// Output file (default: .env.local)
        #[arg(default_value = ".env.local")]
        filename: String,
        /// Target environment (default: development)
        #[arg(long, short, default_value = "development")]
        target: String,
        /// Skip overwrite confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

/// Route env subcommands to handlers.
pub async fn run(session: AppzSession, command: EnvCommands) -> AppResult {
    match command {
        EnvCommands::Ls { target } => ls(session, target).await,
        EnvCommands::Add {
            key,
            value,
            target,
            force,
        } => add(session, key, value, target, force).await,
        EnvCommands::Rm {
            key,
            target,
            yes,
        } => rm(session, key, target, yes).await,
        EnvCommands::Pull {
            filename,
            target,
            yes,
        } => pull_env(session, filename, target, yes).await,
    }
}
