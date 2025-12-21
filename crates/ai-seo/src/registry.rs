
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
    IssueDef { code: "SEO-META-001", severity: Severity::Critical, category: "meta", weight: 20, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-META-002", severity: Severity::High, category: "meta", weight: 10, preferred_scope: Some(FixScope::Template) },
    IssueDef { code: "SEO-H1-001", severity: Severity::Critical, category: "structure", weight: 20, preferred_scope: None },
    IssueDef { code: "SEO-H1-002", severity: Severity::Critical, category: "structure", weight: 20, preferred_scope: None },
    IssueDef { code: "SEO-CONTENT-001", severity: Severity::Medium, category: "content", weight: 10, preferred_scope: None },
    IssueDef { code: "SEO-IMG-001", severity: Severity::Medium, category: "media", weight: 5, preferred_scope: None },
    IssueDef { code: "SEO-IMG-002", severity: Severity::Low, category: "media", weight: 3, preferred_scope: None },
    IssueDef { code: "SEO-LINK-001", severity: Severity::Low, category: "links", weight: 3, preferred_scope: None },
];

pub fn lookup(code: &str) -> Option<&'static IssueDef> {
    ISSUE_REGISTRY.iter().find(|d| d.code == code)
}
