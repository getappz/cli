//! AI-assisted repair with three-agent architecture and retry loop.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌──────────────┐
//! │   Planner   │ ──▶ │   Fixer     │ ──▶ │   Verifier   │
//! │  (analyse)  │     │ (diff patch) │     │  (validate)  │
//! └─────────────┘     └─────────────┘     └──────┬───────┘
//!                                                 │
//!                              ┌──────────────────┘
//!                              ▼
//!                    ┌───────────────────┐
//!                    │  Apply or Retry   │
//!                    │  with Reflection  │
//!                    └───────────────────┘
//! ```
//!
//! # Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`llm`] | LLM client abstraction (OpenAI, Anthropic, Ollama) |
//! | [`agents`] | Planner, Fix, and Verify agents with prompt templates |
//! | [`context`] | Relevant file collection and import tracing |
//! | [`patch`] | Unified diff parsing and safe application |
//! | [`safety`] | Guardrails: protected files, change limits, confidence |

pub mod agents;
pub mod context;
pub mod llm;
pub mod patch;
pub mod safety;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::CheckConfig;
use crate::error::CheckResult;
use crate::output::CheckIssue;

use self::agents::{run_fixer, run_planner, run_verifier};
use self::context::collect_context;
use self::llm::{LlmConfig, TokenUsage};
use self::patch::{apply_patch, display_patch_preview, parse_unified_diff, validate_patch};
use self::safety::{confidence_ok, filter_sendable_issues, validate_safety, SafetyConfig};

// ---------------------------------------------------------------------------
// Public configuration
// ---------------------------------------------------------------------------

/// Configuration for the AI repair pipeline.
#[derive(Debug, Clone)]
pub struct AiRepairConfig {
    /// LLM config for the Planner agent.
    pub planner_llm: LlmConfig,
    /// LLM config for the Fix agent.
    pub fixer_llm: LlmConfig,
    /// LLM config for the Verify agent.
    pub verifier_llm: LlmConfig,
    /// Safety guardrails.
    pub safety: SafetyConfig,
    /// Whether verification is enabled.
    pub verify_enabled: bool,
    /// Whether to print verbose AI reasoning.
    pub verbose: bool,
    /// Maximum context bytes for the AI.
    pub max_context_bytes: Option<usize>,
    /// Project directory for custom prompt resolution.
    pub project_dir: Option<std::path::PathBuf>,
}

impl AiRepairConfig {
    /// Build the repair config from CheckConfig and CLI flags.
    ///
    /// Falls back to sensible defaults for any missing configuration.
    pub fn from_check_config(
        config: &CheckConfig,
        max_attempts: Option<u32>,
        ai_verify: Option<bool>,
        verbose_ai: bool,
    ) -> Option<Self> {
        let provider = config.ai_provider.as_deref().unwrap_or("openai");

        // Resolve API key.
        let api_key = match provider {
            "openai" => std::env::var("OPENAI_API_KEY").ok(),
            "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
            "ollama" => None,
            _ => std::env::var("AI_API_KEY").ok(),
        };

        let base_url = match provider {
            "ollama" => Some(
                std::env::var("OLLAMA_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            ),
            _ => None,
        };

        // Resolve per-role models.
        let default_model = config
            .ai_model
            .as_deref()
            .unwrap_or("gpt-4o")
            .to_string();

        let planner_model = config
            .ai_models
            .as_ref()
            .and_then(|m| m.planner.clone())
            .unwrap_or_else(|| cheap_model_for(&default_model));

        let fixer_model = config
            .ai_models
            .as_ref()
            .and_then(|m| m.fixer.clone())
            .unwrap_or_else(|| default_model.clone());

        let verifier_model = config
            .ai_models
            .as_ref()
            .and_then(|m| m.verifier.clone())
            .unwrap_or_else(|| cheap_model_for(&default_model));

        let make_llm = |model: String, temp: f32| LlmConfig {
            provider: provider.to_string(),
            model,
            api_key: api_key.clone(),
            base_url: base_url.clone(),
            temperature: temp,
        };

        // Safety config from appz.json or defaults.
        let mut safety = config
            .ai_safety
            .clone()
            .unwrap_or_default();

        // CLI max_attempts overrides config.
        if let Some(attempts) = max_attempts {
            safety.max_attempts = attempts;
        }
        if let Some(cfg_attempts) = config.ai_max_attempts {
            if max_attempts.is_none() {
                safety.max_attempts = cfg_attempts;
            }
        }

        // Verification: CLI flag > config > default (true for interactive).
        let verify_enabled = ai_verify.unwrap_or(true);

        Some(Self {
            planner_llm: make_llm(planner_model, 0.1),
            fixer_llm: make_llm(fixer_model, 0.1),
            verifier_llm: make_llm(verifier_model, 0.1),
            safety,
            verify_enabled,
            verbose: verbose_ai,
            max_context_bytes: None,
            project_dir: None,
        })
    }
}

/// Derive a cheaper model name from the main model.
///
/// Heuristic: if the model is "gpt-4o" use "gpt-4o-mini",
/// if "claude-sonnet-*" use same (no smaller variant currently), etc.
fn cheap_model_for(model: &str) -> String {
    if model.starts_with("gpt-4o") && !model.contains("mini") {
        return "gpt-4o-mini".to_string();
    }
    if model.starts_with("gpt-4") && !model.contains("mini") {
        return "gpt-4o-mini".to_string();
    }
    // For other providers, use the same model (user should configure explicitly).
    model.to_string()
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Run the AI repair pipeline.
///
/// This is the main entry point called by the check command when `--ai-fix`
/// is active. It:
///
/// 1. Filters issues through safety guardrails
/// 2. Presents interactive file selection
/// 3. Runs the Plan → Fix → Verify → Apply loop with retries
/// 4. Returns the number of files successfully fixed
pub async fn run_ai_fix(
    sandbox: Arc<dyn sandbox::SandboxProvider>,
    issues: &[CheckIssue],
    config: &AiRepairConfig,
) -> CheckResult<usize> {
    if issues.is_empty() {
        let _ = ui::status::info("No issues to fix with AI.");
        return Ok(0);
    }

    // 1. Filter out issues in never-send files.
    let sendable = filter_sendable_issues(issues, &config.safety);
    if sendable.is_empty() {
        let _ = ui::status::info("All issues are in protected/secret files. Cannot send to AI.");
        return Ok(0);
    }

    // 2. Group by file.
    let mut by_file: HashMap<PathBuf, Vec<&CheckIssue>> = HashMap::new();
    for issue in &sendable {
        by_file
            .entry(issue.file.clone())
            .or_default()
            .push(issue);
    }

    let _ = ui::status::info(&format!(
        "Found {} issue{} in {} file{} for AI review",
        sendable.len(),
        if sendable.len() == 1 { "" } else { "s" },
        by_file.len(),
        if by_file.len() == 1 { "" } else { "s" },
    ));

    // 3. Interactive file selection.
    let options: Vec<(String, String)> = by_file
        .keys()
        .map(|p| {
            let name = p.display().to_string();
            (name.clone(), name)
        })
        .collect();

    let selected: Vec<String> = match ui::prompt::checkbox("Select files to fix with AI:", options)
    {
        Ok(s) => s,
        Err(_) => {
            let _ = ui::status::info("AI fix cancelled.");
            return Ok(0);
        }
    };

    if selected.is_empty() {
        let _ = ui::status::info("No files selected.");
        return Ok(0);
    }

    // Filter issues to selected files only.
    let selected_paths: Vec<PathBuf> = selected.iter().map(PathBuf::from).collect();
    let selected_issues: Vec<CheckIssue> = sendable
        .into_iter()
        .filter(|i| selected_paths.contains(&i.file))
        .collect();

    // 4. Collect context.
    let _ = ui::status::info("Collecting project context for AI...");
    let context = collect_context(sandbox.fs(), &selected_issues, config.max_context_bytes);

    if config.verbose {
        let _ = ui::status::info(&format!(
            "Context: {} error files, {} related files, {:.1} KB total",
            context.error_files.len(),
            context.related_files.len(),
            context.total_bytes as f64 / 1024.0,
        ));
    }

    // 5. Run the repair loop.
    let total_fixed = repair_loop(sandbox, &selected_issues, &context, config).await?;

    Ok(total_fixed)
}

// ---------------------------------------------------------------------------
// Repair loop
// ---------------------------------------------------------------------------

/// The core retry-with-reflection repair loop.
///
/// ```text
/// loop {
///     plan = planner(diagnostics, context)
///     diff = fixer(plan, context)
///     if verify(diff) && confidence >= threshold:
///         apply(diff)
///         re-check patched files
///         if clean: break (success)
///         else: reflect and retry
///     else:
///         reflect and retry
///     if attempts >= max: break (give up)
/// }
/// ```
async fn repair_loop(
    sandbox: Arc<dyn sandbox::SandboxProvider>,
    issues: &[CheckIssue],
    context: &context::RepairContext,
    config: &AiRepairConfig,
) -> CheckResult<usize> {
    let max_attempts = config.safety.max_attempts;
    let mut attempt = 0u32;
    let mut total_fixed = 0usize;
    let mut reflection: Option<String> = None;
    let current_issues = issues.to_vec();
    let mut cumulative_usage = TokenUsage::default();

    while attempt < max_attempts && !current_issues.is_empty() {
        attempt += 1;
        let _ = ui::status::info(&format!(
            "AI repair attempt {}/{}...",
            attempt, max_attempts
        ));

        // --- Plan ---
        let proj_dir = config.project_dir.as_deref();
        let (plan, planner_usage) = match run_planner(
            &config.planner_llm,
            &current_issues,
            context,
            config.verbose,
            proj_dir,
        )
        .await
        {
            Ok(p) => p,
            Err(e) => {
                let _ = ui::status::warning(&format!("[Planner] Failed: {}", e));
                break;
            }
        };
        accumulate_usage(&mut cumulative_usage, planner_usage.as_ref());

        if plan.files_to_edit.is_empty() {
            let _ = ui::status::info("[Planner] No files to edit. Stopping.");
            break;
        }

        // --- Fix ---
        let (diff_text, fixer_usage) = match run_fixer(
            &config.fixer_llm,
            &plan,
            &current_issues,
            context,
            reflection.as_deref(),
            config.verbose,
            proj_dir,
        )
        .await
        {
            Ok(d) => d,
            Err(e) => {
                let _ = ui::status::warning(&format!("[Fixer] Failed: {}", e));
                reflection = Some(format!("Fix agent failed with error: {}", e));
                continue;
            }
        };
        accumulate_usage(&mut cumulative_usage, fixer_usage.as_ref());

        // --- Parse the diff ---
        let parsed_patch = match parse_unified_diff(&diff_text) {
            Ok(p) => p,
            Err(e) => {
                let _ = ui::status::warning(&format!("[Patch] Failed to parse diff: {}", e));
                reflection = Some(format!(
                    "The previous diff was malformed and could not be parsed: {}.\n\
                     Please produce a valid unified diff.",
                    e
                ));
                continue;
            }
        };

        // --- Safety validation ---
        if let Err(e) = validate_safety(&parsed_patch, &config.safety) {
            let _ = ui::status::warning(&format!("[Safety] {}", e));
            reflection = Some(format!(
                "The previous patch was rejected by safety checks: {}.\n\
                 Make smaller, more targeted changes.",
                e
            ));
            continue;
        }

        // Read current file contents for patch application.
        let mut file_contents: HashMap<PathBuf, String> = HashMap::new();
        for fp in &parsed_patch.file_patches {
            let name = fp.path.display().to_string();
            match sandbox.fs().read_to_string(&name) {
                Ok(content) => {
                    file_contents.insert(fp.path.clone(), content);
                }
                Err(e) => {
                    let _ =
                        ui::status::warning(&format!("Could not read {}: {}", fp.path.display(), e));
                }
            }
        }

        // Validate change percentages.
        if let Err(e) = validate_patch(
            &parsed_patch,
            &file_contents,
            config.safety.max_change_pct,
            config.safety.max_files_per_patch,
        ) {
            let _ = ui::status::warning(&format!("[Safety] {}", e));
            reflection = Some(format!(
                "The previous patch was rejected: {}.\n\
                 Reduce the scope of changes.",
                e
            ));
            continue;
        }

        // --- Verify (optional) ---
        if config.verify_enabled {
            match run_verifier(
                &config.verifier_llm,
                &diff_text,
                &current_issues,
                context,
                config.verbose,
                proj_dir,
            )
            .await
            {
                Ok((verify_result, verifier_usage)) => {
                    accumulate_usage(&mut cumulative_usage, verifier_usage.as_ref());
                    if !verify_result.valid {
                        let _ = ui::status::warning(&format!(
                            "[Verifier] Rejected: {}",
                            verify_result.reason
                        ));
                        reflection = Some(format!(
                            "The verifier rejected the patch: {}.\n\
                             Please address these concerns in the next attempt.",
                            verify_result.reason
                        ));
                        continue;
                    }

                    if !confidence_ok(verify_result.risk_score, &config.safety) {
                        // Note: risk_score is inverted vs confidence. High risk = low confidence.
                        // We check if (1.0 - risk_score) >= min_confidence.
                        let confidence = 1.0 - verify_result.risk_score;
                        if confidence < config.safety.min_confidence {
                            let _ = ui::status::warning(&format!(
                                "[Verifier] Confidence too low ({:.2}, min: {:.2}): {}",
                                confidence, config.safety.min_confidence, verify_result.reason
                            ));
                            reflection = Some(format!(
                                "The verifier gave a low confidence score ({:.2}): {}.\n\
                                 Make simpler, more targeted changes.",
                                confidence, verify_result.reason
                            ));
                            continue;
                        }
                    }

                    if config.verbose {
                        let _ = ui::status::success(&format!(
                            "[Verifier] Approved (risk: {:.2}): {}",
                            verify_result.risk_score, verify_result.reason
                        ));
                    }
                }
                Err(e) => {
                    let _ = ui::status::warning(&format!(
                        "[Verifier] Failed (proceeding anyway): {}",
                        e
                    ));
                }
            }
        }

        // --- Show patch preview ---
        let _ = ui::layout::blank_line();
        display_patch_preview(&parsed_patch);
        let _ = ui::layout::blank_line();

        // --- Confirm with user ---
        let apply = ui::prompt::confirm("Apply this patch?", true).unwrap_or_default();

        if !apply {
            let _ = ui::status::info("Patch rejected by user.");
            break;
        }

        // --- Apply the patch ---
        match apply_patch(&parsed_patch, &file_contents) {
            Ok(results) => {
                let mut files_fixed = 0;
                for (path, result) in &results {
                    let name = path.display().to_string();
                    match sandbox.fs().write_string(&name, &result.content) {
                        Ok(()) => {
                            let _ = ui::status::success(&format!(
                                "Patched {} (+{} -{}, {:.1}% changed)",
                                name, result.lines_added, result.lines_removed, result.change_pct
                            ));
                            files_fixed += 1;
                        }
                        Err(e) => {
                            let _ = ui::status::error(&format!(
                                "Failed to write {}: {}",
                                name, e
                            ));
                        }
                    }
                }
                total_fixed += files_fixed;

                if files_fixed == 0 {
                    let _ = ui::status::warning("No files were actually patched.");
                    break;
                }

                // For now, we do a single repair pass per user confirmation.
                // In future, we could re-run diagnostics on the patched files
                // and continue the loop if new errors appear.
                let _ = ui::status::success(&format!(
                    "AI repair complete: {} file{} patched.",
                    files_fixed,
                    if files_fixed == 1 { "" } else { "s" }
                ));
                break;
            }
            Err(e) => {
                let _ = ui::status::error(&format!("[Patch] Application failed: {}", e));
                reflection = Some(format!(
                    "The previous patch failed to apply: {}.\n\
                     Ensure line numbers and context match the current file content.",
                    e
                ));
                continue;
            }
        }
    }

    if attempt >= max_attempts && !current_issues.is_empty() {
        let _ = ui::status::warning(&format!(
            "AI repair reached max attempts ({}) without fully resolving all issues.",
            max_attempts
        ));
    }

    // Display token usage summary.
    if config.verbose && cumulative_usage.total_tokens > 0 {
        let cost = cumulative_usage.estimate_cost_usd(&config.fixer_llm.model);
        let _ = ui::status::info(&format!(
            "[Tokens] Total: {} (prompt: {}, completion: {}) — est. cost: ${:.4}",
            cumulative_usage.total_tokens,
            cumulative_usage.prompt_tokens,
            cumulative_usage.completion_tokens,
            cost,
        ));
    }

    Ok(total_fixed)
}

/// Accumulate token usage from an API call into a running total.
fn accumulate_usage(total: &mut TokenUsage, usage: Option<&TokenUsage>) {
    if let Some(u) = usage {
        total.prompt_tokens += u.prompt_tokens;
        total.completion_tokens += u.completion_tokens;
        total.total_tokens += u.total_tokens;
    }
}
