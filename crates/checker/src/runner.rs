//! Parallel check execution engine with streaming output.
//!
//! Orchestrates running multiple check providers concurrently, **streaming**
//! results to the terminal as each provider completes, and aggregating into
//! a final [`CheckReport`].

use std::sync::Arc;
use std::time::Instant;

use futures::stream::FuturesUnordered;
use futures::StreamExt;

use crate::config::{CheckConfig, CheckContext};
use crate::error::{CheckResult, CheckerError};
use crate::output::{CheckReport, Severity};
use crate::provider::{detect_applicable_providers, get_provider, CheckProvider};

/// Options for a check run.
#[derive(Default)]
pub struct RunOptions {
    /// Fix mode — auto-fix safe issues.
    pub fix: bool,
    /// Format mode — check/fix formatting.
    pub format: bool,
    /// Strict mode — treat warnings as errors.
    pub strict: bool,
    /// JSON output mode.
    pub json_output: bool,
    /// CI mode (non-interactive).
    pub is_ci: bool,
    /// File filter (from git --changed / --staged).
    pub file_filter: Option<Vec<String>>,
    /// Specific checker slug to run (overrides detection).
    pub checker: Option<String>,
    /// Number of concurrent jobs.
    pub jobs: Option<usize>,
    /// Check config from appz.json.
    pub check_config: CheckConfig,
}


/// Run checks on a project using the sandbox.
///
/// This is the main entry point. It:
/// 1. Detects applicable providers (or uses explicit selection).
/// 2. Ensures tools are installed.
/// 3. Runs all providers concurrently, **streaming results as each completes**.
/// 4. Optionally runs auto-fix.
/// 5. Aggregates results into a `CheckReport`.
///
/// When not in JSON mode, issues are printed to the terminal as each
/// provider finishes — the user sees output immediately instead of waiting
/// for all providers to complete.
pub async fn run_checks(
    sandbox: Arc<dyn sandbox::SandboxProvider>,
    options: RunOptions,
) -> CheckResult<CheckReport> {
    let start = Instant::now();
    let project_dir = sandbox.project_path().to_path_buf();
    let stream_output = !options.json_output;

    // 1. Determine which providers to run.
    let providers = resolve_providers(&project_dir, &options)?;

    if providers.is_empty() {
        return Err(CheckerError::NoProvidersDetected);
    }

    let total_providers = providers.len();

    if stream_output {
        let names: Vec<&str> = providers.iter().map(|p| p.name()).collect();
        let _ = ui::status::info(&format!(
            "Running {} checker{}: {}",
            total_providers,
            if total_providers == 1 { "" } else { "s" },
            names.join(", ")
        ));
    }

    // 2. Build the shared context.
    let ctx = CheckContext::new(sandbox.clone())
        .with_config(options.check_config.clone())
        .with_fix(options.fix)
        .with_format(options.format)
        .with_strict(options.strict)
        .with_file_filter(options.file_filter.clone())
        .with_json_output(options.json_output)
        .with_ci(options.is_ci);

    // Wrap providers in Arc for shared access across async tasks.
    let providers: Vec<Arc<dyn CheckProvider>> = providers
        .into_iter()
        .map(Arc::from)
        .collect();

    // 3. Ensure tools are installed (in parallel, streamed).
    let runnable = ensure_tools_streamed(&providers, &sandbox, stream_output).await;

    if runnable.is_empty() {
        return Err(CheckerError::NoProvidersDetected);
    }

    // 4. Run checks concurrently with streaming output.
    let do_fix = options.fix;
    let (report, total_fixed) = run_checks_streamed(
        &providers,
        &runnable,
        &ctx,
        do_fix,
        stream_output,
        total_providers,
    )
    .await;

    // 5. Build the final report.
    let mut report = report;
    report.duration = start.elapsed();
    report.recount();

    // Build fix report if applicable.
    if options.fix {
        let remaining_fixable = report
            .issues
            .iter()
            .filter(|i| i.has_safe_fix())
            .count();
        report.fix_report = Some(crate::output::FixReport {
            fixed_count: total_fixed,
            remaining_count: remaining_fixable,
            fixed_issues: Vec::new(),
            remaining_issues: Vec::new(),
        });
    }

    Ok(report)
}

// ---------------------------------------------------------------------------
// Tool installation (streamed)
// ---------------------------------------------------------------------------

/// Ensure tools are installed, printing status as each completes.
///
/// Returns the indices of providers whose tools installed successfully.
async fn ensure_tools_streamed(
    providers: &[Arc<dyn CheckProvider>],
    sandbox: &Arc<dyn sandbox::SandboxProvider>,
    stream: bool,
) -> Vec<usize> {
    let mut futures = FuturesUnordered::new();

    for (i, p) in providers.iter().enumerate() {
        let sandbox = sandbox.clone();
        let name = p.name().to_string();
        let provider = p.clone();
        futures.push(async move {
            let result = provider.ensure_tool(sandbox.as_ref()).await;
            (i, name, result)
        });
    }

    let mut runnable = Vec::new();

    while let Some((i, name, result)) = futures.next().await {
        match result {
            Ok(()) => {
                runnable.push(i);
            }
            Err(e) => {
                if stream {
                    let _ = ui::status::warning(&format!(
                        "Skipping {} (tool install failed: {})",
                        name, e
                    ));
                }
            }
        }
    }

    runnable
}

// ---------------------------------------------------------------------------
// Check execution (streamed)
// ---------------------------------------------------------------------------

/// Result from a single provider check.
struct ProviderResult {
    slug: String,
    name: String,
    issues: Vec<crate::output::CheckIssue>,
    fixed_count: Option<usize>,
    duration: std::time::Duration,
    error: Option<CheckerError>,
}

/// Run checks across all runnable providers, streaming results as each finishes.
async fn run_checks_streamed(
    providers: &[Arc<dyn CheckProvider>],
    runnable: &[usize],
    ctx: &CheckContext,
    do_fix: bool,
    stream: bool,
    total_providers: usize,
) -> (CheckReport, usize) {
    let mut futures = FuturesUnordered::new();

    for &i in runnable {
        let provider = providers[i].clone();
        let ctx = ctx.clone();
        let slug = provider.slug().to_string();
        let name = provider.name().to_string();
        let supports_fix = provider.supports_fix();

        futures.push(async move {
            let provider_start = Instant::now();

            if do_fix && supports_fix {
                match provider.fix(&ctx).await {
                    Ok(fix_report) => ProviderResult {
                        slug,
                        name,
                        issues: fix_report.remaining_issues,
                        fixed_count: Some(fix_report.fixed_count),
                        duration: provider_start.elapsed(),
                        error: None,
                    },
                    Err(e) => ProviderResult {
                        slug,
                        name,
                        issues: Vec::new(),
                        fixed_count: None,
                        duration: provider_start.elapsed(),
                        error: Some(e),
                    },
                }
            } else {
                match provider.check(&ctx).await {
                    Ok(issues) => ProviderResult {
                        slug,
                        name,
                        issues,
                        fixed_count: None,
                        duration: provider_start.elapsed(),
                        error: None,
                    },
                    Err(e) => ProviderResult {
                        slug,
                        name,
                        issues: Vec::new(),
                        fixed_count: None,
                        duration: provider_start.elapsed(),
                        error: Some(e),
                    },
                }
            }
        });
    }

    let mut report = CheckReport::new();
    let mut total_fixed = 0usize;
    let mut completed = 0usize;

    while let Some(result) = futures.next().await {
        completed += 1;

        if let Some(err) = result.error {
            if stream {
                let _ = ui::status::warning(&format!(
                    "[{}/{}] {} failed: {} ({:.1}s)",
                    completed,
                    total_providers,
                    result.name,
                    err,
                    result.duration.as_secs_f64()
                ));
            }
            report.providers_run.push(result.slug);
            continue;
        }

        // Accumulate stats.
        let errors = result
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count();
        let warnings = result
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count();

        if let Some(fc) = result.fixed_count {
            total_fixed += fc;
        }

        report.providers_run.push(result.slug);

        // --- Stream: print provider result immediately ---
        if stream {
            let status_msg = format_provider_status(
                completed,
                total_providers,
                &result.name,
                errors,
                warnings,
                result.fixed_count,
                result.duration,
            );

            if errors > 0 {
                let _ = ui::status::error(&status_msg);
            } else if warnings > 0 {
                let _ = ui::status::warning(&status_msg);
            } else {
                let _ = ui::status::success(&status_msg);
            }

            // Print individual issues for this provider (if any).
            if !result.issues.is_empty() {
                display_issues_inline(&result.issues);
            }
        }

        report.issues.extend(result.issues);
    }

    // Sort all accumulated issues: errors first, then by file, then by line.
    report.issues.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });

    (report, total_fixed)
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

/// Format a per-provider status line.
fn format_provider_status(
    completed: usize,
    total: usize,
    name: &str,
    errors: usize,
    warnings: usize,
    fixed_count: Option<usize>,
    duration: std::time::Duration,
) -> String {
    let time = format!("{:.1}s", duration.as_secs_f64());

    let mut parts = Vec::new();
    if errors > 0 {
        parts.push(format!(
            "{} error{}",
            errors,
            if errors == 1 { "" } else { "s" }
        ));
    }
    if warnings > 0 {
        parts.push(format!(
            "{} warning{}",
            warnings,
            if warnings == 1 { "" } else { "s" }
        ));
    }
    if let Some(fc) = fixed_count {
        if fc > 0 {
            parts.push(format!("{} fixed", fc));
        }
    }

    let detail = if parts.is_empty() {
        "clean".to_string()
    } else {
        parts.join(", ")
    };

    format!("[{}/{}] {} ({}) — {}", completed, total, name, time, detail)
}

/// Display issues inline for a single provider (during streaming).
fn display_issues_inline(issues: &[crate::output::CheckIssue]) {
    let mut current_file = String::new();

    for issue in issues {
        let file_str = issue.file.display().to_string();
        if file_str != current_file {
            let _ = ui::status::info(&format!("  {}", file_str));
            current_file = file_str;
        }

        let location = match (issue.line, issue.column) {
            (Some(l), Some(c)) => format!("{}:{}", l, c),
            (Some(l), None) => format!("{}:1", l),
            _ => String::new(),
        };

        let code_str = issue
            .code
            .as_deref()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();

        let fix_indicator = if issue.has_safe_fix() {
            " (fixable)"
        } else {
            ""
        };

        let line = format!(
            "    {:>8} {:>10}  {}{}{}",
            issue.severity, location, issue.message, code_str, fix_indicator
        );

        match issue.severity {
            Severity::Error => {
                let _ = ui::status::error(&line);
            }
            Severity::Warning => {
                let _ = ui::status::warning(&line);
            }
            _ => {
                let _ = ui::status::info(&line);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Provider resolution
// ---------------------------------------------------------------------------

/// Resolve which providers to run based on options and project detection.
fn resolve_providers(
    project_dir: &std::path::Path,
    options: &RunOptions,
) -> CheckResult<Vec<Box<dyn CheckProvider>>> {
    // If a specific checker is requested, use it.
    if let Some(ref slug) = options.checker {
        let provider = get_provider(slug)?;
        return Ok(vec![provider]);
    }

    // If config specifies explicit providers, use those.
    if let Some(ref providers_list) = options.check_config.providers {
        let mut providers = Vec::new();
        for slug in providers_list {
            providers.push(get_provider(slug)?);
        }
        return Ok(providers);
    }

    // Auto-detect applicable providers.
    let frameworks: Vec<&str> = Vec::new(); // Framework hints (TODO: pass from detection).
    let mut providers = detect_applicable_providers(project_dir, &frameworks);

    // Remove disabled providers.
    if let Some(ref disabled) = options.check_config.disabled {
        providers.retain(|p| !disabled.contains(&p.slug().to_string()));
    }

    Ok(providers)
}
