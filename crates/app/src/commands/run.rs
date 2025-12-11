use crate::session::AppzSession;
use starbase::AppResult;
use task::{Context, Runner};
use tokio_util::sync::CancellationToken;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn run(session: AppzSession, task: String, force: bool, changed: bool) -> AppResult {
    let registry = session.get_task_registry();
    let mut ctx = Context::new();
    ctx.set_working_path(session.working_dir.clone());
    let mut r = if session.cli.verbose {
        Runner::new_verbose(&registry)
    } else {
        Runner::new(&registry)
    };

    // Create cancellation token for graceful shutdown on Ctrl+C
    let cancellation_token = CancellationToken::new();

    // Spawn task to listen for Ctrl+C and cancel execution
    let cancellation_token_clone = cancellation_token.clone();
    let verbose = session.cli.verbose;
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("Failed to listen for Ctrl+C: {}", e);
            return;
        }
        if verbose {
            eprintln!("\nReceived Ctrl+C, cancelling tasks...");
        }
        cancellation_token_clone.cancel();
    });

    let res = r
        .invoke_async(
            &task,
            &mut ctx,
            Some(cancellation_token.clone()),
            force,
            changed,
        )
        .await;

    // If cancelled via Ctrl+C, exit cleanly with code 130
    if cancellation_token.is_cancelled() {
        eprintln!("Cancelled.");
        std::process::exit(130);
    }

    // Otherwise propagate any error
    res.map_err(|e| miette::miette!("{}", e))?;

    Ok(None)
}
