//! SEO fix command - preview and apply SEO fixes with routing and control

use crate::session::AppzSession;
use crate::utils::build::detect_build_output_dir;
use ai_seo::{
    analyze, aggregation::SiteAggregator,
    diff::{build_diff, DiffRecorder},
    fix_plan::{generate_site_fix_plans, FixScope},
    mutation::{apply_fix_plans_with_context, MutationContext},
    preview::{build_ci_preview, build_fix_preview, format_cli_preview},
    FixRouting,
};
use std::rc::Rc;
use futures::future;
use itertools::Itertools;
use serde_json;
use starbase::AppResult;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::instrument;
use walkdir::WalkDir;

#[instrument(skip_all)]
pub async fn seo_fix(
    session: AppzSession,
    dir: Option<PathBuf>,
    preview: bool,
    apply: bool,
    json: bool,
    scope: Option<String>,
    skip: Option<String>,
    only: Option<String>,
) -> AppResult {
    // Default to preview mode if neither preview nor apply is specified
    let preview_mode = preview || !apply;

    // Use the working directory from session (already respects --cwd)
    let project_path = session.working_dir.clone();

    // Check if path exists
    if !project_path.exists() {
        return Err(miette::miette!(
            "Path does not exist: {}",
            project_path.display()
        ));
    }

    if !project_path.is_dir() {
        return Err(miette::miette!(
            "Path is not a directory: {}",
            project_path.display()
        ));
    }

    // Detect build output directory using shared utility
    let output_dir = detect_build_output_dir(&project_path, dir.clone()).await?;

    println!("✓ Analyzing SEO in: {}", output_dir.display());

    // Find all HTML files recursively
    let html_files: Result<Vec<PathBuf>, walkdir::Error> = WalkDir::new(&output_dir)
        .follow_links(true)
        .into_iter()
        .filter_ok(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("html"))
                    .unwrap_or(false)
        })
        .map_ok(|e| e.path().to_path_buf())
        .try_collect();

    let html_files = html_files
        .map_err(|e| miette::miette!("Failed to walk directory {}: {}", output_dir.display(), e))?;

    if html_files.is_empty() {
        return Err(miette::miette!(
            "No HTML files found in build output directory: {}",
            output_dir.display()
        ));
    }

    println!("Found {} HTML file(s) to analyze\n", html_files.len());

    // Process all files in parallel
    let results: Vec<Result<(String, ai_seo::models::SeoReport, String, PathBuf), miette::Error>> = 
        future::join_all(
            html_files.iter().map(|file_path| {
                let output_dir = output_dir.clone();
                let file_path = file_path.clone();
                
                async move {
                    // Read file
                    let html_content = tokio::fs::read_to_string(&file_path)
                        .await
                        .map_err(|e| miette::miette!("Failed to read file {}: {}", file_path.display(), e))?;

                    // Get relative path for display
                    let relative_path = file_path
                        .strip_prefix(&output_dir)
                        .unwrap_or(&file_path)
                        .to_string_lossy()
                        .to_string();

                    // Use relative path as URL for analysis
                    let url = format!("file:///{}", relative_path);
                    let report = analyze(&html_content, &url);

                    Ok((relative_path, report, html_content, file_path))
                }
            })
        )
        .await;

    // Collect results and aggregate
    let mut reports = Vec::new();
    let mut file_data = Vec::new(); // Store (relative_path, html_content, file_path) for fixing
    let mut site_aggregator = SiteAggregator::new();
    
    for result in results {
        let (relative_path, report, html_content, file_path) = result?;
        site_aggregator.ingest(&report);
        reports.push((relative_path.clone(), report));
        file_data.push((relative_path, html_content, file_path));
    }

    // Generate site-level summary
    let site_summary = site_aggregator.finalize();

    // Generate site-level fix plans
    let mut site_fix_plans = generate_site_fix_plans(&site_summary);

    // Apply scope override if provided
    if let Some(ref scope_str) = scope {
        let override_routing = parse_scope_override(scope_str);
        for plan in &mut site_fix_plans {
            plan.routing = override_routing.clone();
            plan.scope = match &override_routing {
                FixRouting::Template => FixScope::Template,
                FixRouting::Section { .. } => FixScope::Section,
                FixRouting::Page => FixScope::Page,
            };
        }
    }

    // Apply only filter if provided (takes precedence over skip)
    if let Some(ref only_str) = only {
        let only_list: Vec<&str> = only_str.split(',').map(|s| s.trim()).collect();
        site_fix_plans.retain(|plan| only_list.contains(&plan.issue));
    } else if let Some(ref skip_str) = skip {
        // Apply skip filter if only is not provided
        let skip_list: Vec<&str> = skip_str.split(',').map(|s| s.trim()).collect();
        site_fix_plans.retain(|plan| !skip_list.contains(&plan.issue));
    }

    if site_fix_plans.is_empty() {
        println!("✓ No fixes planned. All issues are either not fixable or were skipped.");
        return Ok(None);
    }

    // Generate dry-run diffs for preview
    let mut diffs_by_issue: HashMap<&'static str, Vec<ai_seo::diff::DryRunDiff>> = HashMap::new();

    if preview_mode {
        // For each fix plan, generate a sample diff
        for site_plan in &site_fix_plans {
            // Find a representative page for this issue
            if site_summary.issues.by_code
                .iter()
                .any(|ic| ic.code == site_plan.issue) {
                // Try to find a page with this issue
                for (relative_path, html_content, _) in &file_data {
                    // Check if this page has the issue
                    let report = reports.iter()
                        .find(|(rp, _)| rp == relative_path)
                        .map(|(_, r)| r);

                    if let Some(report) = report {
                        if report.issues.iter().any(|i| i.code == site_plan.issue) {
                            // Generate dry-run diff for this page
                            let recorder = Rc::new(DiffRecorder::new());
                            let ctx = MutationContext::dry_run_with_recorder(
                                vec![site_plan.plan.clone()],
                                Rc::clone(&recorder),
                            );

                            match apply_fix_plans_with_context(html_content, ctx) {
                                Ok(_) => {
                                    // Get events from the recorder
                                    let events = recorder.events();
                                    if !events.is_empty() {
                                        let diff = build_diff(relative_path, events);
                                        diffs_by_issue
                                            .entry(site_plan.issue)
                                            .or_insert_with(Vec::new)
                                            .push(diff);
                                    }
                                }
                                Err(_) => {
                                    // Skip if mutation fails
                                }
                            }
                            break; // Only need one sample
                        }
                    }
                }
            }
        }
    }

    // Build fix preview
    let fix_preview = build_fix_preview(&site_summary, &site_fix_plans, &diffs_by_issue);

    // Output based on mode
    if json {
        // JSON output for CI/automation
        let ci_preview = build_ci_preview(&fix_preview);
        let json_output = serde_json::to_string_pretty(&ci_preview)
            .map_err(|e| miette::miette!("Failed to serialize preview: {}", e))?;
        println!("{}", json_output);
    } else if preview_mode {
        // Human-readable preview
        let preview_text = format_cli_preview(&fix_preview);
        println!("{}", preview_text);
    }

    // Apply fixes if requested
    if apply {
        println!("\n{}", "=".repeat(80));
        println!("Applying Fixes");
        println!("{}", "=".repeat(80));
        
        let mut fixed_count = 0;
        let mut error_count = 0;

        for (relative_path, html_content, file_path) in &file_data {
            // Find the corresponding report
            let report = reports.iter()
                .find(|(rp, _)| rp == relative_path)
                .map(|(_, r)| r);

            if let Some(report) = report {
                // Collect fix plans for this page based on routing
                let mut page_plans = Vec::new();
                
                for site_plan in &site_fix_plans {
                    // Check if this page should get this fix based on routing
                    let should_fix = match &site_plan.routing {
                        FixRouting::Template => {
                            // Template fixes apply to all pages with the issue
                            report.issues.iter().any(|i| i.code == site_plan.issue)
                        }
                        FixRouting::Section { prefix } => {
                            // Section fixes apply to pages in that section
                            relative_path.starts_with(prefix) && 
                            report.issues.iter().any(|i| i.code == site_plan.issue)
                        }
                        FixRouting::Page => {
                            // Page fixes apply only if this page has the issue
                            report.issues.iter().any(|i| i.code == site_plan.issue)
                        }
                    };

                    if should_fix {
                        if !page_plans.iter().any(|p: &ai_seo::fix_plan::FixPlan| p.issue_code == site_plan.plan.issue_code) {
                            page_plans.push(site_plan.plan.clone());
                        }
                    }
                }

                if !page_plans.is_empty() {
                    // Apply fixes
                    let ctx = MutationContext::new(page_plans);
                    match apply_fix_plans_with_context(html_content, ctx) {
                        Ok(fixed_html) => {
                            if fixed_html != *html_content {
                                // Only write if content changed
                                tokio::fs::write(&file_path, fixed_html)
                                    .await
                                    .map_err(|e| miette::miette!(
                                        "Failed to write fixed file {}: {}",
                                        file_path.display(),
                                        e
                                    ))?;
                                fixed_count += 1;
                                println!("✓ Fixed: {}", relative_path);
                            }
                        }
                        Err(e) => {
                            error_count += 1;
                            eprintln!("✗ Error fixing {}: {}", relative_path, e);
                        }
                    }
                }
            }
        }

        println!("\nFix Summary:");
        println!("  Files fixed: {}", fixed_count);
        if error_count > 0 {
            println!("  Errors: {}", error_count);
        }
        println!("\n{}", "=".repeat(80));
    }

    Ok(None)
}

/// Parse scope override string into FixRouting
fn parse_scope_override(scope_str: &str) -> FixRouting {
    if scope_str == "template" {
        FixRouting::Template
    } else if scope_str == "page" {
        FixRouting::Page
    } else if let Some(prefix) = scope_str.strip_prefix("section:") {
        FixRouting::Section {
            prefix: prefix.to_string(),
        }
    } else {
        // Default to page if invalid
        FixRouting::Page
    }
}

