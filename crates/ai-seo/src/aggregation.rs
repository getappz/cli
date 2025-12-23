use std::collections::HashMap;
use crate::models::*;
use crate::registry::lookup;
use bitvec::prelude::*;

/// Calculate page weight based on URL depth
/// Homepage gets 2.0, shallow pages (1-2 slashes) get 1.5, deep pages get 1.0
pub fn page_weight(url: &str) -> f32 {
    // Extract path from URL (handle file://, http://, etc.)
    let path = if let Some(path_start) = url.find('/') {
        if url.starts_with("file://") {
            url.strip_prefix("file://").unwrap_or(url)
        } else if url.contains("://") {
            url.splitn(4, '/').nth(3).unwrap_or("/")
        } else {
            &url[path_start..]
        }
    } else {
        url
    };

    match path {
        "/" | "" => 2.0,
        _ => {
            let slash_count = path.matches('/').count();
            if slash_count <= 1 {
                1.5
            } else {
                1.0
            }
        }
    }
}

/// Internal accumulator for score aggregation
struct ScoreAccumulator {
    total_score: u64,
    weighted_score: f32,
    total_weight: f32,
    min_score: u8,
    max_score: u8,
}

impl ScoreAccumulator {
    fn new() -> Self {
        Self {
            total_score: 0,
            weighted_score: 0.0,
            total_weight: 0.0,
            min_score: 100,
            max_score: 0,
        }
    }

    fn add(&mut self, score: u8, weight: f32) {
        self.total_score += score as u64;
        self.weighted_score += score as f32 * weight;
        self.total_weight += weight;
        self.min_score = self.min_score.min(score);
        self.max_score = self.max_score.max(score);
    }

    fn finalize(&self, page_count: usize) -> SiteScoreSummary {
        let average = if page_count > 0 {
            (self.total_score / page_count as u64) as u8
        } else {
            0
        };
        let weighted = if self.total_weight > 0.0 {
            (self.weighted_score / self.total_weight) as u8
        } else {
            0
        };
        SiteScoreSummary {
            average,
            weighted,
            min: self.min_score,
            max: self.max_score,
        }
    }
}

/// Internal accumulator for coverage metrics
struct CoverageAccumulator {
    pages_with_title: usize,
    pages_with_meta_desc: usize,
    pages_with_h1: usize,
    images_with_alt: usize,
    total_images: usize,
}

impl CoverageAccumulator {
    fn new() -> Self {
        Self {
            pages_with_title: 0,
            pages_with_meta_desc: 0,
            pages_with_h1: 0,
            images_with_alt: 0,
            total_images: 0,
        }
    }

    fn add(&mut self, report: &SeoReport) {
        if report.title.is_some() {
            self.pages_with_title += 1;
        }
        if report.meta_description.is_some() {
            self.pages_with_meta_desc += 1;
        }
        if report.headings.iter().any(|h| h.level == 1) {
            self.pages_with_h1 += 1;
        }
        for img in &report.images {
            self.total_images += 1;
            if img.alt.is_some() {
                self.images_with_alt += 1;
            }
        }
    }

    fn finalize(&self, page_count: usize) -> CoverageMetrics {
        let title_coverage = if page_count > 0 {
            self.pages_with_title as f32 / page_count as f32
        } else {
            0.0
        };
        let meta_description_coverage = if page_count > 0 {
            self.pages_with_meta_desc as f32 / page_count as f32
        } else {
            0.0
        };
        let h1_coverage = if page_count > 0 {
            self.pages_with_h1 as f32 / page_count as f32
        } else {
            0.0
        };
        let image_alt_coverage = if self.total_images > 0 {
            self.images_with_alt as f32 / self.total_images as f32
        } else {
            0.0
        };
        CoverageMetrics {
            title_coverage,
            meta_description_coverage,
            h1_coverage,
            image_alt_coverage,
        }
    }
}

/// Streaming aggregator for site-level SEO metrics
/// Processes page reports incrementally without keeping all pages in memory
pub struct SiteAggregator {
    page_count: usize,
    score_acc: ScoreAccumulator,
    issue_counts: HashMap<&'static str, usize>,
    /// Track affected pages using BitVec (page index -> bool)
    /// More memory-efficient than HashSet<String>
    affected_pages: HashMap<&'static str, BitVec>,
    /// Map page index to URL for final output
    page_urls: Vec<String>,
    coverage: CoverageAccumulator,
    severity_counts: SeverityCounts,
    category_counts: CategoryCounts,
}

impl SiteAggregator {
    pub fn new() -> Self {
        Self {
            page_count: 0,
            score_acc: ScoreAccumulator::new(),
            issue_counts: HashMap::new(),
            affected_pages: HashMap::new(),
            page_urls: Vec::new(),
            coverage: CoverageAccumulator::new(),
            severity_counts: SeverityCounts {
                critical: 0,
                high: 0,
                medium: 0,
                low: 0,
            },
            category_counts: CategoryCounts {
                meta: 0,
                structure: 0,
                content: 0,
                media: 0,
                links: 0,
            },
        }
    }

    /// Ingest a single page report into the aggregation
    pub fn ingest(&mut self, report: &SeoReport) {
        let page_index = self.page_count;
        self.page_count += 1;

        // Store URL for this page index
        self.page_urls.push(report.url.clone());

        // Add score with weight
        let weight = page_weight(&report.url);
        self.score_acc.add(report.score.total, weight);

        // Track coverage
        self.coverage.add(report);

        // Aggregate issues
        for issue in &report.issues {
            // Count by code
            *self.issue_counts.entry(issue.code).or_insert(0) += 1;
            
            // Track affected pages using BitVec (more efficient than HashSet<String>)
            // Ensure all bitvecs are large enough for current page count
            let bitvec = self.affected_pages
                .entry(issue.code)
                .or_insert_with(|| bitvec![0; self.page_count]);
            
            // Resize if needed (shouldn't happen often, but handle growth)
            if bitvec.len() < self.page_count {
                bitvec.resize(self.page_count, false);
            }
            bitvec.set(page_index, true);

            // Count by severity
            match issue.severity {
                Severity::Critical => self.severity_counts.critical += 1,
                Severity::High => self.severity_counts.high += 1,
                Severity::Medium => self.severity_counts.medium += 1,
                Severity::Low => self.severity_counts.low += 1,
            }

            // Count by category
            if let Some(def) = lookup(issue.code) {
                match def.category {
                    "meta" => self.category_counts.meta += 1,
                    "structure" => self.category_counts.structure += 1,
                    "content" => self.category_counts.content += 1,
                    "media" => self.category_counts.media += 1,
                    "links" => self.category_counts.links += 1,
                    _ => {}
                }
            }
        }
    }

    /// Finalize aggregation and produce site summary
    pub fn finalize(self) -> SiteSeoSummary {
        let score = self.score_acc.finalize(self.page_count);
        let coverage = self.coverage.finalize(self.page_count);

        // Build issue counts by code
        // Convert BitVec indices back to URLs
        let mut issue_counts: Vec<IssueCount> = self
            .issue_counts
            .into_iter()
            .map(|(code, count)| {
                let urls: Vec<String> = self
                    .affected_pages
                    .get(code)
                    .map(|bitvec| {
                        bitvec.iter_ones()
                            .filter_map(|idx| self.page_urls.get(idx).cloned())
                            .collect()
                    })
                    .unwrap_or_default();
                let affected_pages = urls.len();
                IssueCount {
                    code,
                    count,
                    affected_pages,
                    urls,
                }
            })
            .collect();

        // Sort by count descending
        issue_counts.sort_by(|a, b| b.count.cmp(&a.count));

        // Identify hotspots (issues affecting >30% of pages)
        let hotspot_threshold = (self.page_count as f32 * 0.3).ceil() as usize;
        let top_issue_codes: Vec<String> = issue_counts
            .iter()
            .filter(|ic| ic.affected_pages >= hotspot_threshold)
            .map(|ic| ic.code.to_string())
            .collect();

        let templates_with_issues = top_issue_codes.len();

        SiteSeoSummary {
            page_count: self.page_count,
            score,
            issues: IssueAggregation {
                by_code: issue_counts,
                by_severity: self.severity_counts,
                by_category: self.category_counts,
            },
            coverage,
            hotspots: HotspotMetrics {
                templates_with_issues,
                top_issue_codes,
            },
        }
    }
}

impl Default for SiteAggregator {
    fn default() -> Self {
        Self::new()
    }
}

