//! Project transfer commands (Vercel-aligned).
//!
//! - `appz transfer`: Transfer linked project (from CWD).
//! - `appz transfer <project>`: Create a transfer request, returns 24h code.
//! - `appz transfer <project> --to-team <team>`: Direct transfer in one step.
//! - `appz transfer accept <code>`: Accept a transfer into the current team.

use crate::session::AppzSession;
use clap::Subcommand;
use starbase::AppResult;

pub mod accept;
pub mod create;

pub use accept::accept;
pub use create::create;

#[derive(Subcommand, Debug, Clone)]
pub enum TransferCommands {
    /// Create a project transfer request (returns 24h code).
    #[command(alias = "new")]
    Create {
        /// Project name or ID (optional – uses linked project from CWD if omitted)
        #[arg(required = false)]
        project: Option<String>,
        /// Target team for direct transfer (create + accept in one step)
        #[arg(long)]
        to_team: Option<String>,
    },
    /// Accept a transfer into the current team.
    Accept {
        /// Transfer code from the create step
        code: String,
    },
}

/// Route transfer: either create (project given or linked) or accept subcommand.
pub async fn run(
    session: AppzSession,
    command: Option<TransferCommands>,
    project: Option<String>,
    to_team: Option<String>,
) -> AppResult {
    match command {
        Some(TransferCommands::Accept { code }) => accept(session, code).await,
        None => {
            let project = match project {
                Some(p) => p,
                None => {
                    let ctx = session.get_project_context().ok_or_else(|| {
                        miette::miette!(
                            "No project specified and current directory is not linked.\n\
                             Use 'appz transfer <project>', 'appz transfer accept <code>', or run from a linked directory."
                        )
                    })?;
                    ctx.link.project_id.clone()
                }
            };
            create(session, project, to_team).await
        }
        Some(TransferCommands::Create { project, to_team }) => {
            let project = match project {
                Some(p) => p,
                None => {
                    let ctx = session.get_project_context().ok_or_else(|| {
                        miette::miette!(
                            "No project specified and current directory is not linked.\n\
                             Use 'appz transfer create <project>' or run from a linked directory."
                        )
                    })?;
                    ctx.link.project_id.clone()
                }
            };
            create(session, project, to_team).await
        }
    }
}
