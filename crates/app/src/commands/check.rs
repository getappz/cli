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
    runner::RunOptions,
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

/// Run check with an existing sandbox. Used by both the CLI command and the
/// check plugin host function.
#[allow(clippy::too_many_arguments)]
pub async fn run_check_with_sandbox(
    project_dir: &std::path::Path,
    sandbox: Arc<dyn sandbox::SandboxProvider>,
    fix: bool,
    ai_fix: bool,
    strict: bool,
    changed: bool,
    staged: bool,
    format: bool,
    json_output: bool,
    checker_slug: Option<String>,
    jobs: Option<usize>,
    max_attempts: u32,
    ai_verify: Option<bool>,
    verbose_ai: bool,
) -> Result<i32, miette::Report> {
    // 1. Load check config from appz.json.
    let check_config = read_check_config(project_dir)
        .map_err(|e| miette!("{}", e))?
        .unwrap_or_default();

    // Resolve strict from config or CLI flag.
    let effective_strict = strict || check_config.strict.unwrap_or(false);

    // 2. Resolve file filter (--changed / --staged).
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
            return Ok(0);
        }
        if !json_output {
            let _ = ui::status::info(&format!("Checking {} changed file(s)", files.len()));
        }
    }

    // 3. Build run options.
    let options = RunOptions {
        fix,
        format,
        strict: effective_strict,
        json_output,
        is_ci: {
            #[cfg(feature = "deploy")]
            { deployer::is_ci_environment() }
            #[cfg(not(feature = "deploy"))]
            { std::env::var("CI").is_ok() || std::env::var("CONTINUOUS_INTEGRATION").is_ok() }
        },
        file_filter,
        checker: checker_slug,
        jobs,
        check_config: check_config.clone(),
    };

    // 4. Run checks.
    let report = run_checks(sandbox.clone(), options)
        .await
        .map_err(|e| miette!("{}", e))?;

    // 5. AI fix pass (if --ai-fix and there are remaining issues).
    if ai_fix && !report.issues.is_empty() && !json_output {
        let repair_config = ai_fixer::AiRepairConfig::from_check_config(
            &check_config,
            Some(max_attempts),
            ai_verify,
            verbose_ai,
        )
        .map(|mut c| {
            c.project_dir = Some(project_dir.to_path_buf());
            c
        });
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

    // 6. Output results.
    if json_output {
        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| miette!("Failed to serialize report: {}", e))?;
        println!("{}", json);
    } else {
        fixer::display_streamed_summary(&report, effective_strict);
    }

    Ok(if report.passed(effective_strict) { 0 } else { 1 })
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

    // Create sandbox and run check.
    let sandbox = create_check_sandbox(&project_dir).await?;
    let exit_code = run_check_with_sandbox(
        &project_dir,
        sandbox,
        fix,
        ai_fix,
        strict,
        changed,
        staged,
        format,
        json_output,
        checker_slug,
        _jobs,
        max_attempts,
        ai_verify,
        verbose_ai,
    )
    .await
    .map_err(|e| miette!("{}", e))?;

    Ok(if exit_code == 0 {
        None
    } else {
        Some(1)
    })
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
