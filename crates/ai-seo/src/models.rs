
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ImageInfo {
    pub src: String,
    pub alt: Option<String>,
    pub loading: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct LinkInfo {
    pub href: String,
    pub text: String,
}

#[derive(Serialize, Clone)]
pub struct SeoIssue {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub hint: Option<String>,
    /// CSS-like selector for the element (replaces line numbers which are unreliable with streaming parsers)
    pub selector: Option<String>,
    /// AI-generated suggestion (advisory only, never affects rule execution)
    pub suggestion: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Serialize)]
pub struct SeoScore {
    pub meta: u8,
    pub structure: u8,
    pub content: u8,
    pub media: u8,
    pub links: u8,
    pub total: u8,
}

impl Default for SeoScore {
    fn default() -> Self {
        SeoScore {
            meta: 100,
            structure: 100,
            content: 100,
            media: 100,
            links: 100,
            total: 100,
        }
    }
}

#[derive(Serialize)]
pub struct SeoReport {
    pub url: String,
    pub title: Option<String>,
    pub meta_description: Option<String>,
    pub canonical: Option<String>,
    pub word_count: usize,
    pub headings: Vec<Heading>,
    pub images: Vec<ImageInfo>,
    pub links: Vec<LinkInfo>,
    pub issues: Vec<SeoIssue>,
    pub score: SeoScore,
    // Additional metadata for enhanced rules
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub robots_meta: Option<String>,
    pub favicon: bool,
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub og_tags: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub twitter_card: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub json_ld_scripts: Vec<String>,
}

// Site-level aggregation models

#[derive(Serialize)]
pub struct SiteSeoSummary {
    pub page_count: usize,
    pub score: SiteScoreSummary,
    pub issues: IssueAggregation,
    pub coverage: CoverageMetrics,
    pub hotspots: HotspotMetrics,
}

#[derive(Serialize)]
pub struct SiteScoreSummary {
    pub average: u8,
    pub weighted: u8,
    pub min: u8,
    pub max: u8,
}

#[derive(Serialize)]
pub struct IssueAggregation {
    pub by_code: Vec<IssueCount>,
    pub by_severity: SeverityCounts,
    pub by_category: CategoryCounts,
}

#[derive(Serialize, Clone)]
pub struct IssueCount {
    pub code: &'static str,
    pub count: usize,
    pub affected_pages: usize,
    /// URLs of affected pages (for routing decisions)
    pub urls: Vec<String>,
}

#[derive(Serialize)]
pub struct SeverityCounts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

#[derive(Serialize)]
pub struct CategoryCounts {
    pub meta: usize,
    pub structure: usize,
    pub content: usize,
    pub media: usize,
    pub links: usize,
}

#[derive(Serialize)]
pub struct CoverageMetrics {
    pub title_coverage: f32,
    pub meta_description_coverage: f32,
    pub h1_coverage: f32,
    pub image_alt_coverage: f32,
}

#[derive(Serialize)]
pub struct HotspotMetrics {
    pub templates_with_issues: usize,
    pub top_issue_codes: Vec<String>,
}
