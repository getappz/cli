//! Issue Registry
//!
//! Central registry of all SEO issue definitions with metadata.
//! This is used for scoring, aggregation, and routing decisions.

use crate::models::Severity;
use crate::fix_plan::FixScope;

pub struct IssueDef {
    pub code: &'static str,
    pub severity: Severity,
    pub category: &'static str,
    pub weight: u8,
    /// Preferred scope hint for routing decisions
    /// This is a hint that can be overridden by data-driven routing
    pub preferred_scope: Option<FixScope>,
}

pub const ISSUE_REGISTRY: &[IssueDef] = &[
    // Meta Rules (Tier A)
    IssueDef { code: "SEO-META-001", severity: Severity::Critical, category: "meta", weight: 20, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-META-002", severity: Severity::High, category: "meta", weight: 10, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-META-003", severity: Severity::Medium, category: "meta", weight: 5, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-META-004", severity: Severity::Critical, category: "meta", weight: 15, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-META-005", severity: Severity::Medium, category: "meta", weight: 5, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-META-006", severity: Severity::Low, category: "meta", weight: 3, preferred_scope: Some(FixScope::Template) },
    
    // Structure/Heading Rules (Tier A)
    IssueDef { code: "SEO-H1-001", severity: Severity::Critical, category: "structure", weight: 20, preferred_scope: None },
    IssueDef { code: "SEO-H1-002", severity: Severity::Critical, category: "structure", weight: 20, preferred_scope: None },
    IssueDef { code: "SEO-H1-003", severity: Severity::High, category: "structure", weight: 10, preferred_scope: None },
    IssueDef { code: "SEO-H1-004", severity: Severity::Critical, category: "structure", weight: 15, preferred_scope: None },
    
    // Content Rules
    IssueDef { code: "SEO-CONTENT-001", severity: Severity::Medium, category: "content", weight: 10, preferred_scope: None },
    IssueDef { code: "SEO-CONTENT-002", severity: Severity::Medium, category: "content", weight: 5, preferred_scope: None },
    IssueDef { code: "SEO-CONTENT-003", severity: Severity::Medium, category: "content", weight: 5, preferred_scope: None },
    IssueDef { code: "SEO-CONTENT-004", severity: Severity::Medium, category: "content", weight: 5, preferred_scope: None },
    
    // Media/Image Rules (Tier A)
    IssueDef { code: "SEO-IMG-001", severity: Severity::Medium, category: "media", weight: 5, preferred_scope: None },
    IssueDef { code: "SEO-IMG-002", severity: Severity::Low, category: "media", weight: 3, preferred_scope: None },
    IssueDef { code: "SEO-IMG-003", severity: Severity::Medium, category: "media", weight: 5, preferred_scope: None },
    IssueDef { code: "SEO-IMG-004", severity: Severity::Low, category: "media", weight: 2, preferred_scope: None },
    
    // Link Rules
    IssueDef { code: "SEO-LINK-001", severity: Severity::Low, category: "links", weight: 3, preferred_scope: None },
    IssueDef { code: "SEO-LINK-002", severity: Severity::Low, category: "links", weight: 2, preferred_scope: None },
    IssueDef { code: "SEO-LINK-003", severity: Severity::High, category: "links", weight: 10, preferred_scope: None },
    IssueDef { code: "SEO-LINK-004", severity: Severity::Medium, category: "links", weight: 5, preferred_scope: None },
    
    // Technical Rules (Tier A)
    IssueDef { code: "SEO-TECH-001", severity: Severity::High, category: "technical", weight: 10, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-TECH-002", severity: Severity::Low, category: "technical", weight: 2, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-TECH-003", severity: Severity::Low, category: "technical", weight: 3, preferred_scope: Some(FixScope::Template) },
    
    // Social Rules (Tier A)
    IssueDef { code: "SEO-SOCIAL-001", severity: Severity::Medium, category: "social", weight: 5, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-SOCIAL-002", severity: Severity::Low, category: "social", weight: 3, preferred_scope: Some(FixScope::Template) },
    
    // Structured Data Rules (Tier B)
    IssueDef { code: "SEO-STRUCT-001", severity: Severity::Low, category: "structured_data", weight: 3, preferred_scope: None },
    IssueDef { code: "SEO-STRUCT-002", severity: Severity::Low, category: "structured_data", weight: 2, preferred_scope: None },
    
    // URL Rules (Tier A)
    IssueDef { code: "SEO-URL-001", severity: Severity::Critical, category: "url", weight: 20, preferred_scope: None },
    IssueDef { code: "SEO-URL-002", severity: Severity::Medium, category: "url", weight: 3, preferred_scope: None },
    IssueDef { code: "SEO-URL-003", severity: Severity::Medium, category: "url", weight: 3, preferred_scope: None },
    IssueDef { code: "SEO-URL-004", severity: Severity::Medium, category: "url", weight: 3, preferred_scope: None },
    
    // Security Rules (Tier C)
    IssueDef { code: "SEO-SEC-001", severity: Severity::High, category: "security", weight: 10, preferred_scope: None },
    IssueDef { code: "SEO-SEC-002", severity: Severity::High, category: "security", weight: 10, preferred_scope: None },
    IssueDef { code: "SEO-SEC-003", severity: Severity::Medium, category: "security", weight: 5, preferred_scope: None },
    IssueDef { code: "SEO-SEC-004", severity: Severity::High, category: "security", weight: 10, preferred_scope: None },
];

pub fn lookup(code: &str) -> Option<&'static IssueDef> {
    ISSUE_REGISTRY.iter().find(|d| d.code == code)
}
