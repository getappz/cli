//! Inspect deployment details (Vercel-aligned).

use crate::commands::deployment_utils::resolve_deployment_by_id_or_url;
use api::models::Deployment;
use miette::Result;
use starbase::AppResult;
use tracing::instrument;
use ui::{format, layout};

#[instrument(skip_all)]
pub async fn inspect(
    session: crate::session::AppzSession,
    deployment: String,
    json_output: bool,
) -> AppResult {
    let client = session.get_api_client();

    if client.get_token().await.is_none() {
        return Err(miette::miette!("Not logged in. Run 'appz login' or set APPZ_TOKEN.").into());
    }

    let dep = resolve_deployment_by_id_or_url(&client, &deployment).await?;

    if json_output {
        let out = serde_json::to_string_pretty(&dep).map_err(|e| miette::miette!("{}", e))?;
        println!("{}", out);
        return Ok(None);
    }

    let _ = layout::blank_line();
    let _ = layout::section_title("Deployment");

    let status_str = dep.status.as_deref().unwrap_or("unknown");
    let status_badge = format::status_badge(status_str);

    println!("  ID:      {}", dep.id);
    println!("  Status:  {}", status_badge);
    println!("  URL:     {}", dep.url.as_deref().unwrap_or("N/A"));
    println!("  Project: {}", dep.projectId.as_deref().unwrap_or("N/A"));
    println!("  Created: {}", format::timestamp_auto(dep.createdAt));

    Ok(None)
}
