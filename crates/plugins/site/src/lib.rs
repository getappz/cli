//! Site WASM Plugin
//!
//! AI-powered website creation, redesign, and cloning.
//! Delegates to the host's site-builder pipeline.

use appz_pdk::prelude::*;
use appz_pdk::security;
use extism_pdk::*;
use std::collections::HashMap;

// Declare host functions provided by the appz CLI
#[host_fn]
extern "ExtismHost" {
    fn appz_psite_run(input: Json<PluginSiteRunInput>) -> Json<PluginSiteRunOutput>;
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
        name: "site".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        commands: vec![PluginCommandDef {
            name: "site".to_string(),
            about: "AI-powered website creation, redesign, and cloning".to_string(),
            args: vec![
                PluginArgDef {
                    name: "output".to_string(),
                    short: Some('o'),
                    long: Some("output".to_string()),
                    help: Some("Output directory for the generated project".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "theme".to_string(),
                    short: None,
                    long: Some("theme".to_string()),
                    help: Some("Theme (nonprofit, corporate, startup, minimal)".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "provider".to_string(),
                    short: None,
                    long: Some("provider".to_string()),
                    help: Some("AI provider (openai, anthropic, ollama, groq, gemini)".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "model".to_string(),
                    short: None,
                    long: Some("model".to_string()),
                    help: Some("AI model for generation".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "transform-content".to_string(),
                    short: None,
                    long: Some("transform-content".to_string()),
                    help: Some("Use AI to rewrite and improve site content".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "no-build".to_string(),
                    short: None,
                    long: Some("no-build".to_string()),
                    help: Some("Skip the build step after generation".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "resume".to_string(),
                    short: None,
                    long: Some("resume".to_string()),
                    help: Some("Resume from last checkpoint".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "dry-run".to_string(),
                    short: None,
                    long: Some("dry-run".to_string()),
                    help: Some("Show plan without executing".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "page".to_string(),
                    short: None,
                    long: Some("page".to_string()),
                    help: Some("Page path(s) to generate (e.g. --page /about)".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "all".to_string(),
                    short: None,
                    long: Some("all".to_string()),
                    help: Some("Generate all remaining pages".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "create".to_string(),
                    short: None,
                    long: Some("create".to_string()),
                    help: Some("This is a create-mode project (for generate-page)".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "url".to_string(),
                    short: None,
                    long: Some("url".to_string()),
                    help: Some("Source URL (for redesign/clone) or project URL (for generate-page)".to_string()),
                    required: false,
                    default: None,
                },
            ],
            subcommands: vec![
                PluginCommandDef {
                    name: "create".to_string(),
                    about: "Create a new website from a natural-language description".to_string(),
                    args: vec![],
                    subcommands: vec![],
                },
                PluginCommandDef {
                    name: "redesign".to_string(),
                    about: "Redesign an existing website with a modern look".to_string(),
                    args: vec![],
                    subcommands: vec![],
                },
                PluginCommandDef {
                    name: "clone".to_string(),
                    about: "Clone an existing website as faithfully as possible".to_string(),
                    args: vec![],
                    subcommands: vec![],
                },
                PluginCommandDef {
                    name: "generate-page".to_string(),
                    about: "Generate specific page(s) for an existing project".to_string(),
                    args: vec![],
                    subcommands: vec![],
                },
            ],
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

    if command != "site" {
        return Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some(format!("Unknown command: {}", command)),
        }));
    }

    let run_input = build_site_run_input(args, working_dir)?;
    let output = unsafe { appz_psite_run(Json(run_input)) }
        .map_err(|e| extism_pdk::Error::msg(e.to_string()))?;

    Ok(Json(PluginExecuteOutput {
        exit_code: output.0.exit_code,
        message: output.0.message,
    }))
}

fn build_site_run_input(
    args: &HashMap<String, serde_json::Value>,
    working_dir: &str,
) -> FnResult<PluginSiteRunInput> {
    let subcommand = get_positional(args, 0).ok_or_else(|| {
        extism_pdk::Error::msg(
            "Missing subcommand. Use: site create <prompt> | site redesign <url> | site clone <url> | site generate-page",
        )
    })?;

    let bool_arg = |key: &str| -> bool {
        args.get(key)
            .and_then(|v| v.as_bool())
            .or_else(|| args.get(key).and_then(|v| v.as_str()).map(|s| s == "true" || s == "1"))
            .unwrap_or(false)
    };

    let str_arg = |key: &str| -> Option<String> {
        args.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
    };

    // For --page, args may have "page" as String (single) or array
    let pages_arg = || -> Option<Vec<String>> {
        if let Some(v) = args.get("page") {
            if let Some(arr) = v.as_array() {
                return Some(
                    arr.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect(),
                );
            }
            if let Some(s) = v.as_str() {
                return Some(vec![s.to_string()]);
            }
        }
        None
    };

    let (url, prompt) = match subcommand.as_str() {
        "create" => {
            let pos = get_positional_vec(args);
            let prompt = if pos.len() > 1 {
                pos[1..].join(" ").trim().to_string()
            } else {
                String::new()
            };
            (None, Some(prompt))
        }
        "redesign" | "clone" => {
            let url = get_positional(args, 1).or_else(|| str_arg("url"));
            (url, None)
        }
        "generate-page" => (str_arg("url"), None),
        _ => (None, None),
    };

    Ok(PluginSiteRunInput {
        working_dir: working_dir.to_string(),
        subcommand,
        url,
        prompt,
        output: str_arg("output"),
        theme: str_arg("theme"),
        provider: str_arg("provider"),
        model: str_arg("model"),
        transform_content: bool_arg("transform-content"),
        no_build: bool_arg("no-build"),
        resume: bool_arg("resume"),
        dry_run: bool_arg("dry-run"),
        pages: pages_arg(),
        all: bool_arg("all"),
        create: bool_arg("create"),
    })
}

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
