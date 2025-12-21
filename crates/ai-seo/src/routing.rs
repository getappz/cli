//! Template vs Page Fix Routing
//!
//! This module provides data-driven routing decisions to determine whether
//! an issue should be fixed once (template/layout) or many times (pages).
//!
//! The routing algorithm prevents fixing the same problem 300 times by
//! analyzing site-level evidence:
//! - Issue coverage ratio (affected_pages / total_pages)
//! - URL prefix clustering (section detection)
//! - Issue category and preferred scope hints
//!
//! This is the decision layer that prevents wasted AI, brittle diffs, and slow CI.

use crate::models::SiteSeoSummary;
use crate::fix_plan::FixScope;
use crate::registry::lookup;
use serde::Serialize;


/// Routing decision for where a fix should be applied
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FixRouting {
    /// Fix in template/layout (affects all pages)
    Template,
    /// Fix in section template (affects pages with matching URL prefix)
    Section { prefix: String },
    /// Fix on individual pages
    Page,
}

/// Route a fix based on site-level evidence
///
/// This implements the deterministic routing algorithm that uses:
/// 1. Registry scope hint (hard override)
/// 2. Coverage threshold (≥30% = Template, 10-29% = Section, <10% = Page)
/// 3. URL prefix clustering (section detection)
/// 4. Issue category fallback
///
/// # Arguments
/// * `issue_code` - The issue code to route
/// * `site` - Site-level aggregation with all evidence
///
/// # Returns
/// Routing decision (Template, Section, or Page)
pub fn route_fix(issue_code: &str, site: &SiteSeoSummary) -> FixRouting {
    // Find the issue in aggregation
    let issue_count = site
        .issues
        .by_code
        .iter()
        .find(|ic| ic.code == issue_code);

    let Some(issue_count) = issue_count else {
        // Issue not found in aggregation, default to Page
        return FixRouting::Page;
    };

    // Rule 1: Registry scope hint (hard override)
    if let Some(def) = lookup(issue_code) {
        if let Some(preferred_scope) = &def.preferred_scope {
            match preferred_scope {
                FixScope::Template | FixScope::Site => {
                    return FixRouting::Template;
                }
                FixScope::Section => {
                    // Try to detect section prefix
                    if let Some(prefix) = detect_section(&issue_count.urls) {
                        return FixRouting::Section { prefix };
                    }
                    // Fall through to other rules
                }
                FixScope::Page => {
                    // Preferred scope is Page, but we can still check if coverage
                    // suggests template fix would be better
                    // For now, respect the hint
                    return FixRouting::Page;
                }
            }
        }
    }

    // Rule 2: Coverage threshold (most important)
    let ratio = if site.page_count > 0 {
        issue_count.affected_pages as f32 / site.page_count as f32
    } else {
        0.0
    };

    if ratio >= 0.30 {
        return FixRouting::Template;
    }

    // Rule 3: Section detection (URL prefix clustering)
    if let Some(prefix) = detect_section(&issue_count.urls) {
        return FixRouting::Section { prefix };
    }

    // Rule 4: Category-based fallback (only when ratios are ambiguous)
    if let Some(def) = lookup(issue_code) {
        match def.category {
            "meta" => {
                // Meta issues default to Template if coverage is reasonable
                if ratio >= 0.10 {
                    return FixRouting::Template;
                }
            }
            "structure" => {
                // Structure issues can be Template or Page depending on coverage
                if ratio >= 0.20 {
                    return FixRouting::Template;
                }
            }
            "media" | "content" => {
                // Media and content issues default to Page
                // (prevents template-level content hallucination)
                return FixRouting::Page;
            }
            _ => {}
        }
    }

    // Default fallback
    FixRouting::Page
}

/// Detect if affected pages share a common URL prefix (section detection)
///
/// Returns the longest common path prefix if at least 2 pages share it
/// and it represents a meaningful section (e.g., /blog, /docs, /pricing).
fn detect_section(urls: &[String]) -> Option<String> {
    if urls.len() < 2 {
        return None;
    }

    // Extract paths from URLs
    let paths: Vec<&str> = urls
        .iter()
        .map(|url| extract_path(url))
        .collect();

    // Find longest common prefix
    if let Some(prefix) = common_prefix(&paths) {
        // Only return if it's a meaningful section (at least 2 path segments)
        // and affects a significant portion of URLs
        let matching_count = paths.iter().filter(|p| p.starts_with(&prefix)).count();
        let match_ratio = matching_count as f32 / paths.len() as f32;

        // Require at least 60% of URLs to share the prefix
        if match_ratio >= 0.60 && prefix.len() > 1 && prefix != "/" {
            // Ensure it ends with a slash (section indicator)
            if prefix.ends_with('/') {
                Some(prefix.to_string())
            } else {
                // Find the last slash to make it a section prefix
                if let Some(last_slash) = prefix.rfind('/') {
                    Some(prefix[..=last_slash].to_string())
                } else {
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Extract path from URL (handles file://, http://, etc.)
fn extract_path(url: &str) -> &str {
    if let Some(path_start) = url.find('/') {
        if url.starts_with("file://") {
            url.strip_prefix("file://").unwrap_or(url)
        } else if url.contains("://") {
            url.splitn(4, '/').nth(3).unwrap_or("/")
        } else {
            &url[path_start..]
        }
    } else {
        url
    }
}

/// Find longest common path prefix among URLs
fn common_prefix(paths: &[&str]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }

    if paths.len() == 1 {
        return Some(paths[0].to_string());
    }

    // Start with first path
    let mut prefix = paths[0].to_string();

    // Compare with each subsequent path
    for path in paths.iter().skip(1) {
        prefix = common_prefix_two(&prefix, path);
        if prefix.is_empty() {
            return None;
        }
    }

    Some(prefix)
}

/// Find longest common prefix between two paths
fn common_prefix_two(a: &str, b: &str) -> String {
    let mut common = String::new();
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let min_len = a_chars.len().min(b_chars.len());

    for i in 0..min_len {
        if a_chars[i] == b_chars[i] {
            common.push(a_chars[i]);
        } else {
            break;
        }
    }

    // If we stopped mid-path-segment, truncate to last complete segment
    if !common.is_empty() && !common.ends_with('/') {
        if let Some(last_slash) = common.rfind('/') {
            common.truncate(last_slash + 1);
        }
    }

    common
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{IssueCount, *};

    fn create_test_site(page_count: usize, issue_count: IssueCount) -> SiteSeoSummary {
        SiteSeoSummary {
            page_count,
            score: SiteScoreSummary {
                average: 80,
                weighted: 80,
                min: 60,
                max: 100,
            },
            issues: IssueAggregation {
                by_code: vec![issue_count],
                by_severity: SeverityCounts {
                    critical: 0,
                    high: 0,
                    medium: 0,
                    low: 0,
                },
                by_category: CategoryCounts {
                    meta: 0,
                    structure: 0,
                    content: 0,
                    media: 0,
                    links: 0,
                },
            },
            coverage: CoverageMetrics {
                title_coverage: 1.0,
                meta_description_coverage: 1.0,
                h1_coverage: 1.0,
                image_alt_coverage: 1.0,
            },
            hotspots: HotspotMetrics {
                templates_with_issues: 0,
                top_issue_codes: vec![],
            },
        }
    }

    #[test]
    fn test_route_template_by_coverage() {
        // 124 pages affected out of 312 = 39.7% → Template
        let issue = IssueCount {
            code: "SEO-META-002",
            count: 124,
            affected_pages: 124,
            urls: (0..124).map(|i| format!("/page-{}", i)).collect(),
        };
        let site = create_test_site(312, issue);
        let routing = route_fix("SEO-META-002", &site);
        assert!(matches!(routing, FixRouting::Template));
    }

    #[test]
    fn test_route_section_by_prefix() {
        // 48 pages in /blog/* → Section
        let issue = IssueCount {
            code: "SEO-H1-001",
            count: 48,
            affected_pages: 48,
            urls: (0..48)
                .map(|i| format!("/blog/post-{}", i))
                .collect(),
        };
        let site = create_test_site(200, issue);
        let routing = route_fix("SEO-H1-001", &site);
        match routing {
            FixRouting::Section { prefix } => {
                assert_eq!(prefix, "/blog/");
            }
            _ => panic!("Expected Section routing"),
        }
    }

    #[test]
    fn test_route_page_by_low_coverage() {
        // 7 pages affected out of 200 = 3.5% → Page
        let issue = IssueCount {
            code: "SEO-IMG-001",
            count: 7,
            affected_pages: 7,
            urls: (0..7).map(|i| format!("/random-{}", i)).collect(),
        };
        let site = create_test_site(200, issue);
        let routing = route_fix("SEO-IMG-001", &site);
        assert!(matches!(routing, FixRouting::Page));
    }

    #[test]
    fn test_route_template_by_registry_hint() {
        // Even with low coverage, registry hint overrides
        let issue = IssueCount {
            code: "SEO-META-001",
            count: 5,
            affected_pages: 5,
            urls: (0..5).map(|i| format!("/page-{}", i)).collect(),
        };
        let site = create_test_site(200, issue);
        let routing = route_fix("SEO-META-001", &site);
        assert!(matches!(routing, FixRouting::Template));
    }

    #[test]
    fn test_detect_section() {
        let urls = vec![
            "/blog/post-1".to_string(),
            "/blog/post-2".to_string(),
            "/blog/post-3".to_string(),
        ];
        assert_eq!(detect_section(&urls), Some("/blog/".to_string()));

        let urls = vec![
            "/docs/getting-started".to_string(),
            "/docs/advanced".to_string(),
            "/docs/api".to_string(),
        ];
        assert_eq!(detect_section(&urls), Some("/docs/".to_string()));

        let urls = vec![
            "/page-1".to_string(),
            "/page-2".to_string(),
            "/random".to_string(),
        ];
        assert_eq!(detect_section(&urls), None);
    }

    #[test]
    fn test_common_prefix() {
        let paths = vec!["/blog/post-1", "/blog/post-2", "/blog/post-3"];
        assert_eq!(
            common_prefix(&paths.iter().map(|s| *s).collect::<Vec<_>>()),
            Some("/blog/".to_string())
        );

        let paths = vec!["/docs/a", "/docs/b", "/other"];
        assert_eq!(
            common_prefix(&paths.iter().map(|s| *s).collect::<Vec<_>>()),
            None
        );
    }
}

