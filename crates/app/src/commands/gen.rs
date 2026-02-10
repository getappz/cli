//! Generate a website from a natural-language prompt (AI, via Appz API).

use crate::session::AppzSession;
use appz_studio::{parse_and_apply, scaffold};
use miette::{miette, Result};
use starbase::AppResult;
use std::path::PathBuf;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn run(
    session: AppzSession,
    prompt: Vec<String>,
    output: Option<PathBuf>,
    name: Option<String>,
    model: Option<String>,
) -> AppResult {
    let working_dir = session.working_dir.clone();
    let output_dir = resolve_output_dir(working_dir, output, name)?;
    let prompt_str = prompt.join(" ").trim().to_string();
    if prompt_str.is_empty() {
        return Err(miette!("Prompt cannot be empty").into());
    }

    tracing::info!("Scaffolding at {}", output_dir.display());
    scaffold(&output_dir).map_err(|e| miette!("{}", e))?;

    let client = session.get_api_client();
    tracing::info!("Requesting AI generation from API");
    let response = client
        .gen()
        .generate(prompt_str, model)
        .await
        .map_err(|e| miette!("API generation failed: {}", e))?;

    if response.trim().is_empty() {
        return Err(miette!("API returned empty response").into());
    }

    tracing::info!("Applying generated code");
    parse_and_apply(response.trim(), &output_dir)
        .await
        .map_err(|e| miette!("{}", e))?;

    println!("Generated project at: {}", output_dir.display());
    println!("Next steps:");
    println!("  cd {}", output_dir.display());
    println!("  npm run dev");

    Ok(None)
}

fn resolve_output_dir(
    working_dir: PathBuf,
    output: Option<PathBuf>,
    name: Option<String>,
) -> Result<PathBuf> {
    if let Some(o) = output {
        return Ok(o);
    }
    let dir_name = name.unwrap_or_else(|| "gen-output".to_string());
    Ok(working_dir.join(dir_name))
}
