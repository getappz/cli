//! Fix Preview Data Model and Formatting
//!
//! This module provides the canonical Fix Preview data model and formatting
//! for CLI, Web UI, and API/JSON outputs. This is the single source of truth
//! for fix preview UX across all surfaces.

use crate::diff::DryRunDiff;
use crate::fix_plan::FixRisk;
use crate::models::{Severity, SiteSeoSummary};
use crate::FixRouting;
use serde::Serialize;
use std::collections::HashMap;
use owo_colors::OwoColorize;

/// Canonical Fix Preview data model
///
/// This is the single source of truth for fix previews across CLI, Web UI, and API.
#[derive(Debug, Clone, Serialize)]
pub struct FixPreview {
    pub summary: FixPreviewSummary,
    pub fixes: Vec<FixPreviewItem>,
}

/// Summary statistics for the fix preview
#[derive(Debug, Clone, Serialize)]
pub struct FixPreviewSummary {
    pub total_issues: usize,
    pub fixes_planned: usize,
    pub pages_affected: usize,
    pub routing: RoutingBreakdown,
}

/// Breakdown of fixes by routing level
#[derive(Debug, Clone, Serialize)]
pub struct RoutingBreakdown {
    pub template: usize,
    pub section: usize,
    pub page: usize,
}

/// Individual fix preview item
#[derive(Debug, Clone, Serialize)]
pub struct FixPreviewItem {
    pub issue_code: &'static str,
    pub severity: Severity,
    pub routing: String,
    pub reason: String,
    pub actions: Vec<String>,
    pub affected_pages: usize,
    pub risk: FixRisk,
    pub ai_used: bool,
    pub ai_scope: Option<String>,
    pub diffs: Vec<FixDiff>,
}

/// Diff preview for a fix
#[derive(Debug, Clone, Serialize)]
pub struct FixDiff {
    pub url: String,
    pub before: String,
    pub after: String,
}

/// CI-safe fix preview output
#[derive(Debug, Clone, Serialize)]
pub struct CiFixPreview {
    pub allowed: bool,
    pub template_fixes: usize,
    pub section_fixes: usize,
    pub page_fixes: usize,
    pub ai_used: bool,
    pub risk: String,
}

/// Build a fix preview from site summary and fix plans
pub fn build_fix_preview(
    site: &SiteSeoSummary,
    site_fix_plans: &[crate::fix_plan::SiteFixPlan],
    diffs_by_issue: &HashMap<&'static str, Vec<DryRunDiff>>,
) -> FixPreview {
    let mut routing_breakdown = RoutingBreakdown {
        template: 0,
        section: 0,
        page: 0,
    };

    let mut fixes = Vec::new();
    let mut total_pages_affected = 0;

    for site_plan in site_fix_plans {
        // Count routing breakdown
        match &site_plan.routing {
            FixRouting::Template => routing_breakdown.template += 1,
            FixRouting::Section { .. } => routing_breakdown.section += 1,
            FixRouting::Page => routing_breakdown.page += 1,
        }

        // Get severity from registry
        let severity = crate::registry::lookup(site_plan.issue)
            .map(|def| def.severity.clone())
            .unwrap_or(Severity::Medium);

        // Format routing string
        let routing_str = match &site_plan.routing {
            FixRouting::Template => "Template".to_string(),
            FixRouting::Section { prefix } => format!("Section: {}", prefix),
            FixRouting::Page => "Page".to_string(),
        };

        // Format reason
        let ratio = if site.page_count > 0 {
            (site_plan.affected_pages as f32 / site.page_count as f32) * 100.0
        } else {
            0.0
        };
        let reason = match &site_plan.routing {
            FixRouting::Template => format!("Affects {} / {} pages ({:.0}%)", 
                site_plan.affected_pages, site.page_count, ratio),
            FixRouting::Section { prefix } => format!("Affects {} pages in {}", 
                site_plan.affected_pages, prefix),
            FixRouting::Page => format!("Affects {} page(s)", site_plan.affected_pages),
        };

        // Format actions
        let actions: Vec<String> = site_plan.plan.actions.iter().map(|action| {
            match action {
                crate::fix_plan::FixAction::InsertMeta { name, .. } => {
                    format!("Insert <meta name=\"{}\">", name)
                }
                crate::fix_plan::FixAction::UpdateTitle { .. } => {
                    "Update <title> tag".to_string()
                }
                crate::fix_plan::FixAction::EnsureSingleH1 => {
                    "Ensure exactly one H1 exists".to_string()
                }
                crate::fix_plan::FixAction::AddImageAlt => {
                    "Add alt text to images".to_string()
                }
                crate::fix_plan::FixAction::AddLazyLoading => {
                    "Add lazy loading to images".to_string()
                }
            }
        }).collect();

        // Get diffs for this issue
        let diffs: Vec<FixDiff> = diffs_by_issue
            .get(site_plan.issue)
            .map(|diffs| {
                diffs.iter().flat_map(|diff| {
                    diff.hunks.iter().map(|hunk| FixDiff {
                        url: diff.url.clone(),
                        before: hunk.before.clone(),
                        after: hunk.after.clone(),
                    })
                }).collect()
            })
            .unwrap_or_default();

        // Determine AI scope
        let ai_scope = if site_plan.plan.ai_allowed {
            match &site_plan.plan.actions[0] {
                crate::fix_plan::FixAction::InsertMeta { name, content_hint } => {
                    Some(format!("Meta {}: {}", name, content_hint))
                }
                crate::fix_plan::FixAction::UpdateTitle { strategy } => {
                    match strategy {
                        crate::fix_plan::TitleStrategy::AiRewrite { max_chars } => {
                            Some(format!("Title rewrite (max {} chars)", max_chars))
                        }
                        _ => None,
                    }
                }
                crate::fix_plan::FixAction::AddImageAlt => {
                    Some("Image alt text generation".to_string())
                }
                _ => None,
            }
        } else {
            None
        };

        total_pages_affected = total_pages_affected.max(site_plan.affected_pages);

        fixes.push(FixPreviewItem {
            issue_code: site_plan.issue,
            severity,
            routing: routing_str,
            reason,
            actions,
            affected_pages: site_plan.affected_pages,
            risk: site_plan.plan.risk.clone(),
            ai_used: site_plan.plan.ai_allowed,
            ai_scope,
            diffs,
        });
    }

    FixPreview {
        summary: FixPreviewSummary {
            total_issues: site.issues.by_code.len(),
            fixes_planned: fixes.len(),
            pages_affected: total_pages_affected,
            routing: routing_breakdown,
        },
        fixes,
    }
}

/// Build CI-safe fix preview
pub fn build_ci_preview(preview: &FixPreview) -> CiFixPreview {
    let ai_used = preview.fixes.iter().any(|f| f.ai_used);
    
    // Determine overall risk (highest risk level)
    let risk_level = preview.fixes.iter()
        .map(|f| match f.risk {
            FixRisk::High => 3,
            FixRisk::Medium => 2,
            FixRisk::Low => 1,
            FixRisk::None => 0,
        })
        .max()
        .unwrap_or(0);
    
    let risk_str = match risk_level {
        3 => "high",
        2 => "medium",
        1 => "low",
        _ => "none",
    };

    CiFixPreview {
        allowed: true, // Can be gated by CI rules
        template_fixes: preview.summary.routing.template,
        section_fixes: preview.summary.routing.section,
        page_fixes: preview.summary.routing.page,
        ai_used,
        risk: risk_str.to_string(),
    }
}

/// Format fix preview for CLI output
pub fn format_cli_preview(preview: &FixPreview) -> String {
    let mut output = String::new();
    
    // Header
    output.push_str(&format!("{}\n\n", "──────────────── SEO FIX PREVIEW ────────────────".bold().cyan()));
    
    // Summary
    output.push_str(&format!("{}\n", "Summary".bold()));
    output.push_str(&format!("{} Issues detected:        {}\n", "•".bright_black(), preview.summary.total_issues.to_string().yellow()));
    output.push_str(&format!("{} Fixes planned:          {}\n", "•".bright_black(), preview.summary.fixes_planned.to_string().green()));
    output.push_str(&format!("{} Pages affected:         {}\n", "•".bright_black(), preview.summary.pages_affected.to_string().cyan()));
    output.push_str(&format!("{} Template fixes:         {}\n", "•".bright_black(), preview.summary.routing.template.to_string().cyan()));
    output.push_str(&format!("{} Section fixes:          {}\n", "•".bright_black(), preview.summary.routing.section.to_string().cyan()));
    output.push_str(&format!("{} Page fixes:             {}\n\n", "•".bright_black(), preview.summary.routing.page.to_string().cyan()));
    
    // Fix plan details
    output.push_str(&format!("{}\n\n", "──────────────── FIX PLAN ────────────────".bold().cyan()));
    
    for fix in &preview.fixes {
        // Get issue name from registry
        let issue_name = match fix.issue_code {
            "SEO-META-001" => "Missing or invalid title tag",
            "SEO-META-002" => "Missing meta description",
            "SEO-H1-001" => "Missing H1 tag",
            "SEO-H1-002" => "Multiple H1 tags",
            "SEO-IMG-001" => "Images missing alt text",
            "SEO-IMG-002" => "Images missing lazy loading",
            _ => "SEO issue",
        };
        
        // Color issue code and name based on severity
        let (colored_code, colored_name) = match fix.severity {
            Severity::Critical => (format!("{}", fix.issue_code.red().bold()), format!("{}", issue_name.red().bold())),
            Severity::High => (format!("{}", fix.issue_code.red()), format!("{}", issue_name.red())),
            Severity::Medium => (format!("{}", fix.issue_code.yellow()), format!("{}", issue_name.yellow())),
            Severity::Low => (format!("{}", fix.issue_code.bright_blue()), format!("{}", issue_name.bright_blue())),
        };
        
        output.push_str(&format!("[{}] {}\n", colored_code, colored_name));
        
        // Color severity
        let severity_str = format!("{:?}", fix.severity);
        let colored_severity = match fix.severity {
            Severity::Critical => format!("{}", severity_str.red().bold()),
            Severity::High => format!("{}", severity_str.red()),
            Severity::Medium => format!("{}", severity_str.yellow()),
            Severity::Low => format!("{}", severity_str.bright_blue()),
        };
        output.push_str(&format!("Severity: {}\n", colored_severity));
        output.push_str(&format!("Routing: {}\n", fix.routing.cyan()));
        output.push_str(&format!("Reason: {}\n\n", fix.reason.bright_black()));
        
        output.push_str(&format!("{}\n", "Action:".bold()));
        for action in &fix.actions {
            output.push_str(&format!("{} {}\n", "•".bright_black(), action));
        }
        output.push_str("\n");
        
        // Show diff preview if available
        if !fix.diffs.is_empty() {
            output.push_str(&format!("{}\n", "Preview:".bold()));
            // Show first diff as example
            if let Some(diff) = fix.diffs.first() {
                if !diff.before.is_empty() {
                    output.push_str(&format!("{} {}\n", "-".red(), diff.before.red()));
                }
                if !diff.after.is_empty() {
                    output.push_str(&format!("{} {}\n", "+".green(), diff.after.green()));
                }
            }
            output.push_str("\n");
        }
        
        // Impact
        output.push_str(&format!("{}\n", "Impact:".bold()));
        output.push_str(&format!("{} Fixes {} page(s)\n", "✓".green().bold(), fix.affected_pages.to_string().cyan()));
        
        // Color risk
        let risk_str = format!("{:?}", fix.risk);
        let colored_risk = match fix.risk {
            FixRisk::None => format!("{}", risk_str.green()),
            FixRisk::Low => format!("{}", risk_str.green()),
            FixRisk::Medium => format!("{}", risk_str.yellow()),
            FixRisk::High => format!("{}", risk_str.yellow()),
        };
        output.push_str(&format!("{} Risk: {}\n", "✓".green().bold(), colored_risk));
        if fix.ai_used {
            output.push_str(&format!("{} {}\n", "✓".green().bold(), "AI-assisted".magenta()));
            if let Some(ref scope) = fix.ai_scope {
                output.push_str(&format!("  Scope: {}\n", scope.magenta()));
            }
        } else {
            output.push_str(&format!("{} {}\n", "✓".green().bold(), "Zero content rewrite".green()));
        }
        output.push_str(&format!("{} {}\n", "✓".green().bold(), "Idempotent".green()));
        
        output.push_str(&format!("\n{}\n\n", "──────────────────────────────────────────".bright_black()));
    }
    
    // Commands
    output.push_str(&format!("{}\n", "Run with:".bold()));
    output.push_str(&format!("  {}        {}\n", "--apply".green().bold(), "Apply fixes"));
    output.push_str(&format!("  {}   {}\n", "--scope=page".yellow(), "Downgrade template fix"));
    output.push_str(&format!("  {}    {}\n", "--skip=CODE".yellow(), "Skip specific issue"));
    
    output
}

