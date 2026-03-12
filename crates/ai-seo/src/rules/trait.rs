//! Rule Trait and Metadata
//!
//! Defines the Rule trait that all SEO rules must implement, along with
//! RuleMeta that includes capability declarations.

use crate::models::{SeoIssue, SeoReport};
use crate::rules::capabilities::RuleCapabilities;

/// Category for organizing rules
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleCategory {
    Meta,
    Structure,
    Content,
    Media,
    Links,
    Technical,
    Social,
    StructuredData,
    Url,
    Performance,
    Security,
}

impl RuleCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleCategory::Meta => "meta",
            RuleCategory::Structure => "structure",
            RuleCategory::Content => "content",
            RuleCategory::Media => "media",
            RuleCategory::Links => "links",
            RuleCategory::Technical => "technical",
            RuleCategory::Social => "social",
            RuleCategory::StructuredData => "structured_data",
            RuleCategory::Url => "url",
            RuleCategory::Performance => "performance",
            RuleCategory::Security => "security",
        }
    }
}

/// Metadata for a rule, including its capabilities
#[derive(Debug, Clone)]
pub struct RuleMeta {
    pub code: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: RuleCategory,
    pub severity: crate::models::Severity,
    pub capabilities: RuleCapabilities,
    pub weight: u8,
}

/// Context passed to rules during execution
pub struct RuleContext {
    /// The URL being analyzed
    pub url: String,
    /// Base URL for resolving relative URLs
    pub base_url: Option<String>,
    /// Whether HTTP checks are enabled
    pub http_enabled: bool,
    /// Whether AI is available (advisory only)
    pub ai_enabled: bool,
}

impl RuleContext {
    pub fn new(url: String) -> Self {
        Self {
            url,
            base_url: None,
            http_enabled: false,
            ai_enabled: false,
        }
    }
    
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = Some(base_url);
        self
    }
    
    pub fn with_http(mut self, enabled: bool) -> Self {
        self.http_enabled = enabled;
        self
    }
    
    pub fn with_ai(mut self, enabled: bool) -> Self {
        self.ai_enabled = enabled;
        self
    }
}

/// Trait that all SEO rules must implement
pub trait Rule: Send + Sync {
    /// Get rule metadata including capabilities
    fn meta(&self) -> &RuleMeta;
    
    /// Check if this rule can run in the given context
    fn can_run(&self, context: &RuleContext) -> bool {
        let meta = self.meta();
        
        // Check if rule requires HTTP but HTTP is disabled
        if meta.capabilities.needs_http() && !context.http_enabled {
            return false;
        }
        
        // Rules can always run if they have STREAMING_HTML or FULL_DOCUMENT
        // HTTP rules are checked above
        true
    }
    
    /// Execute the rule and return any issues found
    /// 
    /// This is called after HTML parsing is complete. The report contains
    /// all extracted data from the HTML.
    fn check(&self, report: &SeoReport, context: &RuleContext) -> Vec<SeoIssue>;
}

/// Helper macro to create a rule implementation
#[macro_export]
macro_rules! define_rule {
    (
        $name:ident,
        $code:expr,
        $rule_name:expr,
        $description:expr,
        $category:expr,
        $severity:expr,
        $capabilities:expr,
        $weight:expr,
        $check_fn:expr
    ) => {
        pub struct $name;

        impl $name {
            pub fn new() -> Self {
                Self
            }
        }

        impl $crate::rules::trait::Rule for $name {
            fn meta(&self) -> &$crate::rules::trait::RuleMeta {
                static META: $crate::rules::trait::RuleMeta = $crate::rules::trait::RuleMeta {
                    code: $code,
                    name: $rule_name,
                    description: $description,
                    category: $category,
                    severity: $severity,
                    capabilities: $capabilities,
                    weight: $weight,
                };
                &META
            }

            fn check(&self, report: &$crate::models::SeoReport, context: &$crate::rules::trait::RuleContext) -> Vec<$crate::models::SeoIssue> {
                $check_fn(report, context)
            }
        }
    };
}

