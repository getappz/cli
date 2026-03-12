//! Generate a website from a natural-language prompt (AI, via Appz API).

use crate::session::AppzSession;
use appz_studio::{parse_and_apply, scaffold};
use miette::{miette, Result};
use starbase::AppResult;
use std::path::PathBuf;
use tracing::instrument;
use ui::progress::SpinnerHandle;
use ui::status;

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
        let _ = status::error("Prompt cannot be empty");
        return Err(miette!("Prompt cannot be empty"));
    }

    let _ = status::info(&format!(
        "Output directory: {}",
        output_dir.display()
    ));
    let _ = status::info("Scaffolding Vite + React + Tailwind project...");
    scaffold(&output_dir).map_err(|e| miette!("{}", e))?;
    let _ = status::success("Scaffold complete");

    let client = session.get_api_client();
    let spinner = SpinnerHandle::new("Generating code from your description...");
    let response = client
        .gen()
        .generate(prompt_str, model)
        .await;
    let response = match response {
        Ok(r) => {
            spinner.finish_with_message("Generation complete");
            r
        }
        Err(e) => {
            spinner.finish();
            let _ = status::error(&format!("API generation failed: {}", e));
            return Err(miette!("API generation failed: {}", e));
        }
    };

    if response.trim().is_empty() {
        let _ = status::error("API returned empty response");
        return Err(miette!("API returned empty response"));
    }

    let _ = status::info("Applying generated files and installing dependencies...");
    if let Err(e) = parse_and_apply(response.trim(), &output_dir).await {
        let _ = status::error(&format!("Failed to apply generated code: {}", e));
        return Err(miette!("{}", e));
    }
    let _ = status::success("Applied successfully");

    let _ = status::success_with_spacing(&format!(
        "Generated project at {}",
        output_dir.display()
    ));
    println!("  Location: {}", output_dir.display());
    println!("\nNext steps:");
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
