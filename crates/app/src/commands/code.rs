//! Code operations: search packed output.

use crate::session::AppzSession;
use clap::Subcommand;
use std::path::PathBuf;
use ui::layout;

#[derive(Subcommand, Debug, Clone)]
pub enum CodeCommands {
    /// Search packed code (runs over cached pack from `appz pack`)
    #[command(name = "search")]
    Search {
        /// Search query (literal string by default)
        query: String,
        /// Project directory (default: current) — finds pack for this workdir
        #[arg(long)]
        workdir: Option<PathBuf>,
        /// Treat query as regex
        #[arg(long)]
        regex: bool,
        /// Restrict results to source files matching glob (e.g. "*.rs")
        #[arg(long)]
        glob: Option<String>,
        /// Max results (default: 20)
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// Output as JSON (for piping)
        #[arg(long)]
        json: bool,
        /// Pack to search (content hash from pack ls). Skips interactive picker.
        #[arg(long)]
        pack: Option<String>,
    },
}

pub async fn run(session: AppzSession, command: CodeCommands) -> starbase::AppResult {
    match command {
        CodeCommands::Search {
            query,
            workdir,
            regex,
            glob,
            limit,
            json,
            pack,
        } => {
            let workdir = workdir
                .unwrap_or_else(|| session.working_dir.clone())
                .canonicalize()
                .map_err(|e| miette::miette!("Invalid workdir: {}", e))?;

            let packs = code_mix::get_packs_for_workdir(&workdir)
                .map_err(|e| miette::miette!("{}", e.0))?;

            if packs.is_empty() {
                return Err(miette::miette!(
                    "No packed code found for this project. Run `appz pack` first."
                ));
            }

            let (_, pack_path) = if let Some(ref hash) = pack {
                packs
                    .into_iter()
                    .find(|(e, _)| e.content_hash == *hash)
                    .ok_or_else(|| {
                        miette::miette!("Pack with hash '{}' not found for this project", hash)
                    })?
            } else if packs.len() == 1 {
                packs.into_iter().next().expect("packs not empty")
            } else {
                let options: Vec<String> = packs
                    .iter()
                    .map(|(e, _)| {
                        let workdir_short = e.workdir.as_deref().map(|p| {
                            std::path::Path::new(p)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(p)
                        }).unwrap_or("—");
                        let style = e.style.as_deref().unwrap_or("—");
                        let file_count = e.file_count.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
                        let ws = e.workspace.as_deref().unwrap_or("—");
                        let hash_short = if e.content_hash.len() >= 12 {
                            &e.content_hash[..12]
                        } else {
                            &e.content_hash
                        };
                        format!("{}  {}  {} files  ws:{}  {}", workdir_short, style, file_count, ws, hash_short)
                    })
                    .collect();
                let chosen = inquire::Select::new("Select pack to search:", options)
                    .prompt()
                    .map_err(|e| miette::miette!("Picker: {}", e))?;
                let idx = packs
                    .iter()
                    .position(|(e, _)| {
                        let workdir_short = e.workdir.as_deref().map(|p| {
                            std::path::Path::new(p)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(p)
                        }).unwrap_or("—");
                        let style = e.style.as_deref().unwrap_or("—");
                        let file_count = e.file_count.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
                        let ws = e.workspace.as_deref().unwrap_or("—");
                        let hash_short = if e.content_hash.len() >= 12 {
                            &e.content_hash[..12]
                        } else {
                            &e.content_hash
                        };
                        format!("{}  {}  {} files  ws:{}  {}", workdir_short, style, file_count, ws, hash_short) == chosen
                    })
                    .expect("chosen must exist");
                packs.into_iter().nth(idx).expect("valid idx")
            };

            let req = code_grep::SearchRequest {
                query,
                is_regex: Some(regex),
                file_glob: glob,
                max_results: Some(limit),
            };

            let results = code_mix::search_packed(&req, &pack_path)
                .map_err(|e| miette::miette!("Search failed: {}", e.0))?;

            if json {
                println!("{}", serde_json::to_string_pretty(&results).map_err(|e| miette::miette!("{}", e))?);
            } else {
                for r in &results {
                    let col = r.column.map(|c| format!(":{}", c)).unwrap_or_default();
                    let _ = layout::indented(
                        &format!("{}:{}:{} {}", r.file, r.line, col, r.snippet.trim()),
                        1,
                    );
                }
            }
            return Ok(None);
        }
    }
}
