
use crate::models::{SeoIssue, SeoScore};
use crate::registry::lookup;

/// Simplified grading profile - just weight sets per category
pub struct GradingProfile {
    pub weights: std::collections::HashMap<&'static str, u32>,
    pub max_score: u32,
}

impl Default for GradingProfile {
    fn default() -> Self {
        let mut weights = std::collections::HashMap::new();
        weights.insert("meta", 1);
        weights.insert("structure", 1);
        weights.insert("content", 1);
        weights.insert("media", 1);
        weights.insert("links", 1);
        weights.insert("technical", 1);
        weights.insert("social", 1);
        weights.insert("structured_data", 1);
        weights.insert("url", 1);
        weights.insert("security", 1);
        
        Self {
            weights,
            max_score: 100,
        }
    }
}

pub fn compute_score(issues: &[SeoIssue]) -> SeoScore {
    compute_score_with_profile(issues, &GradingProfile::default())
}

pub fn compute_score_with_profile(issues: &[SeoIssue], profile: &GradingProfile) -> SeoScore {
    let mut meta: u8 = 100;
    let mut structure: u8 = 100;
    let mut content: u8 = 100;
    let mut media: u8 = 100;
    let mut links: u8 = 100;
    let mut technical: u8 = 100;
    let mut social: u8 = 100;
    let mut structured_data: u8 = 100;
    let mut url: u8 = 100;
    let mut security: u8 = 100;

    for issue in issues {
        if let Some(def) = lookup(issue.code) {
            let weight_multiplier = profile.weights.get(def.category).copied().unwrap_or(1) as u8;
            let adjusted_weight = def.weight * weight_multiplier;
            
            let bucket = match def.category {
                "meta" => &mut meta,
                "structure" => &mut structure,
                "content" => &mut content,
                "media" => &mut media,
                "links" => &mut links,
                "technical" => &mut technical,
                "social" => &mut social,
                "structured_data" => &mut structured_data,
                "url" => &mut url,
                "security" => &mut security,
                _ => continue,
            };
            *bucket = (*bucket).saturating_sub(adjusted_weight);
        }
    }

    // Calculate total as average of all categories
    let total = ((meta as u16 + structure as u16 + content as u16 + media as u16 + links as u16 +
                 technical as u16 + social as u16 + structured_data as u16 + url as u16 + security as u16) / 10) as u8;

    SeoScore {
        meta,
        structure,
        content,
        media,
        links,
        total,
    }
}
