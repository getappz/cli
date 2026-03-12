//! External subcommand handler for downloadable plugins.
//!
//! When the user invokes a command that isn't in the hardcoded `Commands` enum,
//! clap routes it here via `external_subcommand`. This handler:
//!
//! 1. Looks up the command in the plugin manifest
//! 2. Ensures the user is entitled to use it
//! 3. Downloads and verifies the plugin if not cached
//! 4. Creates a sandboxed execution environment
//! 5. Loads the plugin with full security handshake
//! 6. Executes the plugin command

use crate::session::AppzSession;
use crate::wasm::PluginRunner;
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};
use starbase::AppResult;
use std::sync::Arc;

/// Run an external plugin command.
pub async fn run(session: AppzSession, args: Vec<String>) -> AppResult {
    let command_name = args.first().cloned().unwrap_or_default();

    if command_name.is_empty() {
        return Err(miette::miette!("No command specified"));
    }

    tracing::debug!("Looking up plugin command: {}", &command_name);

    // Create the plugin manager
    let api_client = session.get_api_client();
    let plugin_mgr = plugin_manager::PluginManager::new(api_client)
        .await
        .map_err(|e| miette::miette!("{}", e))?;

    // Ensure plugin is available (download, verify, entitlement check)
    let verified = plugin_mgr
        .ensure_plugin(&command_name)
        .await
        .map_err(|e| miette::miette!("{}", e))?;

    tracing::info!(
        "Loading plugin '{}' v{} for command '{}'",
        verified.name,
        verified.entry.version,
        &command_name
    );

    // Create sandbox scoped to the user's working directory (clone to avoid capturing &Path in Send future)
    let working_dir = session.working_dir.clone();
    let sandbox_config = SandboxConfig::new(working_dir.clone())
        .with_settings(SandboxSettings {
            auto_install_mise: false,
            quiet: !session.cli.verbose,
            ..Default::default()
        });

    let sandbox = create_sandbox(sandbox_config)
        .await
        .map_err(|e| miette::miette!("Failed to create sandbox: {}", e))?;

    let scoped_fs = Arc::new(
        sandbox::ScopedFs::new(&working_dir)
            .map_err(|e| miette::miette!("Failed to create scoped filesystem: {}", e))?,
    );

    // Create PluginRunner with sandbox
    let sandbox_arc: Arc<dyn sandbox::SandboxProvider> = Arc::from(sandbox);
    let mut runner = PluginRunner::new(
        sandbox_arc,
        scoped_fs,
    );

    // Load plugin (handshake + info)
    runner.load_verified_plugin(&verified.wasm_path, &verified.name)?;

    // Execute the command
    let remaining_args: Vec<String> = args.into_iter().skip(1).collect();
    let working_dir = session.working_dir.to_string_lossy().to_string();

    runner.execute_command(&command_name, &remaining_args, &working_dir)?;

    // After successful execution, show a periodic update hint (7-day TTL).
    if let Ok(cache_dir) = plugin_manager::PluginManager::default_cache_dir() {
        let checker = plugin_manager::update_check::PluginUpdateChecker::new(&cache_dir);
        if checker.should_show_hint() {
            plugin_manager::update_check::PluginUpdateChecker::show_hint();
            checker.record_hint_shown();
        }
    }

    Ok(None)
}
