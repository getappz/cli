//! Shared utilities for deployment-related commands (promote, rollback)

use crate::ClientExt;
use api::models::Deployment;
use api::Client;
use miette::Result;
use std::sync::Arc;
use std::time::Duration;

/// Resolve a deployment by ID or URL
///
/// If the input is a URL, extracts the deployment ID from it.
/// Otherwise, treats it as a deployment ID and fetches the deployment.
pub async fn resolve_deployment_by_id_or_url(
    client: Arc<Client>,
    deployment_id_or_url: String,
) -> Result<Deployment> {
    // If it's a URL, try to extract ID or fetch by URL
    if deployment_id_or_url.starts_with("http://") || deployment_id_or_url.starts_with("https://") {
        // Try to get deployment by URL (API might support this)
        // For now, extract ID from URL pattern
        // URL format: https://xxx.appz.dev or https://xxx-xxx.appz.dev
        // We'll try to get it directly, and if that fails, extract ID
        match client.deployments().get(&deployment_id_or_url).await {
            Ok(deployment) => Ok(deployment),
            Err(_) => {
                // Fallback: try to extract ID from URL
                // This is a simple heuristic - may need adjustment based on actual URL format
                let id = deployment_id_or_url
                    .split('/')
                    .next_back()
                    .unwrap_or(&deployment_id_or_url);
                client
                    .deployments()
                    .get(id)
                    .await
                    .map_err(|e| miette::miette!("Failed to resolve deployment: {}", e))
            }
        }
    } else {
        // Treat as deployment ID
        client
            .deployments()
            .get(&deployment_id_or_url)
            .await
            .map_err(|e| miette::miette!("Failed to get deployment: {}", e))
    }
}

/// Parse a timeout string (e.g., "3m", "30s") into a Duration
///
/// Supports:
/// - `s` for seconds
/// - `m` for minutes
/// - `h` for hours
///
/// Returns None if parsing fails.
pub fn parse_timeout(timeout_str: &str) -> Option<Duration> {
    let timeout_str = timeout_str.trim();

    if timeout_str.is_empty() {
        return None;
    }

    // Extract number and unit
    let (num_str, unit) = if let Some(pos) = timeout_str.rfind(|c: char| c.is_alphabetic()) {
        let (num, unit) = timeout_str.split_at(pos);
        (num, unit)
    } else {
        // No unit specified, assume seconds
        (timeout_str, "s")
    };

    let num: u64 = num_str.parse().ok()?;

    let duration = match unit {
        "s" | "S" => Duration::from_secs(num),
        "m" | "M" => Duration::from_secs(num * 60),
        "h" | "H" => Duration::from_secs(num * 3600),
        _ => return None,
    };

    Some(duration)
}

/// Format elapsed time for display
pub fn format_elapsed_time(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("({}s)", secs)
    } else if secs < 3600 {
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        if remaining_secs == 0 {
            format!("({}m)", mins)
        } else {
            format!("({}m {}s)", mins, remaining_secs)
        }
    } else {
        let hours = secs / 3600;
        let remaining_mins = (secs % 3600) / 60;
        if remaining_mins == 0 {
            format!("({}h)", hours)
        } else {
            format!("({}h {}m)", hours, remaining_mins)
        }
    }
}
