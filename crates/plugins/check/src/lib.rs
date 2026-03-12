//! Check WASM Plugin
//!
//! Universal code checker with auto-fix and AI-assisted repair.
//! Free tier: basic linting (Biome, tsc, Ruff, Clippy, etc.)
//! Pro tier: AI-assisted fixes (--ai-fix)

use appz_pdk::prelude::*;
use appz_pdk::security;
use extism_pdk::*;
use std::collections::HashMap;

// Declare host functions provided by the appz CLI
#[host_fn]
extern "ExtismHost" {
    // Check run - delegates to host's full checker implementation
    fn appz_pcheck_run(input: Json<PluginCheckRunInput>) -> Json<PluginCheckRunOutput>;

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
        name: "check".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        commands: vec![PluginCommandDef {
            name: "check".to_string(),
            about: "Check code for errors, lint issues, formatting, and secrets".to_string(),
            args: vec![
                PluginArgDef {
                    name: "fix".to_string(),
                    short: None,
                    long: Some("fix".to_string()),
                    help: Some("Auto-fix safe issues".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "ai-fix".to_string(),
                    short: None,
                    long: Some("ai-fix".to_string()),
                    help: Some("Use AI to suggest fixes for complex errors (Pro)".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "strict".to_string(),
                    short: None,
                    long: Some("strict".to_string()),
                    help: Some("Treat warnings as errors".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "changed".to_string(),
                    short: None,
                    long: Some("changed".to_string()),
                    help: Some("Only check files changed since last commit".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "staged".to_string(),
                    short: None,
                    long: Some("staged".to_string()),
                    help: Some("Only check git-staged files".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "format".to_string(),
                    short: None,
                    long: Some("format".to_string()),
                    help: Some("Check and fix formatting".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "json".to_string(),
                    short: None,
                    long: Some("json".to_string()),
                    help: Some("Output results as JSON (for CI/CD)".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "checker".to_string(),
                    short: None,
                    long: Some("checker".to_string()),
                    help: Some("Specific checker to run (biome, tsc, ruff, etc.)".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "jobs".to_string(),
                    short: Some('j'),
                    long: Some("jobs".to_string()),
                    help: Some("Number of parallel jobs".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "init".to_string(),
                    short: None,
                    long: Some("init".to_string()),
                    help: Some("Initialize checker config files".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "max-attempts".to_string(),
                    short: None,
                    long: Some("max-attempts".to_string()),
                    help: Some("Maximum AI fix retry attempts".to_string()),
                    required: false,
                    default: Some("3".to_string()),
                },
                PluginArgDef {
                    name: "ai-verify".to_string(),
                    short: None,
                    long: Some("ai-verify".to_string()),
                    help: Some("Verify AI patches before applying".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "verbose-ai".to_string(),
                    short: None,
                    long: Some("verbose-ai".to_string()),
                    help: Some("Print AI reasoning and confidence scores".to_string()),
                    required: false,
                    default: None,
                },
                PluginArgDef {
                    name: "verify".to_string(),
                    short: None,
                    long: Some("verify".to_string()),
                    help: Some("Full verification: lint + build + tests (Superpowers)".to_string()),
                    required: false,
                    default: None,
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

    if command != "check" {
        return Ok(Json(PluginExecuteOutput {
            exit_code: 1,
            message: Some(format!("Unknown command: {}", command)),
        }));
    }

    let run_input = build_check_run_input(args, working_dir);
    let output = unsafe { appz_pcheck_run(Json(run_input)) }
        .map_err(|e| extism_pdk::Error::msg(e.to_string()))?;

    Ok(Json(PluginExecuteOutput {
        exit_code: output.0.exit_code,
        message: output.0.message,
    }))
}

fn build_check_run_input(
    args: &HashMap<String, serde_json::Value>,
    working_dir: &str,
) -> PluginCheckRunInput {
    let bool_arg = |key: &str| -> bool {
        args.get(key)
            .and_then(|v| v.as_bool())
            .or_else(|| args.get(key).and_then(|v| v.as_str()).map(|s| s == "true" || s == "1"))
            .unwrap_or(false)
    };

    let str_arg = |key: &str| -> Option<String> {
        args.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    let u32_arg = |key: &str, default: u32| -> u32 {
        args.get(key)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .or_else(|| args.get(key).and_then(|v| v.as_u64()).map(|n| n as u32))
            .unwrap_or(default)
    };

    let optional_bool = |key: &str| -> Option<bool> {
        args.get(key).and_then(|v| {
            v.as_bool()
                .or_else(|| v.as_str().map(|s| s == "true" || s == "1"))
        })
    };

    PluginCheckRunInput {
        working_dir: working_dir.to_string(),
        fix: bool_arg("fix"),
        ai_fix: bool_arg("ai-fix"),
        strict: bool_arg("strict"),
        changed: bool_arg("changed"),
        staged: bool_arg("staged"),
        format: bool_arg("format"),
        json: bool_arg("json"),
        checker: str_arg("checker"),
        jobs: str_arg("jobs").and_then(|s| s.parse().ok()),
        init: bool_arg("init"),
        max_attempts: u32_arg("max-attempts", 3),
        ai_verify: optional_bool("ai-verify"),
        verbose_ai: bool_arg("verbose-ai"),
        verify: bool_arg("verify"),
    }
}
