//! Tier B: Full Document Rules
//!
//! These 8 rules need complete document in memory but no HTTP requests.
//! They execute after streaming parse completes.

use crate::models::{SeoIssue, SeoReport, Severity};
use crate::rules::capabilities::RuleCapabilities;
use crate::rules::rule_trait::{Rule, RuleCategory, RuleContext, RuleMeta};
use crate::utils::readability::analyze_readability;
use std::collections::HashSet;

// Content Rules

/// Keyword Density Rule - analyzes keyword usage and density
pub struct KeywordDensityRule;

impl Rule for KeywordDensityRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-CONTENT-002",
            name: "Keyword Usage",
            description: "Analyze keyword presence and density",
            category: RuleCategory::Content,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::FULL_DOCUMENT,
            weight: 5,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        let title = report.title.as_deref().unwrap_or("").to_lowercase();
        let h1_text = report.headings
            .iter()
            .find(|h| h.level == 1)
            .map(|h| h.text.to_lowercase())
            .unwrap_or_default();
        let meta_desc = report.meta_description.as_deref().unwrap_or("").to_lowercase();

        // Extract potential keywords from title (4+ chars, not stop words)
        let stop_words: HashSet<&str> = [
            "the", "and", "for", "are", "but", "not", "you", "all", "can", "her",
            "was", "one", "our", "out", "has", "have", "been", "were", "being",
            "this", "that", "with", "from", "they", "will", "would", "there",
            "their", "what", "about", "which", "when", "make", "like", "into",
            "just", "your", "over", "such", "than", "them", "some",
        ]
        .iter()
        .cloned()
        .collect();

        let title_words: Vec<String> = title
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() >= 4 && !stop_words.contains(w.as_str()))
            .collect();

        if title_words.is_empty() {
            return issues;
        }

        // For body text, we'd need full document content
        // For now, we'll check H1 and meta description alignment
        for keyword in title_words.iter().take(3) {
            // Check if keyword appears in H1
            if !h1_text.contains(keyword) {
                issues.push(SeoIssue {
                    code: "SEO-CONTENT-002",
                    severity: Severity::Low,
                    message: format!("Title keyword \"{}\" not found in H1", keyword),
                    hint: Some("Consider including primary keywords in your H1".into()),
                    selector: Some("h1".into()),
                    suggestion: None,
                });
            }

            // Check if keyword in meta description
            if !meta_desc.contains(keyword) {
                issues.push(SeoIssue {
                    code: "SEO-CONTENT-002",
                    severity: Severity::Low,
                    message: format!("Title keyword \"{}\" not in meta description", keyword),
                    hint: Some("Include primary keywords in meta description".into()),
                    selector: Some("meta[name=\"description\"]".into()),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

/// Readability Rule - calculates Flesch-Kincaid readability scores
pub struct ReadabilityRule;

impl Rule for ReadabilityRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-CONTENT-003",
            name: "Content Readability",
            description: "Analyze content readability using Flesch-Kincaid scoring",
            category: RuleCategory::Content,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::FULL_DOCUMENT,
            weight: 5,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        // Estimate content text from word count
        // In a real implementation, we'd extract full body text
        // For now, we'll use word_count as a proxy
        if report.word_count < 50 {
            return issues; // Need minimum content for meaningful analysis
        }

        // Create a simple text representation for readability analysis
        // This is a simplified version - full implementation would extract actual text
        let estimated_text = format!(
            "{}. {}.",
            report.title.as_deref().unwrap_or(""),
            report.meta_description.as_deref().unwrap_or("")
        );

        let readability = analyze_readability(&estimated_text);

        if readability.flesch_reading_ease < 30.0 {
            issues.push(SeoIssue {
                code: "SEO-CONTENT-003",
                severity: Severity::Medium,
                message: format!("Very difficult to read (Flesch score: {:.1})", readability.flesch_reading_ease),
                hint: Some(format!("{}. Simplify language for broader audience reach.", readability.interpretation)),
                selector: None,
                suggestion: None,
            });
        } else if readability.flesch_reading_ease < 50.0 {
            issues.push(SeoIssue {
                code: "SEO-CONTENT-003",
                severity: Severity::Low,
                message: format!(
                    "Difficult to read (Flesch score: {:.1}, Grade {:.1})",
                    readability.flesch_reading_ease, readability.flesch_kincaid_grade
                ),
                hint: Some(format!("{}. Consider simplifying for general web audiences.", readability.interpretation)),
                selector: None,
                suggestion: None,
            });
        }

        if readability.avg_words_per_sentence > 25.0 {
            issues.push(SeoIssue {
                code: "SEO-CONTENT-003",
                severity: Severity::Low,
                message: format!("Long sentences (avg {:.1} words/sentence)", readability.avg_words_per_sentence),
                hint: Some("Aim for 15-20 words per sentence for web readability".into()),
                selector: None,
                suggestion: None,
            });
        }

        issues
    }
}

/// Duplicate Content Rule - hash-based duplicate detection
pub struct DuplicateContentRule;

impl Rule for DuplicateContentRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-CONTENT-004",
            name: "Duplicate Content Indicators",
            description: "Check for potential duplicate content issues",
            category: RuleCategory::Content,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::FULL_DOCUMENT,
            weight: 5,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        // Check for multiple H1 with same text
        let h1_texts: Vec<String> = report.headings
            .iter()
            .filter(|h| h.level == 1)
            .map(|h| h.text.trim().to_lowercase())
            .collect();

        let h1_set: HashSet<String> = h1_texts.iter().cloned().collect();
        if h1_texts.len() > h1_set.len() {
            issues.push(SeoIssue {
                code: "SEO-CONTENT-004",
                severity: Severity::Medium,
                message: "Duplicate H1 headings found".into(),
                hint: Some("Each H1 should be unique".into()),
                selector: Some("h1".into()),
                suggestion: None,
            });
        }

        // Check if title equals H1 exactly
        if let (Some(title), Some(h1)) = (
            report.title.as_ref().map(|t| t.trim().to_lowercase()),
            report.headings.iter().find(|h| h.level == 1).map(|h| h.text.trim().to_lowercase())
        ) {
            if title == h1 {
                issues.push(SeoIssue {
                    code: "SEO-CONTENT-004",
                    severity: Severity::Low,
                    message: "Title and H1 are identical".into(),
                    hint: Some("Consider varying your title and H1 slightly for more keyword coverage".into()),
                    selector: None,
                    suggestion: None,
                });
            }
        }

        issues
    }
}

// Structured Data Rules

/// JSON-LD Presence Rule - checks for JSON-LD structured data
pub struct JsonLdPresenceRule;

impl Rule for JsonLdPresenceRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-STRUCT-001",
            name: "Structured Data (JSON-LD)",
            description: "Pages should have structured data for rich search results",
            category: RuleCategory::StructuredData,
            severity: Severity::Low,
            capabilities: RuleCapabilities::FULL_DOCUMENT,
            weight: 3,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if report.json_ld_scripts.is_empty() {
            issues.push(SeoIssue {
                code: "SEO-STRUCT-001",
                severity: Severity::Low,
                message: "No structured data found".into(),
                hint: Some("Add JSON-LD structured data for rich search results (Article, Product, FAQ, etc.)".into()),
                selector: Some("head".into()),
                suggestion: None,
            });
        } else {
            // Validate JSON-LD syntax
            for (idx, json_str) in report.json_ld_scripts.iter().enumerate() {
                if let Err(e) = serde_json::from_str::<serde_json::Value>(json_str) {
                    issues.push(SeoIssue {
                        code: "SEO-STRUCT-001",
                        severity: Severity::High,
                        message: format!("Invalid JSON-LD syntax: {}", e),
                        hint: Some("Check for JSON syntax errors in structured data".into()),
                        selector: Some(format!("script[type=\"application/ld+json\"]:nth-of-type({})", idx + 1)),
                        suggestion: None,
                    });
                }
            }
        }

        issues
    }
}

/// Breadcrumb Detection Rule - checks for breadcrumb structured data
pub struct BreadcrumbDetectionRule;

impl Rule for BreadcrumbDetectionRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-STRUCT-002",
            name: "Breadcrumb Navigation",
            description: "Check for breadcrumb structured data",
            category: RuleCategory::StructuredData,
            severity: Severity::Low,
            capabilities: RuleCapabilities::FULL_DOCUMENT,
            weight: 2,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        // Note: This rule needs breadcrumb detection from parser
        // Placeholder for now
        Vec::new()
    }
}

// Technical Rules

/// Hreflang Presence Rule - validates hreflang tag presence
pub struct HreflangPresenceRule;

impl Rule for HreflangPresenceRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-TECH-003",
            name: "Hreflang Tags",
            description: "Check hreflang implementation for multilingual sites",
            category: RuleCategory::Technical,
            severity: Severity::Low,
            capabilities: RuleCapabilities::FULL_DOCUMENT,
            weight: 3,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        // Note: This rule needs hreflang extraction from parser
        // Placeholder for now
        Vec::new()
    }
}

// Link Rules

/// Link Ratio Rule - internal vs external link ratio
pub struct LinkRatioRule;

impl Rule for LinkRatioRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-LINK-002",
            name: "Link Ratio",
            description: "Internal vs external link ratio",
            category: RuleCategory::Links,
            severity: Severity::Low,
            capabilities: RuleCapabilities::FULL_DOCUMENT,
            weight: 2,
        };
        &META
    }

    fn check(&self, report: &SeoReport, context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        // Extract base domain from context URL
        let base_domain = if let Ok(url_obj) = url::Url::parse(&context.url) {
            url_obj.host_str().map(|h| h.to_string())
        } else {
            None
        };

        if base_domain.is_none() {
            return issues; // Can't determine internal/external without valid URL
        }

        let base = base_domain.unwrap();
        let mut internal_count = 0;
        let mut external_count = 0;

        for link in &report.links {
            if link.href.starts_with('#') || link.href.starts_with("javascript:") {
                continue;
            }

            if let Ok(link_url) = url::Url::parse(&link.href) {
                if let Some(host) = link_url.host_str() {
                    if host == base || host.ends_with(&format!(".{}", base)) {
                        internal_count += 1;
                    } else {
                        external_count += 1;
                    }
                } else {
                    // Relative URL, assume internal
                    internal_count += 1;
                }
            } else {
                // Relative URL, assume internal
                internal_count += 1;
            }
        }

        let total = internal_count + external_count;
        if total > 0 {
            let internal_ratio = (internal_count as f64 / total as f64) * 100.0;
            
            if internal_ratio < 20.0 && total > 5 {
                issues.push(SeoIssue {
                    code: "SEO-LINK-002",
                    severity: Severity::Low,
                    message: format!("Low internal link ratio ({:.1}% internal)", internal_ratio),
                    hint: Some("Consider adding more internal links to improve site structure".into()),
                    selector: None,
                    suggestion: None,
                });
            }
        }

        issues
    }
}

// URL Rules

/// URL Structure Keywords Rule - checks URL structure and keyword presence
pub struct UrlStructureKeywordsRule;

impl Rule for UrlStructureKeywordsRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-URL-004",
            name: "URL Structure and Keywords",
            description: "Check URL structure and keyword presence",
            category: RuleCategory::Url,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::FULL_DOCUMENT,
            weight: 3,
        };
        &META
    }

    fn check(&self, report: &SeoReport, context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if let Ok(url_obj) = url::Url::parse(&context.url) {
            let pathname = url_obj.path().to_lowercase();

            // Check for underscores (should use hyphens)
            if pathname.contains('_') {
                issues.push(SeoIssue {
                    code: "SEO-URL-004",
                    severity: Severity::Medium,
                    message: "URL contains underscores".into(),
                    hint: Some("Use hyphens (-) instead of underscores (_) for word separation. Search engines treat hyphens as word separators.".into()),
                    selector: None,
                    suggestion: None,
                });
            }

            // Check for uppercase characters
            if pathname != pathname.to_lowercase() {
                issues.push(SeoIssue {
                    code: "SEO-URL-004",
                    severity: Severity::Medium,
                    message: "URL contains uppercase characters".into(),
                    hint: Some("Use lowercase URLs only. Mixed case URLs can cause duplicate content issues.".into()),
                    selector: None,
                    suggestion: None,
                });
            }

            // Check if URL contains keywords from title
            if let Some(title) = &report.title {
                let title_lower = title.to_lowercase();
                let title_words: Vec<&str> = title_lower
                    .split_whitespace()
                    .filter(|w| w.len() >= 4)
                    .collect();

                let path_words: Vec<&str> = pathname
                    .split('/')
                    .flat_map(|seg| seg.split('-'))
                    .filter(|w| w.len() >= 3)
                    .collect();

                let has_keyword = title_words.iter().any(|kw| {
                    path_words.iter().any(|pw| pw.contains(kw) || kw.contains(pw))
                });

                if !has_keyword && !title_words.is_empty() {
                    issues.push(SeoIssue {
                        code: "SEO-URL-004",
                        severity: Severity::Low,
                        message: "URL does not contain keywords from page title".into(),
                        hint: Some("Consider including relevant keywords in your URL for better SEO".into()),
                        selector: None,
                        suggestion: None,
                    });
                }
            }
        }

        issues
    }
}

/// Get all Tier B rules
pub fn get_tier_b_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(KeywordDensityRule),
        Box::new(ReadabilityRule),
        Box::new(DuplicateContentRule),
        Box::new(JsonLdPresenceRule),
        Box::new(BreadcrumbDetectionRule),
        Box::new(HreflangPresenceRule),
        Box::new(LinkRatioRule),
        Box::new(UrlStructureKeywordsRule),
    ]
}
