//! Semantic code search (index and search) via Repomix + Qdrant.

use crate::session::AppzSession;
use clap::Subcommand;
use std::path::PathBuf;
use std::sync::Arc;
use ui::layout;
use ui::progress::SpinnerHandle;
use ui::status;

#[derive(Subcommand, Debug, Clone)]
pub enum CodeCommands {
    /// Index the project with Repomix and Qdrant
    Index {
        /// Working directory (default: current)
        #[arg(long)]
        workdir: Option<PathBuf>,
        /// Re-index even if already indexed
        #[arg(long)]
        force: bool,
    },
    /// Semantic search over indexed code
    Search {
        /// Search query
        query: String,
        /// Working directory (default: current)
        #[arg(long)]
        workdir: Option<PathBuf>,
        /// Maximum number of results (default: 10)
        #[arg(long)]
        limit: Option<usize>,
    },
}

pub async fn run(session: AppzSession, command: CodeCommands) -> starbase::AppResult {
    let workdir = match &command {
        CodeCommands::Index { workdir, .. } | CodeCommands::Search { workdir, .. } => workdir
            .clone()
            .unwrap_or_else(|| session.working_dir.clone()),
    };

    let workdir = workdir
        .canonicalize()
        .map_err(|e| miette::miette!("Invalid workdir: {}", e))?;

    match command {
        CodeCommands::Index { force, .. } => {
            let _ = status::info(&format!("Indexing codebase at {}", workdir.display()));
            let _ = layout::blank_line();

            let spinner = Arc::new(SpinnerHandle::new("Preparing..."));
            let on_step: code_search::IndexProgressCallback = Box::new({
                let s = spinner.clone();
                move |msg| {
                    s.set_message(msg);
                }
            });

            let result = code_search::index(&workdir, force, Some(&on_step))
                .await
                .map_err(|e| {
                    spinner.finish();
                    miette::miette!("Index failed: {}", e.0)
                })?;

            spinner.finish_with_message("Index complete");

            let _ = layout::blank_line();
            let _ = status::success(&format!(
                "Indexed {} files, {} chunks",
                result.indexed_files, result.chunks
            ));
            let _ = layout::indented(&format!("Collection: {}", result.collection), 1);
            let _ = layout::indented("Run `appz code search <query>` to search.", 1);
        }
        CodeCommands::Search { query, limit, .. } => {
            let spinner = SpinnerHandle::new("Searching...");

            let results = code_search::search(&workdir, &query, limit)
                .await
                .map_err(|e| {
                    spinner.finish();
                    miette::miette!("Search failed: {}", e.0)
                })?;

            spinner.finish_with_message("Search complete");

            if results.is_empty() {
                let _ = layout::blank_line();
                let _ = status::info("No results found.");
                let _ = layout::indented("Run `appz code index` first to index the codebase.", 1);
            } else {
                let _ = layout::blank_line();
                let _ = layout::section_title(&format!("Search results for \"{}\"", query));
                let _ = layout::blank_line();

                for (i, r) in results.iter().enumerate() {
                    let _ = layout::subsection_title(&format!(
                        "{}. {} (relevance: {:.2})",
                        i + 1,
                        r.path,
                        r.score
                    ));
                    let _ = layout::indented(&format!("Lines ~{}+", r.line_start), 1);
                    let _ = layout::separator();
                    // Indent content for readability
                    for line in r.content.lines().take(15) {
                        let _ = layout::indented(line, 1);
                    }
                    if r.content.lines().count() > 15 {
                        let _ = layout::indented("...", 1);
                    }
                    let _ = layout::blank_line();
                }
            }
        }
    }

    Ok(None)
}
