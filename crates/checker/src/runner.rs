//! Parallel check execution engine.
//!
//! Orchestrates running multiple check providers concurrently, streaming
//! results as they complete, and aggregating into a final [`CheckReport`].

use std::sync::Arc;
use std::time::Instant;

use futures::future::join_all;

use crate::config::{CheckConfig, CheckContext};
use crate::error::{CheckResult, CheckerError};
use crate::output::CheckReport;
use crate::provider::{detect_applicable_providers, get_provider, CheckProvider};

/// Options for a check run.
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

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            fix: false,
            format: false,
            strict: false,
            json_output: false,
            is_ci: false,
            file_filter: None,
            checker: None,
            jobs: None,
            check_config: CheckConfig::default(),
        }
    }
}

/// Run checks on a project using the sandbox.
///
/// This is the main entry point. It:
/// 1. Detects applicable providers (or uses explicit selection).
/// 2. Ensures tools are installed.
/// 3. Runs all providers concurrently.
/// 4. Optionally runs auto-fix.
/// 5. Aggregates results into a `CheckReport`.
pub async fn run_checks(
    sandbox: Arc<dyn sandbox::SandboxProvider>,
    options: RunOptions,
) -> CheckResult<CheckReport> {
    let start = Instant::now();
    let project_dir = sandbox.project_path().to_path_buf();

    // 1. Determine which providers to run.
    let providers = resolve_providers(&project_dir, &options)?;

    if providers.is_empty() {
        return Err(CheckerError::NoProvidersDetected);
    }

    if !options.json_output {
        let names: Vec<&str> = providers.iter().map(|p| p.name()).collect();
        let _ = ui::status::info(&format!(
            "Running {} checker{}: {}",
            providers.len(),
            if providers.len() == 1 { "" } else { "s" },
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
        .map(|p| Arc::from(p))
        .collect();

    // 3. Ensure tools are installed (in parallel).
    let tool_futs: Vec<_> = providers
        .iter()
        .map(|p| {
            let sandbox = sandbox.clone();
            let name = p.name().to_string();
            let provider = p.clone();
            async move {
                let result = provider.ensure_tool(sandbox.as_ref()).await;
                (name, result)
            }
        })
        .collect();

    let tool_results = join_all(tool_futs).await;

    // Filter out providers whose tools couldn't be installed.
    let mut runnable_indices = Vec::new();
    for (i, (name, result)) in tool_results.into_iter().enumerate() {
        match result {
            Ok(()) => runnable_indices.push(i),
            Err(e) => {
                if !options.json_output {
                    let _ = ui::status::warning(&format!(
                        "Skipping {} (tool install failed: {})",
                        name, e
                    ));
                }
            }
        }
    }

    // 4. Run checks (concurrently).
    let do_fix = options.fix;
    let check_futs: Vec<_> = runnable_indices
        .iter()
        .map(|&i| {
            let provider = providers[i].clone();
            let ctx = ctx.clone();
            let slug = provider.slug().to_string();
            let supports_fix = provider.supports_fix();
            async move {
                if do_fix && supports_fix {
                    // In fix mode, run fix first then collect remaining issues.
                    let fix_result = provider.fix(&ctx).await;
                    match fix_result {
                        Ok(fix_report) => Ok((slug, fix_report.remaining_issues, Some(fix_report.fixed_count))),
                        Err(e) => Err((slug, e)),
                    }
                } else {
                    match provider.check(&ctx).await {
                        Ok(issues) => Ok((slug, issues, None)),
                        Err(e) => Err((slug, e)),
                    }
                }
            }
        })
        .collect();

    let check_results = join_all(check_futs).await;

    // 5. Aggregate results.
    let mut report = CheckReport::new();
    let mut total_fixed = 0usize;

    for result in check_results {
        match result {
            Ok((slug, issues, fixed_count)) => {
                report.providers_run.push(slug);
                report.issues.extend(issues);
                if let Some(fc) = fixed_count {
                    total_fixed += fc;
                }
            }
            Err((slug, err)) => {
                if !options.json_output {
                    let _ = ui::status::warning(&format!("{} failed: {}", slug, err));
                }
                report.providers_run.push(slug);
            }
        }
    }

    // Sort issues: errors first, then by file, then by line.
    report.issues.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });

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
