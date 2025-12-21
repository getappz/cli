//! SEO audit command - analyze HTML files in build output directory

use crate::session::AppzSession;
use crate::utils::build::detect_build_output_dir;
use ai_seo::{
    analyze, aggregation::SiteAggregator, 
    fix_plan::plan_for_issue,
    mutation::apply_fix_plans,
    models::{SeoReport, Severity}
};
use futures::future;
use itertools::Itertools;
use starbase::AppResult;
use std::path::PathBuf;
use tracing::instrument;
use ui::table;
use walkdir::WalkDir;

#[instrument(skip_all)]
pub async fn seo_audit(
    session: AppzSession,
    dir: Option<PathBuf>,
    verbose: bool,
    fix: bool,
) -> AppResult {
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
    let results: Vec<Result<(String, SeoReport, String, PathBuf), miette::Error>> = future::join_all(
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

    // Display summary table
    let headers = vec!["File", "Score", "Critical", "High", "Medium", "Low"];
    let mut rows = Vec::new();

    for (file_path, report) in &reports {
        let score = report.score.total;

        let critical_count = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Critical))
            .count();
        let high_count = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::High))
            .count();
        let medium_count = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Medium))
            .count();
        let low_count = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Low))
            .count();

        rows.push(vec![
            file_path.clone(),
            score.to_string(),
            critical_count.to_string(),
            high_count.to_string(),
            medium_count.to_string(),
            low_count.to_string(),
        ]);
    }

    // Display table
    table::display(&headers, &rows, Some("SEO Audit Results"))?;

    // Display site-level summary
    println!("\n{}", "=".repeat(80));
    println!("Site-Level Summary");
    println!("{}", "=".repeat(80));
    println!("Pages analyzed: {}", site_summary.page_count);
    println!("\nScore Summary:");
    println!("  Average: {}/100", site_summary.score.average);
    println!("  Weighted: {}/100", site_summary.score.weighted);
    println!("  Range: {}-{}", site_summary.score.min, site_summary.score.max);
    
    println!("\nCoverage Metrics:");
    println!("  Title coverage: {:.1}%", site_summary.coverage.title_coverage * 100.0);
    println!("  Meta description coverage: {:.1}%", site_summary.coverage.meta_description_coverage * 100.0);
    println!("  H1 coverage: {:.1}%", site_summary.coverage.h1_coverage * 100.0);
    println!("  Image alt coverage: {:.1}%", site_summary.coverage.image_alt_coverage * 100.0);
    
    println!("\nIssue Summary:");
    println!("  By Severity:");
    println!("    Critical: {}", site_summary.issues.by_severity.critical);
    println!("    High: {}", site_summary.issues.by_severity.high);
    println!("    Medium: {}", site_summary.issues.by_severity.medium);
    println!("    Low: {}", site_summary.issues.by_severity.low);
    println!("  By Category:");
    println!("    Meta: {}", site_summary.issues.by_category.meta);
    println!("    Structure: {}", site_summary.issues.by_category.structure);
    println!("    Content: {}", site_summary.issues.by_category.content);
    println!("    Media: {}", site_summary.issues.by_category.media);
    println!("    Links: {}", site_summary.issues.by_category.links);
    
    if !site_summary.issues.by_code.is_empty() {
        println!("\nTop Issues (by frequency):");
        for (idx, issue_count) in site_summary.issues.by_code.iter().take(10).enumerate() {
            println!("  {}. {}: {} occurrences across {} pages", 
                idx + 1, 
                issue_count.code, 
                issue_count.count,
                issue_count.affected_pages);
        }
    }
    
    if !site_summary.hotspots.top_issue_codes.is_empty() {
        println!("\nHotspots (systemic issues affecting >30% of pages):");
        println!("  Templates with issues: {}", site_summary.hotspots.templates_with_issues);
        println!("  Top issue codes: {}", site_summary.hotspots.top_issue_codes.join(", "));
    }
    
    println!("\n{}", "=".repeat(80));

    // Display detailed issues grouped by file and severity (only if --verbose flag is set)
    if verbose {
        let mut has_issues = false;
        for (file_path, report) in &reports {
            if !report.issues.is_empty() {
                has_issues = true;
                println!("\n{}", file_path);
                println!("{}", "=".repeat(file_path.len().min(80)));

                // Group issues by severity
                let critical_issues: Vec<_> = report
                    .issues
                    .iter()
                    .filter(|i| matches!(i.severity, Severity::Critical))
                    .collect();
                let high_issues: Vec<_> = report
                    .issues
                    .iter()
                    .filter(|i| matches!(i.severity, Severity::High))
                    .collect();
                let medium_issues: Vec<_> = report
                    .issues
                    .iter()
                    .filter(|i| matches!(i.severity, Severity::Medium))
                    .collect();
                let low_issues: Vec<_> = report
                    .issues
                    .iter()
                    .filter(|i| matches!(i.severity, Severity::Low))
                    .collect();

                for issue in critical_issues {
                    println!("  [Critical] {}: {}", issue.code, issue.message);
                    if let Some(ref hint) = issue.hint {
                        println!("    Hint: {}", hint);
                    }
                }
                for issue in high_issues {
                    println!("  [High] {}: {}", issue.code, issue.message);
                    if let Some(ref hint) = issue.hint {
                        println!("    Hint: {}", hint);
                    }
                }
                for issue in medium_issues {
                    println!("  [Medium] {}: {}", issue.code, issue.message);
                    if let Some(ref hint) = issue.hint {
                        println!("    Hint: {}", hint);
                    }
                }
                for issue in low_issues {
                    println!("  [Low] {}: {}", issue.code, issue.message);
                    if let Some(ref hint) = issue.hint {
                        println!("    Hint: {}", hint);
                    }
                }
            }
        }

        if !has_issues {
            println!("\n✓ No SEO issues found!");
        }
    }

    // Apply fixes if --fix flag is set
    if fix {
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
                // Generate fix plans for this page's issues
                let mut page_plans = Vec::new();
                for issue in &report.issues {
                    if let Some(plan) = plan_for_issue(issue.code) {
                        // Only add if not already present (deduplicate by issue_code)
                        if !page_plans.iter().any(|p: &ai_seo::fix_plan::FixPlan| p.issue_code == plan.issue_code) {
                            page_plans.push(plan);
                        }
                    }
                }

                if !page_plans.is_empty() {
                    // Apply fixes
                    match apply_fix_plans(html_content, &page_plans) {
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

