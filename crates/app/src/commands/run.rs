use crate::session::AppzSession;
use starbase::AppResult;
use task::{Context, Runner};
use tokio_util::sync::CancellationToken;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn run(session: AppzSession, task: Option<String>, force: bool, changed: bool) -> AppResult {
    let registry = session.get_task_registry();

    // No task specified — list available tasks
    let task = match task {
        Some(t) => t,
        None => {
            let mut visible: Vec<_> = registry.all()
                .filter(|(_, t)| !t.hidden)
                .collect();
            visible.sort_by(|a: &(&String, _), b: &(&String, _)| a.0.cmp(b.0));

            if visible.is_empty() {
                println!("No tasks found. Create .appz/blueprint.yaml with a `tasks` section.");
                return Ok(None);
            }

            println!("{}", ui::theme::style_accent_bold("Available tasks"));
            println!();
            for (name, t) in &visible {
                let desc = t.description.as_deref().unwrap_or("");
                if desc.is_empty() {
                    println!("  {}", name);
                } else {
                    println!(
                        "  {:<24} {}",
                        name,
                        ui::theme::style_muted_italic(desc),
                    );
                }
            }
            println!(
                "\n{}",
                ui::theme::style_muted_italic("Run: appz run <task>")
            );
            return Ok(None);
        }
    };

    let mut ctx = Context::new();
    ctx.set_working_path(session.working_dir.clone());
    let mut r = if session.cli.verbose {
        Runner::new_verbose(&registry)
    } else {
        Runner::new(&registry)
    };

    let cancellation_token = CancellationToken::new();
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

    if cancellation_token.is_cancelled() {
        eprintln!("Cancelled.");
        std::process::exit(130);
    }

    res.map_err(|e| miette::miette!("{}", e))?;

    Ok(None)
}
