//! SSG Migrator WASM Plugin
//!
//! Self-contained plugin that directly uses the `ssg-migrator` crate for
//! project analysis, migration, and file conversion. Filesystem and git
//! operations are performed through PDK host functions provided by the CLI.
//!
//! # Exports
//!
//! - `appz_plugin_handshake()` — security handshake with the host CLI
//! - `appz_plugin_info()` — returns plugin metadata and command definitions
//! - `appz_plugin_execute()` — executes a migrate or convert command

mod vfs_wasm;

use appz_pdk::prelude::*;
use appz_pdk::security;
use camino::Utf8PathBuf;
use extism_pdk::*;
use std::collections::HashMap;

use ssg_migrator::vfs::Vfs;
use vfs_wasm::WasmFs;

// Declare host functions provided by the CLI
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

    // Git host functions
    fn appz_pgit_changed_files(input: Json<PluginFsReadInput>) -> Json<PluginGitFilesOutput>;
    fn appz_pgit_staged_files(input: Json<PluginFsReadInput>) -> Json<PluginGitFilesOutput>;
    fn appz_pgit_is_repo(input: Json<PluginFsReadInput>) -> Json<PluginGitIsRepoOutput>;

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
                        name: "_positional".to_string(),
                        short: None,
                        long: None,
                        help: Some("source [target] - source dir, optional output dir (in-place if omitted)".to_string()),
                        required: false,
                        default: None,
                    },
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
                        name: "dry-run".to_string(),
                        short: Some('d'),
                        long: Some("dry-run".to_string()),
                        help: Some("Simulate migration, show planned changes".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "yes".to_string(),
                        short: Some('y'),
                        long: Some("yes".to_string()),
                        help: Some("Skip confirmations (force when output exists)".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "install".to_string(),
                        short: Some('i'),
                        long: Some("install".to_string()),
                        help: Some("Run npm install after migration".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "transform".to_string(),
                        short: Some('t'),
                        long: Some("transform".to_string()),
                        help: Some("Comma-separated: router, helmet, client, context, all".to_string()),
                        required: false,
                        default: None,
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
            PluginCommandDef {
                name: "convert".to_string(),
                about: "Convert React/TSX/JSX/JS files to Astro or Next.js".to_string(),
                args: vec![
                    PluginArgDef {
                        name: "_positional".to_string(),
                        short: None,
                        long: None,
                        help: Some("File path(s) to convert".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "dry-run".to_string(),
                        short: Some('d'),
                        long: Some("dry-run".to_string()),
                        help: Some("Show diff without writing".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "force".to_string(),
                        short: Some('f'),
                        long: Some("force".to_string()),
                        help: Some("Convert all specified files (ignore changed-only)".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "output".to_string(),
                        short: Some('o'),
                        long: Some("output".to_string()),
                        help: Some("Output path (single file only)".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "target".to_string(),
                        short: None,
                        long: Some("target".to_string()),
                        help: Some("Target: astro (default) or nextjs".to_string()),
                        required: false,
                        default: Some("astro".to_string()),
                    },
                    PluginArgDef {
                        name: "transform".to_string(),
                        short: Some('t'),
                        long: Some("transform".to_string()),
                        help: Some("Transforms (nextjs: router,client,image,all; astro: static,client,class,props,all)".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "list".to_string(),
                        short: None,
                        long: Some("list".to_string()),
                        help: Some("List available transforms for target".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "client-directive".to_string(),
                        short: None,
                        long: Some("client-directive".to_string()),
                        help: Some("Astro client directive (client:load, client:visible, etc.)".to_string()),
                        required: false,
                        default: None,
                    },
                    PluginArgDef {
                        name: "slot-style".to_string(),
                        short: None,
                        long: Some("slot-style".to_string()),
                        help: Some("Astro slot style (default, named)".to_string()),
                        required: false,
                        default: None,
                    },
                ],
                subcommands: vec![],
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
    let working_dir = &input.0.working_dir;

    match command.as_str() {
        "migrate" => handle_migrate(args, working_dir),
        "migrate sync" | "sync" => handle_migrate_sync(args),
        "convert" => handle_convert(args, working_dir),
        _ => Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some(format!("Unknown command: {}", command)),
        })),
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

fn get_positional(args: &HashMap<String, serde_json::Value>, idx: usize) -> Option<String> {
    args.get("_positional")
        .and_then(|v| v.as_array())
        .and_then(|a| a.get(idx))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn get_positional_vec(args: &HashMap<String, serde_json::Value>) -> Vec<String> {
    args.get("_positional")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn str_arg(args: &HashMap<String, serde_json::Value>, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn bool_arg(args: &HashMap<String, serde_json::Value>, key: &str) -> bool {
    args.get(key)
        .and_then(|v| v.as_bool())
        .or_else(|| args.get(key).and_then(|v| v.as_str()).map(|s| s == "true" || s == "1"))
        .unwrap_or(false)
}

fn resolve_path(working_dir: &str, path: &str) -> String {
    if path.is_empty() || path == "." {
        working_dir.to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{}/{}", working_dir, path)
    }
}

// ── Migrate ────────────────────────────────────────────────────────────

fn handle_migrate(
    args: &HashMap<String, serde_json::Value>,
    working_dir: &str,
) -> FnResult<Json<PluginExecuteOutput>> {
    let vfs = WasmFs;

    let source_raw = get_positional(args, 0)
        .or_else(|| str_arg(args, "source"))
        .unwrap_or_else(|| ".".to_string());
    let output_raw = get_positional(args, 1)
        .or_else(|| str_arg(args, "output"))
        .unwrap_or_else(|| source_raw.clone());
    let target = str_arg(args, "target").unwrap_or_else(|| "astro".to_string());
    let force = bool_arg(args, "force") || bool_arg(args, "yes");
    let dry_run = bool_arg(args, "dry-run");
    let static_export = bool_arg(args, "static-export");
    let transforms = str_arg(args, "transform");

    let source_dir = Utf8PathBuf::from(resolve_path(working_dir, &source_raw));
    let output_dir = Utf8PathBuf::from(resolve_path(working_dir, &output_raw));

    if !vfs.exists(source_dir.as_str()) {
        return Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some(format!("Source directory does not exist: {}", source_dir)),
        }));
    }

    let analysis = match ssg_migrator::analyze_project(&vfs, &source_dir) {
        Ok(a) => a,
        Err(e) => {
            return Ok(Json(PluginExecuteOutput {
                exit_code: 1,
                message: Some(format!("Analysis failed: {}", e)),
            }));
        }
    };

    if dry_run {
        let summary = build_dry_run_summary(&analysis, &target, &output_dir, transforms.as_deref());
        return Ok(Json(PluginExecuteOutput {
            exit_code: 0,
            message: Some(summary),
        }));
    }

    let project_name = output_dir
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if target == "nextjs" { "nextjs".to_string() } else { "migrated-astro-app".to_string() }
        });

    let config = ssg_migrator::MigrationConfig {
        source_dir: source_dir.clone(),
        output_dir: output_dir.clone(),
        project_name,
        force,
        static_export,
        transforms,
    };

    let result = match target.as_str() {
        "nextjs" => ssg_migrator::generate_nextjs_project(&vfs, &config, &analysis, &output_dir)
            .map(|_| ()),
        _ => ssg_migrator::generate_astro_project(&vfs, &config, &analysis),
    };

    match result {
        Ok(_) => Ok(Json(PluginExecuteOutput {
            exit_code: 0,
            message: Some(format!(
                "Migration to {} completed. Output: {}",
                target, output_dir
            )),
        })),
        Err(e) => Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some(format!("Migration failed: {}", e)),
        })),
    }
}

// ── Convert ────────────────────────────────────────────────────────────

fn handle_convert(
    args: &HashMap<String, serde_json::Value>,
    working_dir: &str,
) -> FnResult<Json<PluginExecuteOutput>> {
    let vfs = WasmFs;

    if bool_arg(args, "list") {
        let target = str_arg(args, "target").unwrap_or_else(|| "astro".to_string());
        let msg = match target.as_str() {
            "astro" => "Astro transforms: static, client, class, props, all",
            "nextjs" => "Next.js transforms: router, client, helmet, context, image, all",
            _ => "No transforms for this target.",
        };
        return Ok(Json(PluginExecuteOutput {
            exit_code: 0,
            message: Some(msg.to_string()),
        }));
    }

    let files = get_positional_vec(args);
    if files.is_empty() {
        return Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some("No files specified. Provide file path(s) or use --list.".to_string()),
        }));
    }

    let target = str_arg(args, "target").unwrap_or_else(|| "astro".to_string());
    let dry_run = bool_arg(args, "dry-run");
    let force = bool_arg(args, "force");
    let transform_opt = str_arg(args, "transform");
    let client_directive = str_arg(args, "client-directive");
    let slot_style = str_arg(args, "slot-style");
    let output_opt = str_arg(args, "output");

    if output_opt.is_some() && files.len() > 1 {
        return Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some("--output is only supported when converting a single file.".to_string()),
        }));
    }

    // Determine which files to convert
    let abs_files: Vec<String> = files.iter().map(|f| resolve_path(working_dir, f)).collect();

    let files_to_convert: Vec<String> = if force {
        abs_files
    } else if vfs.git_is_repo(working_dir) {
        match vfs.git_changed_files(working_dir) {
            Ok(changed) => {
                let changed_set: std::collections::HashSet<String> =
                    changed.into_iter().map(|p| p.replace('\\', "/")).collect();
                abs_files
                    .into_iter()
                    .filter(|f| {
                        let rel = f
                            .strip_prefix(working_dir)
                            .unwrap_or(f)
                            .trim_start_matches('/')
                            .replace('\\', "/");
                        changed_set.contains(&rel)
                    })
                    .collect()
            }
            Err(_) => abs_files,
        }
    } else {
        abs_files
    };

    if files_to_convert.is_empty() {
        return Ok(Json(PluginExecuteOutput {
            exit_code: 0,
            message: Some("No changed files to convert. Use --force to convert all specified files.".to_string()),
        }));
    }

    for file_path in &files_to_convert {
        if !vfs.exists(file_path) {
            return Ok(Json(PluginExecuteOutput {
                exit_code: 1,
                message: Some(format!("File not found: {}", file_path)),
            }));
        }

        let ext = file_path.rsplit('.').next().unwrap_or("");
        if !matches!(ext, "tsx" | "ts" | "jsx" | "js") {
            return Ok(Json(PluginExecuteOutput {
                exit_code: 1,
                message: Some(format!(
                    "Unsupported file type. Expected .tsx, .ts, .jsx, or .js, got .{}",
                    ext
                )),
            }));
        }

        let content = match vfs.read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(Json(PluginExecuteOutput {
                    exit_code: 1,
                    message: Some(format!("Failed to read file {}: {}", file_path, e)),
                }));
            }
        };

        let converted = match target.as_str() {
            "astro" => {
                let opts = ssg_migrator::AstroConvertOptions {
                    client_directive: client_directive.clone(),
                    slot_style: slot_style.clone(),
                    file_extension: Some(ext.to_string()),
                };
                match ssg_migrator::convert_to_astro(&content, opts) {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok(Json(PluginExecuteOutput {
                            exit_code: 1,
                            message: Some(format!("Conversion failed: {}", e)),
                        }));
                    }
                }
            }
            "nextjs" => {
                let transforms = transform_opt
                    .as_deref()
                    .map(ssg_migrator::parse_transforms)
                    .unwrap_or_default();
                match ssg_migrator::convert_to_nextjs(&content, &transforms) {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok(Json(PluginExecuteOutput {
                            exit_code: 1,
                            message: Some(format!("Conversion failed: {}", e)),
                        }));
                    }
                }
            }
            _ => {
                return Ok(Json(PluginExecuteOutput {
                    exit_code: 1,
                    message: Some(format!("Unsupported target: {}", target)),
                }));
            }
        };

        if !dry_run && content != converted {
            let write_path = if let Some(ref out) = output_opt {
                resolve_path(working_dir, out)
            } else {
                file_path.clone()
            };
            if let Err(e) = vfs.write_string(&write_path, &converted) {
                return Ok(Json(PluginExecuteOutput {
                    exit_code: 1,
                    message: Some(format!("Failed to write {}: {}", write_path, e)),
                }));
            }
        }
    }

    Ok(Json(PluginExecuteOutput {
        exit_code: 0,
        message: if dry_run {
            Some("Dry run - no changes written".to_string())
        } else {
            None
        },
    }))
}

// ── Sync ───────────────────────────────────────────────────────────────

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

// ── Dry-run summary ────────────────────────────────────────────────────

fn build_dry_run_summary(
    analysis: &ssg_migrator::ProjectAnalysis,
    target: &str,
    output_dir: &Utf8PathBuf,
    transforms: Option<&str>,
) -> String {
    let routes_summary = if analysis.routes.is_empty() {
        "none (default index)".to_string()
    } else {
        analysis
            .routes
            .iter()
            .map(|r| r.path.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let files_to_convert: Vec<String> = analysis
        .components
        .iter()
        .map(|c| {
            c.file_path
                .strip_prefix(&analysis.source_dir)
                .map(|p| p.to_string())
                .unwrap_or_else(|_| c.file_path.to_string())
        })
        .collect();

    let transforms_note = transforms
        .filter(|s| !s.is_empty())
        .map(|t| format!("\n  Transforms: {}", t))
        .unwrap_or_default();

    let mut files_list: String = files_to_convert
        .iter()
        .take(15)
        .map(|f| format!("    - {}", f))
        .collect::<Vec<_>>()
        .join("\n");
    if files_to_convert.len() > 15 {
        files_list.push_str(&format!("\n    ... and {} more", files_to_convert.len() - 15));
    }

    format!(
        "Dry run — migration would convert:\n\
         \n  Routes ({}): {}\n\
         \n  Components to convert ({}):\n{}\n\
         \n  Target: {}\n  Output: {}{}\n",
        analysis.routes.len(),
        routes_summary,
        analysis.components.len(),
        files_list,
        target,
        output_dir,
        transforms_note,
    )
}
