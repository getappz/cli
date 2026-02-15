//! SSG Migrator WASM Plugin
//!
//! This is the thin WASM entry point for the SSG Migrator. It exports the
//! required plugin interface functions and delegates to the `ssg-migrator`
//! crate (compiled with the `wasm` feature flag).
//!
//! # Exports
//!
//! - `appz_plugin_handshake()` — security handshake with the host CLI
//! - `appz_plugin_info()` — returns plugin metadata and command definitions
//! - `appz_plugin_execute()` — executes a migrate command

use appz_pdk::prelude::*;
use appz_pdk::security;
use extism_pdk::*;

// Declare host functions that will be available from the appz CLI
#[host_fn]
extern "ExtismHost" {
    // Plugin filesystem host functions
    fn appz_pfs_read_file(input: Json<PluginFsReadInput>) -> Json<PluginFsReadOutput>;
    fn appz_pfs_write_file(input: Json<PluginFsWriteInput>) -> Json<PluginFsWriteOutput>;
    fn appz_pfs_walk_dir(input: Json<PluginFsWalkInput>) -> Json<PluginFsWalkOutput>;
    fn appz_pfs_exists(input: Json<PluginFsReadInput>) -> Json<PluginFsExistsOutput>;
    fn appz_pfs_is_file(input: Json<PluginFsReadInput>) -> Json<PluginFsExistsOutput>;
    fn appz_pfs_is_dir(input: Json<PluginFsReadInput>) -> Json<PluginFsExistsOutput>;
    fn appz_pfs_mkdir(input: Json<PluginFsReadInput>) -> Json<PluginFsWriteOutput>;
    fn appz_pfs_copy(input: Json<PluginFsCopyInput>) -> Json<PluginFsWriteOutput>;
    fn appz_pfs_remove(input: Json<PluginFsReadInput>) -> Json<PluginFsWriteOutput>;
    fn appz_pfs_list_dir(input: Json<PluginFsReadInput>) -> Json<PluginFsWriteOutput>;
    fn appz_pfs_read_json(input: Json<PluginFsReadInput>) -> Json<PluginFsJsonOutput>;
    fn appz_pfs_write_json(input: Json<PluginFsJsonInput>) -> Json<PluginFsWriteOutput>;

    // Git host functions
    fn appz_pgit_changed_files(input: Json<PluginFsReadInput>) -> Json<PluginGitFilesOutput>;
    fn appz_pgit_staged_files(input: Json<PluginFsReadInput>) -> Json<PluginGitFilesOutput>;
    fn appz_pgit_is_repo(input: Json<PluginFsReadInput>) -> Json<PluginGitIsRepoOutput>;

    // Sandbox exec host functions
    fn appz_psandbox_exec(input: Json<PluginSandboxExecInput>) -> Json<PluginSandboxExecOutput>;

    // AST host functions
    fn appz_past_transform(input: Json<PluginAstTransformInput>) -> Json<PluginAstTransformOutput>;
    fn appz_past_parse_jsx(input: Json<PluginAstParseInput>) -> Json<PluginAstParseOutput>;

    // Logging
    fn appz_util_info(message: String) -> Json<appz_pdk::VoidResponse>;
}

/// Security handshake — called immediately after the plugin is loaded.
#[plugin_fn]
pub fn appz_plugin_handshake(
    input: Json<PluginHandshakeChallenge>,
) -> FnResult<Json<PluginHandshakeResponse>> {
    let response = security::compute_handshake(&input.0);
    Ok(Json(response))
}

/// Plugin metadata — called after handshake succeeds.
#[plugin_fn]
pub fn appz_plugin_info() -> FnResult<Json<PluginInfo>> {
    Ok(Json(PluginInfo {
        name: "ssg-migrator".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        commands: vec![
            PluginCommandDef {
                name: "migrate".to_string(),
                about: "Migrate React SPA to Astro or Next.js".to_string(),
                args: vec![
                    PluginArgDef {
                        name: "source".to_string(),
                        short: Some('s'),
                        long: Some("source".to_string()),
                        help: Some("Source React SPA directory".to_string()),
                        required: false,
                        default: Some(".".to_string()),
                    },
                    PluginArgDef {
                        name: "output".to_string(),
                        short: Some('o'),
                        long: Some("output".to_string()),
                        help: Some("Output directory for migrated project".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "target".to_string(),
                        short: None,
                        long: Some("target".to_string()),
                        help: Some("Migration target: astro (default) or nextjs".to_string()),
                        required: false,
                        default: Some("astro".to_string()),
                    },
                    PluginArgDef {
                        name: "force".to_string(),
                        short: None,
                        long: Some("force".to_string()),
                        help: Some("Overwrite existing output directory".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "static-export".to_string(),
                        short: None,
                        long: Some("static-export".to_string()),
                        help: Some("Generate static-export Next.js project".to_string()),
                        required: false,
                        default: None,
                    },
                ],
                subcommands: vec![
                    PluginCommandDef {
                        name: "sync".to_string(),
                        about: "Sync changes between original and migrated projects".to_string(),
                        args: vec![
                            PluginArgDef {
                                name: "source".to_string(),
                                short: Some('s'),
                                long: Some("source".to_string()),
                                help: Some("Original React SPA directory".to_string()),
                                required: false,
                                default: None,
                            },
                            PluginArgDef {
                                name: "output".to_string(),
                                short: Some('o'),
                                long: Some("output".to_string()),
                                help: Some("Migrated project directory".to_string()),
                                required: false,
                                default: None,
                            },
                            PluginArgDef {
                                name: "mode".to_string(),
                                short: None,
                                long: Some("mode".to_string()),
                                help: Some("Sync mode: forward or backward".to_string()),
                                required: false,
                                default: Some("forward".to_string()),
                            },
                        ],
                        subcommands: vec![],
                    },
                ],
            },
        ],
    }))
}

/// Execute a plugin command.
#[plugin_fn]
pub fn appz_plugin_execute(
    input: Json<PluginExecuteInput>,
) -> FnResult<Json<PluginExecuteOutput>> {
    let command = &input.0.command;
    let args = &input.0.args;

    match command.as_str() {
        "migrate" => handle_migrate(args),
        "migrate sync" | "sync" => handle_migrate_sync(args),
        _ => Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some(format!("Unknown command: {}", command)),
        })),
    }
}

fn handle_migrate(
    args: &std::collections::HashMap<String, serde_json::Value>,
) -> FnResult<Json<PluginExecuteOutput>> {
    let target = args
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("astro");

    let source = args
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or(".");

    unsafe {
        let _ = appz_util_info(format!(
            "Starting {} migration from '{}'...",
            target, source
        ));
    }

    // The actual migration logic would call into ssg-migrator (wasm feature)
    // which uses host functions for file I/O, git, and AST operations.
    // For now, return a placeholder indicating the plugin is working.
    Ok(Json(PluginExecuteOutput {
        exit_code: 0,
        message: Some(format!(
            "Migration to {} initiated from '{}'. Plugin system operational.",
            target, source
        )),
    }))
}

fn handle_migrate_sync(
    args: &std::collections::HashMap<String, serde_json::Value>,
) -> FnResult<Json<PluginExecuteOutput>> {
    let mode = args
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("forward");

    unsafe {
        let _ = appz_util_info(format!("Starting {} sync...", mode));
    }

    Ok(Json(PluginExecuteOutput {
        exit_code: 0,
        message: Some(format!("Sync ({} mode) initiated. Plugin system operational.", mode)),
    }))
}
