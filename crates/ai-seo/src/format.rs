//! Output Formatting for CLI
//!
//! Provides capyseo-style formatting with improvements:
//! - Per-page grouping
//! - Inline severity markers
//! - Human-readable rule names
//! - Action hints below issues
//! - Capability hints
//! - Autofix indicators

use crate::models::{SeoIssue, SeoReport, Severity};
use crate::fix_plan::plan_for_issue;
use owo_colors::OwoColorize;

/// Get human-readable rule name from rule code
pub fn get_rule_name(code: &str) -> &'static str {
    match code {
        "SEO-META-001" => "Page Title",
        "SEO-META-002" => "Meta Description",
        "SEO-META-003" => "Meta Charset",
        "SEO-META-004" => "Meta Viewport",
        "SEO-META-005" => "Meta Description Length",
        "SEO-META-006" => "Language Attribute",
        "SEO-H1-001" => "Missing H1",
        "SEO-H1-002" => "Multiple H1 Tags",
        "SEO-H1-003" => "Skipped Heading Level",
        "SEO-H1-004" => "Empty Heading",
        "SEO-IMG-001" => "Image Missing Alt Text",
        "SEO-IMG-002" => "Image Missing Lazy Loading",
        "SEO-IMG-003" => "Image Missing Dimensions",
        "SEO-IMG-004" => "Image Format",
        "SEO-LINK-001" => "Broken External Link",
        "SEO-LINK-002" => "Internal Link",
        "SEO-CANONICAL-001" => "Canonical URL",
        "SEO-FAVICON-001" => "Favicon",
        "SEO-OG-001" => "Open Graph Tags",
        "SEO-TWITTER-001" => "Twitter Card",
        "SEO-URL-001" => "HTTPS Static Resources",
        "SEO-URL-002" => "Trailing Slash",
        "SEO-URL-003" => "URL Length",
        "SEO-URL-004" => "URL Structure",
        _ => "SEO Issue",
    }
}

/// Get severity symbol
pub fn severity_symbol(severity: &Severity) -> &'static str {
    match severity {
        Severity::Critical => "[x]",
        Severity::High => "[!]",
        Severity::Medium => "[!]",
        Severity::Low => "[i]",
    }
}

/// Get capability hint for a rule code
fn get_capability_hint(code: &str) -> Option<&'static str> {
    // Tier C (live HTTP) rules
    if code.starts_with("SEO-SEC-") || code.starts_with("SEO-LINK-") {
        Some("[live]")
    }
    // Tier B (document) rules
    else if code.starts_with("SEO-STRUCT-") {
        Some("[document]")
    }
    // Tier A (streaming) rules - no hint needed
    else {
        None
    }
}

/// Format a single issue line
pub fn format_issue(issue: &SeoIssue, show_capability: bool) -> String {
    let symbol = severity_symbol(&issue.severity);
    let rule_name = get_rule_name(issue.code);
    
    // Color the symbol based on severity
    let colored_symbol = match issue.severity {
        Severity::Critical => format!("{}", symbol.red().bold()),
        Severity::High => format!("{}", symbol.red()),
        Severity::Medium => format!("{}", symbol.yellow()),
        Severity::Low => format!("{}", symbol.bright_blue()),
    };
    
    // Color the rule name
    let colored_rule_name = match issue.severity {
        Severity::Critical => format!("{}", rule_name.red().bold()),
        Severity::High => format!("{}", rule_name.red()),
        Severity::Medium => format!("{}", rule_name.yellow()),
        Severity::Low => format!("{}", rule_name.bright_blue()),
    };
    
    // Color the issue code
    let colored_code = issue.code.bright_black();
    
    // Build capability hint if needed
    let capability_str = if show_capability {
        if let Some(hint) = get_capability_hint(issue.code) {
            format!(" {}", hint.bright_black())
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    
    let mut output = format!("{} {} ({}){}", colored_symbol, colored_rule_name, colored_code, capability_str);
    
    // Add selector location if available (similar to Cargo's error format)
    if let Some(ref selector) = issue.selector {
        let location_color = match issue.severity {
            Severity::Critical => "  --> ".bright_black(),
            Severity::High => "  --> ".bright_black(),
            Severity::Medium => "  --> ".bright_black(),
            Severity::Low => "  --> ".bright_black(),
        };
        output.push_str(&format!("\n{}{}", location_color, selector.bright_black()));
    }
    
    output
}

/// Format action hint for an issue
pub fn format_action_hint(issue: &SeoIssue) -> Option<String> {
    // Check if autofix is available
    if let Some(plan) = plan_for_issue(issue.code) {
        let risk_str = match plan.risk {
            crate::fix_plan::FixRisk::None => format!("{}", "safe".green()),
            crate::fix_plan::FixRisk::Low => format!("{}", "safe".green()),
            crate::fix_plan::FixRisk::Medium => format!("{}", "review".yellow()),
            crate::fix_plan::FixRisk::High => format!("{}", "review".yellow()),
        };
        Some(format!("    {} Autofix available ({})", "→".bright_black(), risk_str))
    } else if let Some(ref hint) = issue.hint {
        Some(format!("    {} {}", "→".bright_black(), hint.bright_black()))
    } else {
        None
    }
}

/// Format per-page report
pub fn format_page_report(file_path: &str, report: &SeoReport) -> String {
    let mut output = String::new();
    
    output.push_str(&format!("{}\n", file_path.bold().cyan()));
    
    // Color score based on value
    let score_color = if report.score.total >= 80 {
        format!("{}", report.score.total.to_string().green())
    } else if report.score.total >= 60 {
        format!("{}", report.score.total.to_string().yellow())
    } else {
        format!("{}", report.score.total.to_string().red())
    };
    output.push_str(&format!("Score: {}/100\n\n", score_color));
    
    if report.issues.is_empty() {
        output.push_str(&format!("{} No issues found\n", "✓".green().bold()));
        return output;
    }
    
    // Sort issues by severity (Critical, High, Medium, Low)
    let mut sorted_issues = report.issues.clone();
    sorted_issues.sort_by(|a, b| {
        let severity_order = |s: &Severity| match s {
            Severity::Critical => 0,
            Severity::High => 1,
            Severity::Medium => 2,
            Severity::Low => 3,
        };
        severity_order(&a.severity).cmp(&severity_order(&b.severity))
    });
    
    for issue in &sorted_issues {
        output.push_str(&format!("{}\n", format_issue(issue, true)));
        if let Some(hint) = format_action_hint(issue) {
            output.push_str(&format!("{}\n", hint));
        }
    }
    
    output
}

/// Format summary section
pub fn format_summary(
    page_count: usize,
    average_score: u8,
    error_count: usize,
    warning_count: usize,
    info_count: usize,
    autofixable_count: usize,
) -> String {
    let mut output = String::new();
    
    output.push_str(&format!("{}\n", "Summary".bold().white()));
    output.push_str(&format!("{}\n", "─".repeat(40).bright_black()));
    
    // Color average score
    let score_color = if average_score >= 80 {
        format!("{}", average_score.to_string().green())
    } else if average_score >= 60 {
        format!("{}", average_score.to_string().yellow())
    } else {
        format!("{}", average_score.to_string().red())
    };
    output.push_str(&format!("Average Score: {}/100\n", score_color));
    output.push_str(&format!("Pages Analyzed: {}\n", page_count.to_string().cyan()));
    output.push_str("Issues:\n");
    output.push_str(&format!("  Errors:   {}\n", error_count.to_string().red().bold()));
    output.push_str(&format!("  Warnings: {}\n", warning_count.to_string().yellow()));
    output.push_str(&format!("  Info:     {}\n", info_count.to_string().bright_blue()));
    output.push_str("\n");
    output.push_str(&format!("Autofixable: {}\n", autofixable_count.to_string().green()));
    let review_count = error_count + warning_count + info_count - autofixable_count;
    output.push_str(&format!("Requires review: {}\n", review_count.to_string().yellow()));
    
    output
}

