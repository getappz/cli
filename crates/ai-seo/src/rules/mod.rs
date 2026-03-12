//! SEO Rules Module
//!
//! Organized into tiers based on execution requirements:
//! - Tier A: Streaming-safe rules (can execute during HTML parse)
//! - Tier B: Full document rules (need complete document in memory)
//! - Tier C: Live HTTP rules (require network requests)

pub mod capabilities;
pub mod rule_trait;

// Tier modules
pub mod tier_a_streaming;
pub mod tier_b_document;
pub mod tier_c_live;

// Re-export for convenience
pub use capabilities::RuleCapabilities;
pub use rule_trait::{Rule, RuleCategory, RuleContext, RuleMeta};

// Legacy function for backward compatibility
// This will be replaced by the new rule system
use crate::models::*;

pub fn apply_rules(report: &mut SeoReport) {
    // This is the old rule application logic
    // It will be replaced by the new rule system that uses capabilities
    
    if report.title.is_none() {
        report.issues.push(SeoIssue {
            code: "SEO-META-001",
            severity: Severity::Critical,
            message: "Missing <title> tag".into(),
            hint: Some("Add a 50–60 character title".into()),
            selector: None,
            suggestion: None,
        });
    }

    if report.meta_description.is_none() {
        report.issues.push(SeoIssue {
            code: "SEO-META-002",
            severity: Severity::High,
            message: "Missing meta description".into(),
            hint: Some("Add a 140–160 character description".into()),
            selector: None,
            suggestion: None,
        });
    }

    let h1_count = report.headings.iter().filter(|h| h.level == 1).count();
    if h1_count == 0 {
        report.issues.push(SeoIssue {
            code: "SEO-H1-001",
            severity: Severity::Critical,
            message: "No H1 found".into(),
            hint: Some("Add exactly one H1".into()),
            selector: None,
            suggestion: None,
        });
    } else if h1_count > 1 {
        report.issues.push(SeoIssue {
            code: "SEO-H1-002",
            severity: Severity::Critical,
            message: "Multiple H1 tags found".into(),
            hint: Some("Keep exactly one H1".into()),
            selector: None,
            suggestion: None,
        });
    }

    if report.word_count < 300 {
        report.issues.push(SeoIssue {
            code: "SEO-CONTENT-001",
            severity: Severity::Medium,
            message: "Thin content".into(),
            hint: Some("Expand content to at least 300 words".into()),
            selector: None,
            suggestion: None,
        });
    }

    for img in &report.images {
        if img.alt.as_deref().unwrap_or("").is_empty() {
            report.issues.push(SeoIssue {
                code: "SEO-IMG-001",
                severity: Severity::Medium,
                message: format!("Image {} missing alt text", img.src),
                hint: Some("Add descriptive alt text".into()),
                selector: Some(format!("img[src=\"{}\"]", img.src)),
                suggestion: None,
            });
        }
        if img.loading.is_none() {
            report.issues.push(SeoIssue {
                code: "SEO-IMG-002",
                severity: Severity::Low,
                message: format!("Image {} missing loading attribute", img.src),
                hint: Some("Use loading=lazy".into()),
                selector: Some(format!("img[src=\"{}\"]", img.src)),
                suggestion: None,
            });
        }
    }

    for link in &report.links {
        if link.text.trim().is_empty() {
            report.issues.push(SeoIssue {
                code: "SEO-LINK-001",
                severity: Severity::Low,
                message: format!("Link {} has empty text", link.href),
                hint: Some("Use descriptive anchor text".into()),
                selector: Some(format!("a[href=\"{}\"]", link.href)),
                suggestion: None,
            });
        }
    }
}

