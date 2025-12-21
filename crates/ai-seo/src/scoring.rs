
use crate::models::{SeoIssue, SeoScore};
use crate::registry::lookup;

pub fn compute_score(issues: &[SeoIssue]) -> SeoScore {
    let mut meta: u8 = 100;
    let mut structure: u8 = 100;
    let mut content: u8 = 100;
    let mut media: u8 = 100;
    let mut links: u8 = 100;

    for issue in issues {
        if let Some(def) = lookup(issue.code) {
            let bucket = match def.category {
                "meta" => &mut meta,
                "structure" => &mut structure,
                "content" => &mut content,
                "media" => &mut media,
                "links" => &mut links,
                _ => continue,
            };
            *bucket = (*bucket).saturating_sub(def.weight);
        }
    }

    let total = ((meta as u16 + structure as u16 + content as u16 + media as u16 + links as u16) / 5) as u8;

    SeoScore {
        meta,
        structure,
        content,
        media,
        links,
        total,
    }
}
