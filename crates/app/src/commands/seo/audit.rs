//! SEO audit command - analyze HTML files in build output directory

use crate::project::get_appz_directory;
use crate::session::AppzSession;
use crate::utils::build::detect_build_output_dir;
use ai_seo::{
    analyze, aggregation::SiteAggregator, 
    db,
    fix_plan::plan_for_issue,
    mutation::apply_fix_plans,
    models::SeoReport,
    scoring,
    format,
};
use futures::future;
use futures::stream::{self, StreamExt};
use itertools::Itertools;
use rusqlite::Connection;
use rayon::prelude::*;
use starbase::AppResult;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::instrument;
use walkdir::WalkDir;

#[instrument(skip_all)]
pub async fn seo_audit(
    session: AppzSession,
    paths: Vec<PathBuf>,
    dir: Option<PathBuf>,
    _verbose: bool,
    fix: bool,
    force: bool,
) -> AppResult {
    // Use the working directory from session (already respects --cwd)
    let project_path = session.working_dir.clone();

    // If paths are provided, analyze only those files/directories
    let html_files = if !paths.is_empty() {
        // Collect HTML files from provided paths
        let mut collected_files = Vec::new();
        
        for path in &paths {
            let resolved_path = if path.is_absolute() {
                path.clone()
            } else {
                project_path.join(path)
            };
            
            if !resolved_path.exists() {
                return Err(miette::miette!(
                    "Path does not exist: {}",
                    resolved_path.display()
                ));
            }
            
            if resolved_path.is_file() {
                // Single file
                if resolved_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("html"))
                    .unwrap_or(false)
                {
                    collected_files.push(resolved_path);
                } else {
                    return Err(miette::miette!(
                        "File is not an HTML file: {}",
                        resolved_path.display()
                    ));
                }
            } else if resolved_path.is_dir() {
                // Directory - find all HTML files
                let dir_files: Result<Vec<PathBuf>, walkdir::Error> = WalkDir::new(&resolved_path)
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
                
                let dir_files = dir_files
                    .map_err(|e| miette::miette!("Failed to walk directory {}: {}", resolved_path.display(), e))?;
                
                collected_files.extend(dir_files);
            } else {
                return Err(miette::miette!(
                    "Path is neither a file nor a directory: {}",
                    resolved_path.display()
                ));
            }
        }
        
        if collected_files.is_empty() {
            return Err(miette::miette!(
                "No HTML files found in provided paths"
            ));
        }
        
        collected_files
    } else {
        // No paths provided - use default directory scanning behavior
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

        // Find all HTML files recursively
        let dir_html_files: Result<Vec<PathBuf>, walkdir::Error> = WalkDir::new(&output_dir)
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

        let dir_html_files = dir_html_files
            .map_err(|e| miette::miette!("Failed to walk directory {}: {}", output_dir.display(), e))?;

        if dir_html_files.is_empty() {
            return Err(miette::miette!(
                "No HTML files found in build output directory: {}",
                output_dir.display()
            ));
        }
        
        dir_html_files
    };

    // Determine the base directory for relative paths (for display)
    let base_dir = if !paths.is_empty() {
        // Use the first path's parent or the path itself if it's a directory
        paths[0]
            .parent()
            .map(|p| if p.is_absolute() {
                p.to_path_buf()
            } else {
                project_path.join(p)
            })
            .unwrap_or_else(|| project_path.clone())
    } else {
        // Use detected output directory
        detect_build_output_dir(&project_path, dir.clone()).await?
    };

    // Open database and run migrations
    let appz_dir = get_appz_directory(&project_path);
    let db_path = appz_dir.join("seo.db");
    let db_path_str = db_path.to_string_lossy().to_string();
    
    // Ensure .appz directory exists
    tokio::fs::create_dir_all(&appz_dir)
        .await
        .map_err(|e| miette::miette!("Failed to create .appz directory: {}", e))?;
    
    // Open and migrate database (synchronous operation in async context)
    let conn = tokio::task::spawn_blocking({
        let db_path_str = db_path_str.clone();
        move || {
            let mut conn = db::open_db(&db_path_str)
                .map_err(|e| miette::miette!("Failed to open database: {}", e))?;
            db::migrate(&mut conn)
                .map_err(|e| miette::miette!("Failed to run migrations: {}", e))?;
            Ok::<Connection, miette::Error>(conn)
        }
    })
    .await
    .map_err(|e| miette::miette!("Database operation failed: {}", e))??;
    
    // Wrap connection in Arc<Mutex> for sharing across async tasks
    let db_conn = Arc::new(Mutex::new(conn));

    println!("Found {} HTML file(s) to analyze\n", html_files.len());

    // Read all files and compute hashes in parallel
    let file_data: Vec<Result<(String, String, PathBuf, String), miette::Error>> = future::join_all(
        html_files.iter().map(|file_path| {
            let base_dir = base_dir.clone();
            let file_path = file_path.clone();
            
            async move {
                // Read file
                let html_content = tokio::fs::read_to_string(&file_path)
                    .await
                    .map_err(|e| miette::miette!("Failed to read file {}: {}", file_path.display(), e))?;

                // Get relative path for display
                let relative_path = file_path
                    .strip_prefix(&base_dir)
                    .unwrap_or(&file_path)
                    .to_string_lossy()
                    .to_string();

                // Compute hash
                let hash = db::compute_hash(&html_content);

                Ok((relative_path, html_content, file_path, hash))
            }
        })
    )
    .await;

    // Calculate concurrency budget (reserve one CPU for async runtime)
    let cpu_budget = std::cmp::max(1, num_cpus::get().saturating_sub(1));
    
    // Phase 1: Prepare page data and batch check which pages should be analyzed
    let page_data: Vec<(String, String, String, PathBuf, String)> = file_data
        .into_iter()
        .map(|result| -> Result<(String, String, String, PathBuf, String), miette::Error> {
            let (relative_path, html_content, file_path, hash) = result?;
            let url = format!("file:///{}", relative_path);
            Ok((url, hash, relative_path, file_path, html_content))
        })
        .collect::<Result<Vec<_>, miette::Error>>()?;
    
    let should_analyze_map = if force {
        // If force, all pages should be analyzed
        page_data.iter()
            .map(|(url, _, _, _, _)| (url.clone(), true))
            .collect::<std::collections::HashMap<_, _>>()
    } else {
        // Batch check which pages need analysis
        tokio::task::spawn_blocking({
            let db_conn = db_conn.clone();
            let page_hashes_for_check: Vec<(String, String)> = page_data
                .iter()
                .map(|(url, hash, _, _, _)| (url.clone(), hash.clone()))
                .collect();
            move || {
                let conn = db_conn.lock().unwrap();
                db::batch_should_analyze(&conn, &page_hashes_for_check)
                    .map_err(|e| miette::miette!("Database error: {}", e))
            }
        })
        .await
        .map_err(|e| miette::miette!("Database operation failed: {}", e))??
    };
    
    // Phase 2: Process files in parallel with lock-free aggregation
    // Collect results concurrently (lock-free hot path)
    let page_results: Vec<Result<(String, SeoReport, String, PathBuf, String, String, bool), miette::Error>> = stream::iter(
        page_data.into_iter().map(|(url, hash, relative_path, file_path, html_content)| {
            let should_analyze = should_analyze_map.get(&url).copied().unwrap_or(true);
            (url, hash, relative_path, file_path, html_content, should_analyze)
        })
    )
        .map(|(url, hash, relative_path, file_path, html_content, should_analyze)| {
            async move {
                let report = if should_analyze {
                    // Analyze the page (CPU-bound, can run in parallel)
                    analyze(&html_content, &url)
                } else {
                    // Skip analysis - we'll load from DB later if needed
                    // For now, create empty report (will be loaded in batch)
                    SeoReport {
                        url: url.clone(),
                        title: None,
                        meta_description: None,
                        canonical: None,
                        word_count: 0,
                        headings: Vec::new(),
                        images: Vec::new(),
                        links: Vec::new(),
                        issues: Vec::new(),
                        score: ai_seo::models::SeoScore::default(),
                        charset: None,
                        viewport: None,
                        lang: None,
                        robots_meta: None,
                        favicon: false,
                        og_tags: std::collections::HashMap::new(),
                        twitter_card: None,
                        json_ld_scripts: Vec::new(),
                    }
                };
                
                Ok((relative_path, report, html_content, file_path, url, hash, should_analyze))
            }
        })
        .buffer_unordered(cpu_budget)
        .collect()
        .await;
    
    // Collect DB operations into plan and separate analyzed vs skipped
    let mut db_plan = db::AuditDbPlan::new();
    let mut analyzed_reports = Vec::new();
    let mut skipped_data = Vec::new();
    
    for result in page_results {
        let (relative_path, report, html_content, file_path, url, hash, should_analyze) = result?;
        
        if should_analyze {
            // Add to DB plan for batch write
            db_plan.add_page(url.clone(), hash);
            for issue in &report.issues {
                db_plan.add_issue(url.clone(), issue.code);
            }
            analyzed_reports.push((relative_path, report, html_content, file_path));
        } else {
            skipped_data.push((url, relative_path, html_content, file_path));
        }
    }
    
    // Phase 3: Execute batch DB operations
    if !db_plan.pages.is_empty() || !db_plan.issues.is_empty() {
        tokio::task::spawn_blocking({
            let db_conn = db_conn.clone();
            move || {
                let mut conn = db_conn.lock().unwrap();
                db::execute_plan(&mut *conn, db_plan)
                    .map_err(|e| miette::miette!("Failed to execute DB plan: {}", e))
            }
        })
        .await
        .map_err(|e| miette::miette!("Database operation failed: {}", e))??;
    }
    
    // Load skipped pages from DB if needed
    let mut reports = Vec::new();
    let mut file_data_for_fixing = Vec::new();
    let mut site_aggregator = SiteAggregator::new();
    
    // Add analyzed reports
    for (relative_path, report, html_content, file_path) in analyzed_reports {
        site_aggregator.ingest(&report);
        reports.push((relative_path.clone(), report));
        file_data_for_fixing.push((relative_path, html_content, file_path));
    }
    
    // Load skipped pages from DB
    if !skipped_data.is_empty() {
        let skipped_urls: Vec<String> = skipped_data.iter().map(|(url, _, _, _)| url.clone()).collect();
        let loaded_reports = tokio::task::spawn_blocking({
            let db_conn = db_conn.clone();
            let skipped_urls = skipped_urls.clone();
            move || {
                let conn = db_conn.lock().unwrap();
                let mut reports = Vec::new();
                for url in skipped_urls {
                    match db::load_issues(&conn, &url) {
                        Ok(issues) => {
                            let score = scoring::compute_score(&issues);
                            let relative_path = url.strip_prefix("file:///").unwrap_or(&url).to_string();
                            reports.push((
                                relative_path,
                                SeoReport {
                                    url: url.clone(),
                                    title: None,
                                    meta_description: None,
                                    canonical: None,
                                    word_count: 0,
                                    headings: Vec::new(),
                                    images: Vec::new(),
                                    links: Vec::new(),
                                    issues,
                                    score,
                                    charset: None,
                                    viewport: None,
                                    lang: None,
                                    robots_meta: None,
                                    favicon: false,
                                    og_tags: std::collections::HashMap::new(),
                                    twitter_card: None,
                                    json_ld_scripts: Vec::new(),
                                },
                            ));
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load issues for {}: {}", url, e);
                        }
                    }
                }
                Ok::<Vec<_>, miette::Error>(reports)
            }
        })
        .await
        .map_err(|e| miette::miette!("Database operation failed: {}", e))??;
        
        for (relative_path, report) in loaded_reports {
            site_aggregator.ingest(&report);
            reports.push((relative_path, report));
        }
        
        // Also add skipped files to file_data_for_fixing (for --fix option)
        for (_, relative_path, html_content, file_path) in skipped_data {
            file_data_for_fixing.push((relative_path, html_content, file_path));
        }
    }
    
    // Sort results for deterministic ordering (by relative_path)
    reports.sort_by(|a, b| a.0.cmp(&b.0));
    
    // Generate site-level summary
    let site_summary = site_aggregator.finalize();

    // Display per-page reports (capyseo-style format)
    for (file_path, report) in &reports {
        println!("{}", format::format_page_report(file_path, report));
    }

    // Calculate summary statistics
    let error_count = site_summary.issues.by_severity.critical;
    let warning_count = site_summary.issues.by_severity.high + site_summary.issues.by_severity.medium;
    let info_count = site_summary.issues.by_severity.low;
    
    // Count autofixable issues
    let autofixable_count = reports
        .iter()
        .flat_map(|(_, report)| &report.issues)
        .filter(|issue| plan_for_issue(issue.code).is_some())
        .count();

    // Display summary
    println!("\n{}", format::format_summary(
        site_summary.page_count,
        site_summary.score.average,
        error_count,
        warning_count,
        info_count,
        autofixable_count,
    ));

    // Apply fixes if --fix flag is set
    if fix {
        println!("\n{}", "=".repeat(80));
        println!("Applying Fixes");
        println!("{}", "=".repeat(80));
        
        // Build a map of relative_path -> report for quick lookup
        let report_map: HashMap<_, _> = reports.iter()
            .map(|(path, report)| (path.clone(), report))
            .collect();
        
        // Phase 1: Parallel HTML mutation (CPU-bound)
        let fixed_pages: Vec<Result<(String, PathBuf, String, bool), miette::Error>> = file_data_for_fixing
            .par_iter()
            .map(|(relative_path, html_content, file_path)| {
                // Find the corresponding report
                let report = report_map.get(relative_path);
                
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
                        // Apply fixes (CPU-bound, parallelized)
                        match apply_fix_plans(html_content, &page_plans) {
                            Ok(fixed_html) => {
                                let changed = fixed_html != *html_content;
                                Ok((relative_path.clone(), file_path.clone(), fixed_html, changed))
                            }
                            Err(e) => Err(miette::miette!("Error fixing {}: {}", relative_path, e)),
                        }
                    } else {
                        Ok((relative_path.clone(), file_path.clone(), html_content.clone(), false))
                    }
                } else {
                    Ok((relative_path.clone(), file_path.clone(), html_content.clone(), false))
                }
            })
            .collect();
        
        // Phase 2: Serialize file writes per directory (group by directory to avoid disk thrashing)
        let mut fixed_count = 0;
        let mut error_count = 0;
        
        // Group by directory for serialized writes
        let mut by_directory: HashMap<PathBuf, Vec<(String, PathBuf, String, bool)>> = HashMap::new();
        
        for result in fixed_pages {
            match result {
                Ok((relative_path, file_path, fixed_html, changed)) => {
                    if changed {
                        let dir = file_path.parent().unwrap_or_else(|| std::path::Path::new("."));
                        by_directory.entry(dir.to_path_buf())
                            .or_insert_with(Vec::new)
                            .push((relative_path, file_path, fixed_html, changed));
                    }
                }
                Err(e) => {
                    error_count += 1;
                    eprintln!("✗ {}", e);
                }
            }
        }
        
        // Write files serially per directory (to avoid disk thrashing)
        for (dir, files) in by_directory {
            for (relative_path, file_path, fixed_html, _) in files {
                // Write to temporary location first, then atomic rename
                let temp_path = file_path.with_extension(".seo-tmp");
                
                match tokio::fs::write(&temp_path, fixed_html).await {
                    Ok(_) => {
                        // Atomic rename on success
                        match tokio::fs::rename(&temp_path, &file_path).await {
                            Ok(_) => {
                                fixed_count += 1;
                                println!("✓ Fixed: {}", relative_path);
                            }
                            Err(e) => {
                                error_count += 1;
                                eprintln!("✗ Error renaming {}: {}", relative_path, e);
                                // Clean up temp file
                                let _ = tokio::fs::remove_file(&temp_path).await;
                            }
                        }
                    }
                    Err(e) => {
                        error_count += 1;
                        eprintln!("✗ Error writing {}: {}", relative_path, e);
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

