//! WordPress-to-Markdown WASM Plugin
//!
//! Self-contained plugin that directly uses the `wp2md` crate for
//! WordPress WXR export conversion. Filesystem and network operations
//! are performed through PDK host functions provided by the CLI.
//!
//! # Exports
//!
//! - `appz_plugin_handshake()` — security handshake with the host CLI
//! - `appz_plugin_info()` — returns plugin metadata and command definitions
//! - `appz_plugin_execute()` — executes the wp2md command

mod vfs_wasm;

use appz_pdk::prelude::*;
use appz_pdk::security;
use extism_pdk::*;
use std::collections::HashMap;

use vfs_wasm::WasmVfs;
use wp2md::config::{
    parse_frontmatter_fields, DateFolders, SaveImages, Wp2mdConfig,
};
use wp2md::vfs::Wp2mdVfs;

// Declare host functions provided by the CLI
#[host_fn]
extern "ExtismHost" {
    // Plugin filesystem host functions
    fn appz_pfs_read_file(input: Json<PluginFsReadInput>) -> Json<PluginFsReadOutput>;
    fn appz_pfs_write_file(input: Json<PluginFsWriteInput>) -> Json<PluginFsWriteOutput>;
    fn appz_pfs_exists(input: Json<PluginFsReadInput>) -> Json<PluginFsExistsOutput>;
    fn appz_pfs_mkdir(input: Json<PluginFsReadInput>) -> Json<PluginFsWriteOutput>;

    // HTTP download host function
    fn appz_phttp_download(
        input: Json<PluginHttpDownloadInput>,
    ) -> Json<PluginHttpDownloadOutput>;

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
        name: "wp2md".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        commands: vec![PluginCommandDef {
            name: "wp2md".to_string(),
            about: "Convert WordPress export XML or wp-json site to Markdown files".to_string(),
            args: vec![
                PluginArgDef {
                    name: "_positional".to_string(),
                    short: None,
                    long: None,
                    help: Some(
                        "Path to WXR export file or WordPress site URL (e.g. https://mysite.com)"
                            .to_string(),
                    ),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "input".to_string(),
                    short: Some('i'),
                    long: Some("input".to_string()),
                    help: Some(
                        "Path to WXR export file or WordPress site URL (e.g. https://mysite.com)"
                            .to_string(),
                    ),
                    required: false,
                    default: Some("export.xml".to_string()),
                },
                PluginArgDef {
                    name: "output".to_string(),
                    short: Some('o'),
                    long: Some("output".to_string()),
                    help: Some("Output directory for generated files".to_string()),
                    required: false,
                    default: Some("output".to_string()),
                },
                PluginArgDef {
                    name: "post-folders".to_string(),
                    short: None,
                    long: Some("post-folders".to_string()),
                    help: Some("Put each post into its own folder (true/false)".to_string()),
                    required: false,
                    default: Some("true".to_string()),
                },
                PluginArgDef {
                    name: "prefix-date".to_string(),
                    short: None,
                    long: Some("prefix-date".to_string()),
                    help: Some("Add date prefix to posts (true/false)".to_string()),
                    required: false,
                    default: Some("false".to_string()),
                },
                PluginArgDef {
                    name: "date-folders".to_string(),
                    short: None,
                    long: Some("date-folders".to_string()),
                    help: Some("Organize into date folders: none, year, year-month".to_string()),
                    required: false,
                    default: Some("none".to_string()),
                },
                PluginArgDef {
                    name: "save-images".to_string(),
                    short: None,
                    long: Some("save-images".to_string()),
                    help: Some("Which images to save: none, attached, scraped, all".to_string()),
                    required: false,
                    default: Some("all".to_string()),
                },
                PluginArgDef {
                    name: "frontmatter-fields".to_string(),
                    short: None,
                    long: Some("frontmatter-fields".to_string()),
                    help: Some(
                        "Comma-separated frontmatter fields (e.g. title,date,categories,tags)"
                            .to_string(),
                    ),
                    required: false,
                    default: Some("title,date,categories,tags,coverImage,draft".to_string()),
                },
                PluginArgDef {
                    name: "request-delay".to_string(),
                    short: None,
                    long: Some("request-delay".to_string()),
                    help: Some("Delay between image requests in ms".to_string()),
                    required: false,
                    default: Some("500".to_string()),
                },
                PluginArgDef {
                    name: "timezone".to_string(),
                    short: None,
                    long: Some("timezone".to_string()),
                    help: Some("Timezone for post dates (IANA name)".to_string()),
                    required: false,
                    default: Some("utc".to_string()),
                },
                PluginArgDef {
                    name: "include-time".to_string(),
                    short: None,
                    long: Some("include-time".to_string()),
                    help: Some("Include time in frontmatter dates (true/false)".to_string()),
                    required: false,
                    default: Some("false".to_string()),
                },
                PluginArgDef {
                    name: "date-format".to_string(),
                    short: None,
                    long: Some("date-format".to_string()),
                    help: Some("Custom date format string".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "quote-date".to_string(),
                    short: None,
                    long: Some("quote-date".to_string()),
                    help: Some("Wrap dates in quotes (true/false)".to_string()),
                    required: false,
                    default: Some("false".to_string()),
                },
                PluginArgDef {
                    name: "strict-ssl".to_string(),
                    short: None,
                    long: Some("strict-ssl".to_string()),
                    help: Some("Use strict SSL for image downloads (true/false)".to_string()),
                    required: false,
                    default: Some("true".to_string()),
                },
            ],
            subcommands: vec![],
        }],
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
        "wp2md" => handle_wp2md(args, working_dir),
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

fn str_arg(args: &HashMap<String, serde_json::Value>, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn bool_arg(args: &HashMap<String, serde_json::Value>, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| {
        v.as_bool()
            .or_else(|| v.as_str().map(|s| s == "true" || s == "1"))
    })
}

/// Resolve a path to a form suitable for the host ScopedFs.
/// The host expects paths *relative* to working_dir (sandbox root), not absolute.
fn to_vfs_path(working_dir: &str, path: &str) -> String {
    let path = path.trim().trim_start_matches("./");
    if path.is_empty() || path == "." {
        return ".".to_string();
    }
    if path.starts_with('/') {
        // Absolute path: strip working_dir prefix so host gets a relative path
        let wd = working_dir.trim_end_matches('/');
        if let Some(rel) = path.strip_prefix(wd) {
            return rel.trim_start_matches('/').to_string();
        }
    }
    path.to_string()
}

// ── Command Handler ───────────────────────────────────────────────────

fn handle_wp2md(
    args: &HashMap<String, serde_json::Value>,
    working_dir: &str,
) -> FnResult<Json<PluginExecuteOutput>> {
    let vfs = WasmVfs;

    // Resolve input path or URL
    let input_raw = get_positional(args, 0)
        .or_else(|| str_arg(args, "input"))
        .unwrap_or_else(|| "export.xml".to_string());
    let trimmed = input_raw.trim();
    let is_url = trimmed.starts_with("http://")
        || trimmed.starts_with("https://");
    let input_path = if is_url {
        trimmed.to_string()
    } else {
        to_vfs_path(working_dir, &input_raw)
    };

    // Check input file exists (skip for URL — we fetch from network)
    if !is_url && !vfs.exists(&input_path) {
        return Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some(format!(
                "Export file not found: {}\nProvide the path with --input or as the first argument.",
                input_path
            )),
        }));
    }

    // Output path must be relative to working_dir for host ScopedFs
    let output_raw = str_arg(args, "output").unwrap_or_else(|| "output".to_string());
    let output_path = to_vfs_path(working_dir, &output_raw);

    // Parse boolean options
    let post_folders = bool_arg(args, "post-folders").unwrap_or(true);
    let prefix_date = bool_arg(args, "prefix-date").unwrap_or(false);
    let include_time = bool_arg(args, "include-time").unwrap_or(false);
    let quote_date = bool_arg(args, "quote-date").unwrap_or(false);
    let strict_ssl = bool_arg(args, "strict-ssl").unwrap_or(true);

    // Parse enum options
    let date_folders = match str_arg(args, "date-folders")
        .as_deref()
        .unwrap_or("none")
    {
        "year" => DateFolders::Year,
        "year-month" => DateFolders::YearMonth,
        _ => DateFolders::None,
    };

    let save_images = match str_arg(args, "save-images")
        .as_deref()
        .unwrap_or("all")
    {
        "none" => SaveImages::None,
        "attached" => SaveImages::Attached,
        "scraped" => SaveImages::Scraped,
        _ => SaveImages::All,
    };

    // Parse frontmatter fields
    let frontmatter_fields_str = str_arg(args, "frontmatter-fields")
        .unwrap_or_else(|| "title,date,categories,tags,coverImage,draft".to_string());
    let frontmatter_fields = parse_frontmatter_fields(&frontmatter_fields_str);

    // Parse numeric options
    let request_delay_ms = str_arg(args, "request-delay")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(500);

    let timezone = str_arg(args, "timezone").unwrap_or_else(|| "utc".to_string());
    let date_format = str_arg(args, "date-format").filter(|s| !s.is_empty());

    let config = Wp2mdConfig {
        input: input_path,
        output: output_path,
        wpjson_per_page: 100,
        wpjson_include_pages: true,
        post_folders,
        prefix_date,
        date_folders,
        save_images,
        frontmatter_fields,
        request_delay_ms,
        timezone,
        include_time,
        date_format,
        quote_date,
        strict_ssl,
    };

    // Log start
    unsafe {
        let _ = appz_util_info(format!("Converting {} ...", config.input));
    }

    match wp2md::convert_export(&vfs, &config) {
        Ok(result) => {
            let mut parts = Vec::new();
            if result.posts_written > 0 {
                parts.push(format!("{} posts written", result.posts_written));
            }
            if result.posts_skipped > 0 {
                parts.push(format!("{} posts skipped (already exist)", result.posts_skipped));
            }
            if result.images_downloaded > 0 {
                parts.push(format!("{} images downloaded", result.images_downloaded));
            }
            if result.images_skipped > 0 {
                parts.push(format!(
                    "{} images skipped (already exist)",
                    result.images_skipped
                ));
            }

            let message = if parts.is_empty() {
                "No posts found in export file.".to_string()
            } else {
                format!("Done! {}", parts.join(", "))
            };

            Ok(Json(PluginExecuteOutput {
                exit_code: 0,
                message: Some(message),
            }))
        }
        Err(e) => Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some(format!("Conversion failed: {}", e)),
        })),
    }
}
