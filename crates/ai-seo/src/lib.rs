
pub mod analyzer;
pub mod models;
pub mod rules;
pub mod registry;
pub mod scoring;
pub mod aggregation;
pub mod fix_plan;
pub mod routing;
pub mod mutation;
pub mod diff;
pub mod preview;
pub mod db;
pub mod utils;
pub mod format;

// Re-export commonly used types
pub use routing::FixRouting;
pub use fix_plan::{FixPlan, FixScope, FixRisk};

use analyzer::analyze_html;
use models::SeoReport;

pub fn analyze(html: &str, url: &str) -> SeoReport {
    analyze_html(html, url)
}
