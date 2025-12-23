//! Tier C: Live HTTP Rules
//!
//! These 6 rules require HTTP requests and execute in a separate pass.
//! They include SSRF protection, request budgets, and caching.

use crate::models::{SeoIssue, SeoReport, Severity};
use crate::rules::capabilities::RuleCapabilities;
use crate::rules::rule_trait::{Rule, RuleCategory, RuleContext, RuleMeta};

/// Broken Links Rule - checks for broken internal/external links
pub struct BrokenLinksRule;

impl Rule for BrokenLinksRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-LINK-003",
            name: "Broken Links",
            description: "Check for broken internal and external links",
            category: RuleCategory::Links,
            severity: Severity::High,
            capabilities: RuleCapabilities::LIVE_HTTP,
            weight: 10,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        // This rule requires HTTP client - will be implemented in Phase 7
        Vec::new()
    }
}

/// Redirect Chains Rule - detects redirect chains
pub struct RedirectChainsRule;

impl Rule for RedirectChainsRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-LINK-004",
            name: "Redirect Chains",
            description: "Detect redirect chains",
            category: RuleCategory::Links,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::LIVE_HTTP,
            weight: 5,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        // This rule requires HTTP client - will be implemented in Phase 7
        Vec::new()
    }
}

/// Security Headers Rule - checks security headers
pub struct SecurityHeadersRule;

impl Rule for SecurityHeadersRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-SEC-001",
            name: "Security Headers",
            description: "Check security headers (CSP, HSTS, etc.)",
            category: RuleCategory::Security,
            severity: Severity::High,
            capabilities: RuleCapabilities::LIVE_HTTP,
            weight: 10,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        // This rule requires HTTP client - will be implemented in Phase 7
        Vec::new()
    }
}

/// Mixed Content Rule - detects mixed HTTP/HTTPS content
pub struct MixedContentRule;

impl Rule for MixedContentRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-SEC-002",
            name: "Mixed Content",
            description: "Detect mixed HTTP/HTTPS content",
            category: RuleCategory::Security,
            severity: Severity::High,
            capabilities: RuleCapabilities::LIVE_HTTP,
            weight: 10,
        };
        &META
    }

    fn check(&self, report: &SeoReport, context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        // Check if page is HTTPS but has HTTP resources
        if context.url.starts_with("https://") {
            // Check images
            for img in &report.images {
                if img.src.starts_with("http://") {
                    issues.push(SeoIssue {
                        code: "SEO-SEC-002",
                        severity: Severity::High,
                        message: format!("Mixed content: HTTP image on HTTPS page: {}", img.src),
                        hint: Some("Use HTTPS for all resources on HTTPS pages".into()),
                        selector: Some(format!("img[src=\"{}\"]", img.src)),
                        suggestion: None,
                    });
                }
            }

            // Check links
            for link in &report.links {
                if link.href.starts_with("http://") {
                    issues.push(SeoIssue {
                        code: "SEO-SEC-002",
                        severity: Severity::High,
                        message: format!("Mixed content: HTTP link on HTTPS page: {}", link.href),
                        hint: Some("Use HTTPS for all links on HTTPS pages".into()),
                        selector: Some(format!("a[href=\"{}\"]", link.href)),
                        suggestion: None,
                    });
                }
            }
        }

        issues
    }
}

/// External Scripts Rule - analyzes external script domains
pub struct ExternalScriptsRule;

impl Rule for ExternalScriptsRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-SEC-003",
            name: "External Scripts",
            description: "Analyze external script domains",
            category: RuleCategory::Security,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::LIVE_HTTP,
            weight: 5,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        // This rule requires script extraction from parser
        // Placeholder for now
        Vec::new()
    }
}

/// Form Security Rule - checks form action security
pub struct FormSecurityRule;

impl Rule for FormSecurityRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-SEC-004",
            name: "Form Security",
            description: "Check form action security",
            category: RuleCategory::Security,
            severity: Severity::High,
            capabilities: RuleCapabilities::LIVE_HTTP,
            weight: 10,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        // This rule requires form extraction from parser
        // Placeholder for now
        Vec::new()
    }
}

/// Get all Tier C rules
pub fn get_tier_c_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(BrokenLinksRule),
        Box::new(RedirectChainsRule),
        Box::new(SecurityHeadersRule),
        Box::new(MixedContentRule),
        Box::new(ExternalScriptsRule),
        Box::new(FormSecurityRule),
    ]
}
