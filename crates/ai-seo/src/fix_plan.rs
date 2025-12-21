//! Issue → Fix Plan Mapping
//!
//! This module provides deterministic, executable fix plans for SEO issues.
//! Fix plans bridge the gap between issue detection and automated fixes.

use crate::models::SiteSeoSummary;
use crate::routing::{route_fix, FixRouting};
use serde::Serialize;

/// Fix scope determines where a fix should be applied
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FixScope {
    /// Single page HTML
    Page,
    /// Section of pages (e.g., /blog/*, /docs/*)
    Section,
    /// Layout / shared head template
    Template,
    /// Global site configuration
    Site,
}

/// Fix priority for ordering and gating
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FixPriority {
    /// Breaks indexing / ranking
    P0,
    /// Ranking degradation
    P1,
    /// Hygiene / polish
    P2,
}

/// Risk classification for fix preview UX
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FixRisk {
    /// Attribute-only changes (safest)
    None,
    /// Head text only (meta tags, title)
    Low,
    /// Body text changes
    Medium,
    /// Structural changes (DOM manipulation)
    High,
}

/// Title generation strategy
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TitleStrategy {
    /// Extract from existing H1 tag
    FromH1,
    /// Use template variable
    FromTemplate,
    /// AI rewrite with character limit
    AiRewrite { max_chars: u8 },
}

/// Executable fix actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FixAction {
    /// Insert a meta tag
    InsertMeta {
        name: &'static str,
        content_hint: &'static str,
    },
    /// Update the title tag
    UpdateTitle {
        strategy: TitleStrategy,
    },
    /// Ensure exactly one H1 exists
    EnsureSingleH1,
    /// Add alt text to images
    AddImageAlt,
    /// Add lazy loading to images
    AddLazyLoading,
}

/// Complete fix plan for an issue
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixPlan {
    pub issue_code: &'static str,
    pub scope: FixScope,
    pub priority: FixPriority,
    pub actions: Vec<FixAction>,
    pub ai_allowed: bool,
    pub risk: FixRisk,
}

/// Determine risk level from fix actions
fn classify_risk(actions: &[FixAction]) -> FixRisk {
    for action in actions {
        match action {
            FixAction::InsertMeta { .. } | FixAction::UpdateTitle { .. } => {
                return FixRisk::Low; // Head text only
            }
            FixAction::AddImageAlt => {
                return FixRisk::Medium; // Content-adjacent
            }
            FixAction::EnsureSingleH1 => {
                return FixRisk::High; // Structural change
            }
            FixAction::AddLazyLoading => {
                return FixRisk::None; // Attribute-only
            }
        }
    }
    FixRisk::None
}

/// Get fix plan for an issue code
///
/// This is the core mapping table that determines what fixes should be applied
/// for each detected issue. The mapping is deterministic, testable, and versionable.
pub fn plan_for_issue(code: &str) -> Option<FixPlan> {
    match code {
        "SEO-META-001" => {
            let actions = vec![FixAction::UpdateTitle {
                strategy: TitleStrategy::AiRewrite { max_chars: 60 },
            }];
            Some(FixPlan {
                issue_code: "SEO-META-001",
                scope: FixScope::Template,
                priority: FixPriority::P0,
                ai_allowed: true,
                risk: classify_risk(&actions),
                actions,
            })
        }

        "SEO-META-002" => {
            let actions = vec![FixAction::InsertMeta {
                name: "description",
                content_hint: "Summarize page value in 150 characters",
            }];
            Some(FixPlan {
                issue_code: "SEO-META-002",
                scope: FixScope::Template,
                priority: FixPriority::P0,
                ai_allowed: true,
                risk: classify_risk(&actions),
                actions,
            })
        }

        "SEO-H1-001" => {
            let actions = vec![FixAction::EnsureSingleH1];
            Some(FixPlan {
                issue_code: "SEO-H1-001",
                scope: FixScope::Page,
                priority: FixPriority::P0,
                ai_allowed: false,
                risk: classify_risk(&actions),
                actions,
            })
        }

        "SEO-H1-002" => {
            let actions = vec![FixAction::EnsureSingleH1];
            Some(FixPlan {
                issue_code: "SEO-H1-002",
                scope: FixScope::Page,
                priority: FixPriority::P0,
                ai_allowed: false,
                risk: classify_risk(&actions),
                actions,
            })
        }

        "SEO-IMG-001" => {
            let actions = vec![FixAction::AddImageAlt];
            Some(FixPlan {
                issue_code: "SEO-IMG-001",
                scope: FixScope::Page,
                priority: FixPriority::P2,
                ai_allowed: true,
                risk: classify_risk(&actions),
                actions,
            })
        }

        "SEO-IMG-002" => {
            let actions = vec![FixAction::AddLazyLoading];
            Some(FixPlan {
                issue_code: "SEO-IMG-002",
                scope: FixScope::Page,
                priority: FixPriority::P2,
                ai_allowed: false,
                risk: classify_risk(&actions),
                actions,
            })
        }

        _ => None,
    }
}

/// Site-level fix plan with deduplication
///
/// Given site aggregation, this collapses multiple page-level issues into
/// a single fix plan when appropriate (e.g., template-level fixes).
#[derive(Debug, Clone, Serialize)]
pub struct SiteFixPlan {
    pub issue: &'static str,
    pub scope: FixScope,
    pub routing: FixRouting,
    pub affected_pages: usize,
    pub plan: FixPlan,
}

/// Generate site-level fix plans from aggregated issues
///
/// This uses data-driven routing to determine whether fixes should be applied
/// at the template, section, or page level. The routing prevents fixing the
/// same problem multiple times when a template fix would be more efficient.
///
/// # Arguments
/// * `site` - Site-level aggregation with all evidence needed for routing
///
/// # Returns
/// Vector of site fix plans with routing decisions
pub fn generate_site_fix_plans(site: &SiteSeoSummary) -> Vec<SiteFixPlan> {
    let mut plans = Vec::new();

    for issue_count in &site.issues.by_code {
        if let Some(plan) = plan_for_issue(issue_count.code) {
            // Use routing to determine where the fix should be applied
            let routing = route_fix(issue_count.code, site);

            // Map routing to scope for the plan
            // Note: The plan's scope is the base scope from the registry,
            // but routing may override it based on site-level evidence
            let effective_scope = match &routing {
                FixRouting::Template => FixScope::Template,
                FixRouting::Section { .. } => FixScope::Section,
                FixRouting::Page => FixScope::Page,
            };

            plans.push(SiteFixPlan {
                issue: issue_count.code,
                scope: effective_scope,
                routing,
                affected_pages: issue_count.affected_pages,
                plan,
            });
        }
    }

    // Sort by priority (P0 first) then by affected pages (descending)
    plans.sort_by(|a, b| {
        let priority_order = |p: &FixPriority| match p {
            FixPriority::P0 => 0,
            FixPriority::P1 => 1,
            FixPriority::P2 => 2,
        };
        priority_order(&a.plan.priority)
            .cmp(&priority_order(&b.plan.priority))
            .then_with(|| b.affected_pages.cmp(&a.affected_pages))
    });

    plans
}

