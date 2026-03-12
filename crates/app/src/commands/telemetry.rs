//! Telemetry command - enable or disable telemetry collection (Vercel-aligned).
//!
//! Commands: status | enable | disable
//!
//! Respects APPZ_TELEMETRY_DISABLED env var (like VERCEL_TELEMETRY_DISABLED).

use crate::config::{load_config, save_config, TelemetryConfig, UserConfig};
use clap::Subcommand;
use starbase::AppResult;
use tracing::instrument;

const LEARN_MORE_URL: &str = "https://docs.appz.dev/cli/about-telemetry";

#[derive(Subcommand, Debug, Clone)]
pub enum TelemetryCommands {
    /// Show whether telemetry collection is enabled or disabled
    Status,
    /// Enable telemetry collection
    Enable,
    /// Disable telemetry collection
    Disable,
}

/// Check if telemetry is disabled via environment variable (takes precedence over config).
fn env_disabled() -> bool {
    match std::env::var("APPZ_TELEMETRY_DISABLED") {
        Ok(v) => matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => false,
    }
}

/// Show telemetry status
#[instrument(skip_all)]
pub async fn status() -> AppResult {
    let config = load_config().unwrap_or_else(|_| UserConfig::new());

    // Env var overrides config
    let enabled = !env_disabled() && config.telemetry_enabled();

    println!();
    if enabled {
        println!("> Telemetry status: Enabled");
        println!();
        println!("> You have opted in to Appz CLI telemetry");
    } else {
        println!("> Telemetry status: Disabled");
        println!();
        println!("> You have opted out of Appz CLI telemetry");
        println!("> No data will be collected from your machine");
    }
    println!();
    println!("Learn more: {LEARN_MORE_URL}");
    println!();

    Ok(None)
}

/// Enable telemetry collection
#[instrument(skip_all)]
pub async fn enable() -> AppResult {
    let mut config = load_config().unwrap_or_else(|_| UserConfig::new());
    config.telemetry = Some(TelemetryConfig { enabled: true });
    save_config(&config).map_err(|e| miette::miette!("Failed to save config: {}", e))?;

    // Show status after change
    status().await
}

/// Disable telemetry collection
#[instrument(skip_all)]
pub async fn disable() -> AppResult {
    let mut config = load_config().unwrap_or_else(|_| UserConfig::new());
    config.telemetry = Some(TelemetryConfig { enabled: false });
    save_config(&config).map_err(|e| miette::miette!("Failed to save config: {}", e))?;

    // Show status after change
    status().await
}

/// Run the telemetry subcommand
pub async fn run(command: TelemetryCommands) -> AppResult {
    match command {
        TelemetryCommands::Status => status().await,
        TelemetryCommands::Enable => enable().await,
        TelemetryCommands::Disable => disable().await,
    }
}
