//! Check command implementation.
//!
//! Orchestrates the check flow:
//! 1. Create a sandbox for the project directory.
//! 2. Load check configuration from appz.json.
//! 3. Optionally filter files via git (--changed / --staged).
//! 4. Detect applicable providers and run checks (in parallel).
//! 5. Optionally run auto-fix (--fix) or AI-assisted fix (--ai-fix).
//! 6. Display results (or JSON output for CI).

use std::sync::Arc;

use checker::{
    ai_fixer, fixer, git, init, read_check_config, run_checks,
    runner::RunOptions, CheckConfig,
};
use miette::{miette, Result};
use starbase::AppResult;

use crate::session::AppzSession;

// ---------------------------------------------------------------------------
// Sandbox creation helper
// ---------------------------------------------------------------------------

/// Create and initialise a sandbox for the project directory.
async fn create_check_sandbox(
    project_dir: &std::path::Path,
) -> Result<Arc<dyn sandbox::SandboxProvider>> {
    let config = sandbox::SandboxConfig::new(project_dir)
        .with_settings(sandbox::SandboxSettings::default().quiet());

    let sb = sandbox::create_sandbox(config)
        .await
        .map_err(|e| miette!("Failed to create check sandbox: {}", e))?;

    Ok(Arc::from(sb))
}

/// Main check command entry point.
#[allow(clippy::too_many_arguments)]
pub async fn check(
    session: AppzSession,
    fix: bool,
    ai_fix: bool,
    strict: bool,
    changed: bool,
    staged: bool,
    format: bool,
    json_output: bool,
    _watch: bool,
    checker_slug: Option<String>,
    _jobs: Option<usize>,
    do_init: bool,
    max_attempts: u32,
    ai_verify: Option<bool>,
    verbose_ai: bool,
) -> AppResult {
    let project_dir = session.working_dir.clone();

    // Handle --init separately.
    if do_init {
        return run_init(&project_dir);
    }

    // 1. Load check config from appz.json.
    let check_config = read_check_config(&project_dir)
        .map_err(|e| miette!("{}", e))?
        .unwrap_or_default();

    // Resolve strict from config or CLI flag.
    let effective_strict = strict || check_config.strict.unwrap_or(false);

    // 2. Create sandbox.
    let sandbox = create_check_sandbox(&project_dir).await?;

    // 3. Resolve file filter (--changed / --staged).
    let file_filter = if staged {
        Some(
            git::staged_files(sandbox.as_ref())
                .await
                .map_err(|e| miette!("{}", e))?,
        )
    } else if changed {
        Some(
            git::changed_files(sandbox.as_ref())
                .await
                .map_err(|e| miette!("{}", e))?,
        )
    } else {
        None
    };

    // Check if file filter resulted in no files.
    if let Some(ref files) = file_filter {
        if files.is_empty() {
            if !json_output {
                let _ = ui::status::success("No changed files to check.");
            }
            return Ok(None);
        }
        if !json_output {
            let _ = ui::status::info(&format!("Checking {} changed file(s)", files.len()));
        }
    }

    // 4. Build run options.
    let options = RunOptions {
        fix,
        format,
        strict: effective_strict,
        json_output,
        is_ci: deployer::is_ci_environment(),
        file_filter,
        checker: checker_slug,
        jobs: _jobs,
        check_config: check_config.clone(),
    };

    // 5. Run checks.
    let report = run_checks(sandbox.clone(), options)
        .await
        .map_err(|e| miette!("{}", e))?;

    // 6. AI fix pass (if --ai-fix and there are remaining issues).
    if ai_fix && !report.issues.is_empty() && !json_output {
        let repair_config = ai_fixer::AiRepairConfig::from_check_config(
            &check_config,
            Some(max_attempts),
            ai_verify,
            verbose_ai,
        );
        match repair_config {
            Some(config) => {
                let applied = ai_fixer::run_ai_fix(sandbox.clone(), &report.issues, &config)
                    .await
                    .map_err(|e| miette!("{}", e))?;

                if applied > 0 {
                    let _ = ui::status::success(&format!(
                        "AI applied fixes to {} file{}",
                        applied,
                        if applied == 1 { "" } else { "s" }
                    ));
                }
            }
            None => {
                let _ = ui::status::warning(
                    "AI fix requires configuration. Set aiProvider and aiModel in appz.json check section,\n\
                     and set the appropriate API key environment variable (OPENAI_API_KEY, ANTHROPIC_API_KEY)."
                );
            }
        }
    }

    // 7. Output results.
    if json_output {
        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| miette!("Failed to serialize report: {}", e))?;
        println!("{}", json);
    } else {
        fixer::display_report_summary(&report, effective_strict);
    }

    // 8. Exit with appropriate code.
    if report.passed(effective_strict) {
        Ok(None)
    } else {
        Ok(Some(1))
    }
}

/// Run the --init flow.
fn run_init(project_dir: &std::path::Path) -> AppResult {
    let _ = ui::status::info("Initializing checker configuration...");

    let created = init::run_init(project_dir).map_err(|e| miette!("{}", e))?;

    if created.is_empty() {
        let _ = ui::status::info(
            "All config files already exist. No changes made.\n\
             Delete existing config files if you want to regenerate them.",
        );
    } else {
        for file in &created {
            let _ = ui::status::success(&format!("Created {}", file));
        }
        let _ = ui::status::success(&format!(
            "Initialized {} config file{}. Run 'appz check' to start checking.",
            created.len(),
            if created.len() == 1 { "" } else { "s" }
        ));
    }

    Ok(None)
}
