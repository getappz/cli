//! Tier A: Streaming-Safe Rules
//!
//! These 18 rules can execute during streaming HTML parse.
//! They require no HTTP requests and no full document context.

use crate::models::{SeoIssue, SeoReport, Severity};
use crate::rules::capabilities::RuleCapabilities;
use crate::rules::rule_trait::{Rule, RuleCategory, RuleContext, RuleMeta};

// Meta Rules

/// Meta Title Rule - checks for title presence and length
pub struct MetaTitleRule;

impl Rule for MetaTitleRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-META-001",
            name: "Page Title",
            description: "Pages should have a unique title between 30-60 characters",
            category: RuleCategory::Meta,
            severity: Severity::Critical,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 20,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        match &report.title {
            None => {
                issues.push(SeoIssue {
                    code: "SEO-META-001",
                    severity: Severity::Critical,
                    message: "Missing <title> tag".into(),
                    hint: Some("Add a 50–60 character title".into()),
                    selector: Some("head".into()),
                    suggestion: None,
                });
            }
            Some(title) => {
                let len = title.len();
                if len < 30 {
                    issues.push(SeoIssue {
                        code: "SEO-META-001",
                        severity: Severity::High,
                        message: format!("Title too short ({} chars, minimum 30)", len),
                        hint: Some("Expand the title with more descriptive keywords".into()),
                        selector: Some("title".into()),
                        suggestion: None,
                    });
                } else if len > 60 {
                    issues.push(SeoIssue {
                        code: "SEO-META-001",
                        severity: Severity::High,
                        message: format!("Title too long ({} chars, maximum 60)", len),
                        hint: Some("Shorten the title to prevent truncation in search results".into()),
                        selector: Some("title".into()),
                        suggestion: None,
                    });
                }
            }
        }

        issues
    }
}

/// Meta Description Rule - checks for meta description presence and length
pub struct MetaDescriptionRule;

impl Rule for MetaDescriptionRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-META-002",
            name: "Meta Description",
            description: "Pages should have a meta description between 120-160 characters",
            category: RuleCategory::Meta,
            severity: Severity::High,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 10,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        match &report.meta_description {
            None => {
                issues.push(SeoIssue {
                    code: "SEO-META-002",
                    severity: Severity::High,
                    message: "Missing meta description".into(),
                    hint: Some("Add a 140–160 character description".into()),
                    selector: Some("head".into()),
                    suggestion: None,
                });
            }
            Some(desc) => {
                let len = desc.len();
                if len < 120 {
                    issues.push(SeoIssue {
                        code: "SEO-META-002",
                        severity: Severity::Medium,
                        message: format!("Meta description too short ({} chars, minimum 120)", len),
                        hint: Some("Expand the description to 120-160 characters".into()),
                        selector: Some("meta[name=\"description\"]".into()),
                        suggestion: None,
                    });
                } else if len > 160 {
                    issues.push(SeoIssue {
                        code: "SEO-META-002",
                        severity: Severity::Medium,
                        message: format!("Meta description too long ({} chars, maximum 160)", len),
                        hint: Some("Shorten the description to prevent truncation".into()),
                        selector: Some("meta[name=\"description\"]".into()),
                        suggestion: None,
                    });
                }
            }
        }

        issues
    }
}

/// Meta Charset Rule - checks for UTF-8 charset declaration
pub struct MetaCharsetRule;

impl Rule for MetaCharsetRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-META-003",
            name: "Character Encoding",
            description: "Pages should declare UTF-8 character encoding",
            category: RuleCategory::Meta,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 5,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if report.charset.is_none() {
            issues.push(SeoIssue {
                code: "SEO-META-003",
                severity: Severity::Medium,
                message: "Missing character encoding declaration".into(),
                hint: Some("Add <meta charset=\"UTF-8\"> as the first element in <head>".into()),
                selector: Some("head".into()),
                suggestion: None,
            });
        } else if let Some(charset) = &report.charset {
            let charset_lower = charset.to_lowercase();
            if charset_lower != "utf-8" && charset_lower != "utf8" {
                issues.push(SeoIssue {
                    code: "SEO-META-003",
                    severity: Severity::Low,
                    message: format!("Non-UTF-8 encoding detected: {}", charset),
                    hint: Some("Consider using UTF-8 for better compatibility".into()),
                    selector: Some("meta[charset]".into()),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

/// Meta Viewport Rule - checks for viewport meta tag
pub struct MetaViewportRule;

impl Rule for MetaViewportRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-META-004",
            name: "Viewport Meta",
            description: "Pages should have a viewport meta tag for mobile responsiveness",
            category: RuleCategory::Meta,
            severity: Severity::Critical,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 15,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if report.viewport.is_none() {
            issues.push(SeoIssue {
                code: "SEO-META-004",
                severity: Severity::Critical,
                message: "Missing viewport meta tag".into(),
                hint: Some("Add <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">".into()),
                selector: Some("head".into()),
                suggestion: None,
            });
        } else if let Some(viewport) = &report.viewport {
            if !viewport.to_lowercase().contains("width=device-width") {
                issues.push(SeoIssue {
                    code: "SEO-META-004",
                    severity: Severity::High,
                    message: "Viewport should include width=device-width".into(),
                    hint: Some("Use: <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">".into()),
                    selector: Some("meta[name=\"viewport\"]".into()),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

/// Lang Attribute Rule - checks for lang attribute on HTML element
pub struct LangAttributeRule;

impl Rule for LangAttributeRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-META-005",
            name: "Language Attribute",
            description: "HTML element should have a lang attribute",
            category: RuleCategory::Meta,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 5,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if report.lang.is_none() {
            issues.push(SeoIssue {
                code: "SEO-META-005",
                severity: Severity::Medium,
                message: "Missing lang attribute on <html> element".into(),
                hint: Some("Add lang attribute, e.g., <html lang=\"en\">".into()),
                selector: Some("html".into()),
                suggestion: None,
            });
        }

        issues
    }
}

/// Robots Meta Rule - checks for robots meta tags
pub struct RobotsMetaRule;

impl Rule for RobotsMetaRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-META-006",
            name: "Robots Meta",
            description: "Check for blocking robots meta tags",
            category: RuleCategory::Meta,
            severity: Severity::Low,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 3,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if let Some(robots) = &report.robots_meta {
            let robots_lower = robots.to_lowercase();
            if robots_lower.contains("noindex") {
                issues.push(SeoIssue {
                    code: "SEO-META-006",
                    severity: Severity::High,
                    message: "Page has noindex meta tag - will not be indexed by search engines".into(),
                    hint: Some("Remove noindex if you want this page to appear in search results".into()),
                    selector: Some("meta[name=\"robots\"]".into()),
                    suggestion: None,
                });
            }
            if robots_lower.contains("nofollow") {
                issues.push(SeoIssue {
                    code: "SEO-META-006",
                    severity: Severity::Medium,
                    message: "Page has nofollow meta tag - links will not be followed".into(),
                    hint: Some("Remove nofollow if you want search engines to follow links on this page".into()),
                    selector: Some("meta[name=\"robots\"]".into()),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

// Heading Rules

/// Single H1 Rule - ensures exactly one H1
pub struct SingleH1Rule;

impl Rule for SingleH1Rule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-H1-001",
            name: "Single H1",
            description: "Pages should have exactly one H1 heading",
            category: RuleCategory::Structure,
            severity: Severity::Critical,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 20,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();
        let h1_count = report.headings.iter().filter(|h| h.level == 1).count();

        if h1_count == 0 {
            issues.push(SeoIssue {
                code: "SEO-H1-001",
                severity: Severity::Critical,
                message: "No H1 found".into(),
                hint: Some("Add exactly one H1".into()),
                selector: None,
                suggestion: None,
            });
        } else if h1_count > 1 {
            issues.push(SeoIssue {
                code: "SEO-H1-002",
                severity: Severity::Critical,
                message: format!("Multiple H1 tags found ({})", h1_count),
                hint: Some("Keep exactly one H1".into()),
                selector: None,
                suggestion: None,
            });
        }

        issues
    }
}

/// Heading Hierarchy Rule - validates proper H1→H2→H3 hierarchy
pub struct HeadingHierarchyRule;

impl Rule for HeadingHierarchyRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-H1-003",
            name: "Heading Hierarchy",
            description: "Pages should have proper heading hierarchy starting with H1",
            category: RuleCategory::Structure,
            severity: Severity::High,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 10,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        // Check for skipped levels
        for i in 1..report.headings.len() {
            let current = &report.headings[i];
            let previous = &report.headings[i - 1];

            if current.level > previous.level + 1 {
                issues.push(SeoIssue {
                    code: "SEO-H1-003",
                    severity: Severity::High,
                    message: format!("Skipped heading level: H{} → H{}", previous.level, current.level),
                    hint: Some(format!("Use H{} instead of H{}", previous.level + 1, current.level)),
                    selector: Some(format!("h{}", current.level)),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

/// Empty Heading Rule - detects empty headings
pub struct EmptyHeadingRule;

impl Rule for EmptyHeadingRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-H1-004",
            name: "Empty Headings",
            description: "Headings should not be empty",
            category: RuleCategory::Structure,
            severity: Severity::Critical,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 15,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        for heading in &report.headings {
            if heading.text.trim().is_empty() {
                issues.push(SeoIssue {
                    code: "SEO-H1-004",
                    severity: Severity::Critical,
                    message: format!("Empty H{} heading found", heading.level),
                    hint: Some("Add meaningful text or remove the empty heading".into()),
                    selector: Some(format!("h{}", heading.level)),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

// Image Rules

/// Image Alt Rule - checks for alt text on images
pub struct ImageAltRule;

impl Rule for ImageAltRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-IMG-001",
            name: "Image Alt Text",
            description: "All content images should have descriptive alt text",
            category: RuleCategory::Media,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 5,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        for img in &report.images {
            if img.alt.as_deref().unwrap_or("").is_empty() {
                issues.push(SeoIssue {
                    code: "SEO-IMG-001",
                    severity: Severity::Medium,
                    message: format!("Image {} missing alt text", img.src),
                    hint: Some("Add descriptive alt text".into()),
                    selector: Some(format!("img[src=\"{}\"]", img.src)),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

/// Image Lazy Load Rule - checks for lazy loading on images
pub struct ImageLazyLoadRule;

impl Rule for ImageLazyLoadRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-IMG-002",
            name: "Image Lazy Loading",
            description: "Below-the-fold images should use lazy loading",
            category: RuleCategory::Media,
            severity: Severity::Low,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 3,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        // Skip first 2 images (likely above fold)
        for img in report.images.iter().skip(2) {
            if img.loading.is_none() {
                issues.push(SeoIssue {
                    code: "SEO-IMG-002",
                    severity: Severity::Low,
                    message: format!("Image {} missing loading attribute", img.src),
                    hint: Some("Use loading=lazy".into()),
                    selector: Some(format!("img[src=\"{}\"]", img.src)),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

/// Image Dimensions Rule - checks for width/height attributes
pub struct ImageDimensionsRule;

impl Rule for ImageDimensionsRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-IMG-003",
            name: "Image Dimensions",
            description: "Images should specify width and height to prevent layout shift",
            category: RuleCategory::Media,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 5,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        // Note: This rule needs width/height extraction from parser
        // Placeholder for now
        Vec::new()
    }
}

/// Image Format Rule - checks for modern formats (WebP/AVIF)
pub struct ImageFormatRule;

impl Rule for ImageFormatRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-IMG-004",
            name: "Modern Image Formats",
            description: "Consider using modern image formats like WebP or AVIF",
            category: RuleCategory::Media,
            severity: Severity::Low,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 2,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();
        let legacy_formats = [".jpg", ".jpeg", ".png", ".gif"];

        for img in &report.images {
            let src_lower = img.src.to_lowercase();
            if legacy_formats.iter().any(|ext| src_lower.ends_with(ext)) {
                issues.push(SeoIssue {
                    code: "SEO-IMG-004",
                    severity: Severity::Low,
                    message: format!("Consider modern format for: {}", img.src),
                    hint: Some("Use WebP or AVIF with <picture> element for better compression".into()),
                    selector: Some(format!("img[src=\"{}\"]", img.src)),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

// Technical Rules

/// Canonical URL Rule - checks for canonical URL
pub struct CanonicalUrlRule;

impl Rule for CanonicalUrlRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-TECH-001",
            name: "Canonical URL",
            description: "Pages should specify a canonical URL",
            category: RuleCategory::Technical,
            severity: Severity::High,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 10,
        };
        &META
    }

    fn check(&self, report: &SeoReport, context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if report.canonical.is_none() {
            issues.push(SeoIssue {
                code: "SEO-TECH-001",
                severity: Severity::High,
                message: "Missing canonical URL".into(),
                hint: Some(format!("Add <link rel=\"canonical\" href=\"{}\">", context.url)),
                selector: Some("head".into()),
                suggestion: None,
            });
        } else if let Some(canonical) = &report.canonical {
            if !canonical.starts_with("http") {
                issues.push(SeoIssue {
                    code: "SEO-TECH-001",
                    severity: Severity::Medium,
                    message: "Canonical URL should be absolute".into(),
                    hint: Some("Use a full URL including protocol and domain".into()),
                    selector: Some("link[rel=\"canonical\"]".into()),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

/// Favicon Rule - checks for favicon link tags
pub struct FaviconRule;

impl Rule for FaviconRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-TECH-002",
            name: "Favicon",
            description: "Pages should have a favicon",
            category: RuleCategory::Technical,
            severity: Severity::Low,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 2,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if !report.favicon {
            issues.push(SeoIssue {
                code: "SEO-TECH-002",
                severity: Severity::Low,
                message: "No favicon found".into(),
                hint: Some("Add a favicon link: <link rel=\"icon\" href=\"/favicon.ico\">".into()),
                selector: Some("head".into()),
                suggestion: None,
            });
        }

        issues
    }
}

// Social Rules

/// Open Graph Tags Rule - checks for OG meta tags
pub struct OpenGraphRule;

impl Rule for OpenGraphRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-SOCIAL-001",
            name: "Open Graph Tags",
            description: "Pages should have Open Graph meta tags for social sharing",
            category: RuleCategory::Social,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 5,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        let required_og_tags = ["og:title", "og:description", "og:image", "og:url", "og:type"];
        let mut missing_tags = Vec::new();

        for tag in &required_og_tags {
            if !report.og_tags.contains_key(*tag) {
                missing_tags.push(*tag);
            }
        }

        if !missing_tags.is_empty() {
            issues.push(SeoIssue {
                code: "SEO-SOCIAL-001",
                severity: Severity::Medium,
                message: format!("Missing Open Graph tags: {}", missing_tags.join(", ")),
                hint: Some("Add Open Graph meta tags for better social media sharing".into()),
                selector: Some("head".into()),
                suggestion: None,
            });
        }

        issues
    }
}

/// Twitter Card Rule - checks for Twitter Card meta tags
pub struct TwitterCardRule;

impl Rule for TwitterCardRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-SOCIAL-002",
            name: "Twitter Card",
            description: "Pages should have Twitter Card meta tags",
            category: RuleCategory::Social,
            severity: Severity::Low,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 3,
        };
        &META
    }

    fn check(&self, report: &SeoReport, _context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if report.twitter_card.is_none() {
            issues.push(SeoIssue {
                code: "SEO-SOCIAL-002",
                severity: Severity::Low,
                message: "No Twitter Card meta tags found".into(),
                hint: Some("Add Twitter Card meta tags for better Twitter sharing: <meta name=\"twitter:card\" content=\"summary\">".into()),
                selector: Some("head".into()),
                suggestion: None,
            });
        }

        issues
    }
}

// URL Rules

/// HTTPS Static Rule - checks for HTTPS usage (static URL check)
pub struct HttpsStaticRule;

impl Rule for HttpsStaticRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-URL-001",
            name: "HTTPS Usage",
            description: "Pages should use HTTPS",
            category: RuleCategory::Url,
            severity: Severity::Critical,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 20,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if context.url.starts_with("http://") {
            issues.push(SeoIssue {
                code: "SEO-URL-001",
                severity: Severity::Critical,
                message: "Page uses HTTP instead of HTTPS".into(),
                hint: Some("Use HTTPS for better security and SEO".into()),
                selector: None,
                suggestion: None,
            });
        }

        issues
    }
}

/// Trailing Slash Rule - checks trailing slash consistency
pub struct TrailingSlashRule;

impl Rule for TrailingSlashRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-URL-002",
            name: "URL Trailing Slash",
            description: "Check trailing slash consistency",
            category: RuleCategory::Url,
            severity: Severity::Low,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 2,
        };
        &META
    }

    fn check(&self, report: &SeoReport, context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        // Check if canonical URL has different trailing slash
        if let Some(canonical) = &report.canonical {
            let url_has_slash = context.url.ends_with('/');
            let canonical_has_slash = canonical.ends_with('/');

            if url_has_slash != canonical_has_slash {
                issues.push(SeoIssue {
                    code: "SEO-URL-002",
                    severity: Severity::Medium,
                    message: "URL and canonical have different trailing slashes".into(),
                    hint: Some("Ensure consistent trailing slash between URL and canonical to avoid duplicate content".into()),
                    selector: Some("link[rel=\"canonical\"]".into()),
                    suggestion: None,
                });
            }
        }

        issues
    }
}

/// URL Length Rule - checks URL path length
pub struct UrlLengthRule;

impl Rule for UrlLengthRule {
    fn meta(&self) -> &RuleMeta {
        static META: RuleMeta = RuleMeta {
            code: "SEO-URL-003",
            name: "URL Length",
            description: "Check if URL length is optimal for SEO",
            category: RuleCategory::Url,
            severity: Severity::Medium,
            capabilities: RuleCapabilities::STREAMING_HTML,
            weight: 3,
        };
        &META
    }

    fn check(&self, _report: &SeoReport, context: &RuleContext) -> Vec<SeoIssue> {
        let mut issues = Vec::new();

        if let Ok(url_obj) = url::Url::parse(&context.url) {
            let path_with_query = format!("{}{}", url_obj.path(), url_obj.query().unwrap_or(""));
            
            if path_with_query.len() > 100 {
                issues.push(SeoIssue {
                    code: "SEO-URL-003",
                    severity: Severity::Medium,
                    message: format!("URL path is very long ({} characters)", path_with_query.len()),
                    hint: Some("URLs over 100 characters are harder to share and may be truncated. Aim for under 75 characters.".into()),
                    selector: None,
                    suggestion: None,
                });
            } else if path_with_query.len() > 75 {
                issues.push(SeoIssue {
                    code: "SEO-URL-003",
                    severity: Severity::Low,
                    message: format!("URL path is somewhat long ({} characters)", path_with_query.len()),
                    hint: Some("Consider shortening URLs for better user experience and shareability".into()),
                    selector: None,
                    suggestion: None,
                });
            }

            // Check for very deep nesting
            let path_segments = url_obj.path_segments()
                .map(|segments| segments.filter(|s| !s.is_empty()).count())
                .unwrap_or(0);
            
            if path_segments > 4 {
                issues.push(SeoIssue {
                    code: "SEO-URL-003",
                    severity: Severity::Low,
                    message: format!("Deep URL nesting ({} levels)", path_segments),
                    hint: Some("Flatten URL structure where possible for better crawlability".into()),
                    selector: None,
                    suggestion: None,
                });
            }
        }

        issues
    }
}

/// Get all Tier A rules
pub fn get_tier_a_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(MetaTitleRule),
        Box::new(MetaDescriptionRule),
        Box::new(MetaCharsetRule),
        Box::new(MetaViewportRule),
        Box::new(LangAttributeRule),
        Box::new(RobotsMetaRule),
        Box::new(SingleH1Rule),
        Box::new(HeadingHierarchyRule),
        Box::new(EmptyHeadingRule),
        Box::new(ImageAltRule),
        Box::new(ImageLazyLoadRule),
        Box::new(ImageDimensionsRule),
        Box::new(ImageFormatRule),
        Box::new(CanonicalUrlRule),
        Box::new(FaviconRule),
        Box::new(OpenGraphRule),
        Box::new(TwitterCardRule),
        Box::new(HttpsStaticRule),
        Box::new(TrailingSlashRule),
        Box::new(UrlLengthRule),
    ]
}
