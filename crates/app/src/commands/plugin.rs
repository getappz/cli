//! Plugin management commands: list installed plugins and check for updates.

use crate::session::AppzSession;
use clap::Subcommand;
use starbase::AppResult;

#[derive(Subcommand, Debug, Clone)]
pub enum PluginCommands {
    /// List available plugins and their installed versions
    List,
    /// Check for and download plugin updates
    Update {
        /// Update only the named plugin (e.g. `ssg-migrator`)
        name: Option<String>,
    },
}

pub async fn run(session: AppzSession, command: PluginCommands) -> AppResult {
    match command {
        PluginCommands::List => list_plugins(session).await,
        PluginCommands::Update { name } => update_plugins(session, name).await,
    }
}

async fn list_plugins(session: AppzSession) -> AppResult {
    let api_client = session.get_api_client();
    let plugin_mgr = plugin_manager::PluginManager::new(api_client)
        .await
        .map_err(|e| miette::miette!("{}", e))?;

    let plugins = plugin_mgr.installed_plugins();

    if plugins.is_empty() {
        println!("No plugins found in the manifest.");
        return Ok(None);
    }

    println!("{:<20} {:<12} {:<12} STATUS", "PLUGIN", "INSTALLED", "LATEST");
    println!("{}", "─".repeat(60));

    for p in &plugins {
        let status = if p.version == "—" {
            "not installed"
        } else if p.has_update {
            "update available"
        } else {
            "up to date"
        };
        println!(
            "{:<20} {:<12} {:<12} {}",
            p.name, p.version, p.manifest_version, status
        );
    }

    Ok(None)
}

async fn update_plugins(session: AppzSession, name: Option<String>) -> AppResult {
    let api_client = session.get_api_client();
    let mut plugin_mgr = plugin_manager::PluginManager::new(api_client)
        .await
        .map_err(|e| miette::miette!("{}", e))?;

    // Force-refresh the manifest to get the very latest versions
    println!("Refreshing plugin manifest...");
    plugin_mgr
        .force_refresh_manifest()
        .await
        .map_err(|e| miette::miette!("{}", e))?;

    let target = name.as_deref();
    let results = plugin_mgr
        .update_plugins(target)
        .await
        .map_err(|e| miette::miette!("{}", e))?;

    if results.is_empty() {
        println!("No plugins to update.");
        return Ok(None);
    }

    let mut any_updated = false;
    for p in &results {
        if p.has_update {
            println!(
                "  Updated {} {} -> {}",
                p.name, p.version, p.manifest_version
            );
            any_updated = true;
        } else {
            println!("  {} {} (up to date)", p.name, p.manifest_version);
        }
    }

    if !any_updated {
        println!("\nAll plugins are up to date.");
    } else {
        println!("\nDone.");
    }

    // Record that we checked so the periodic hint resets
    if let Ok(cache_dir) = plugin_manager::PluginManager::default_cache_dir() {
        let checker = plugin_manager::update_check::PluginUpdateChecker::new(&cache_dir);
        checker.record_hint_shown();
    }

    Ok(None)
}
