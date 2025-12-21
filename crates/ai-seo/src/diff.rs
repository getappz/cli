//! Dry-Run Diff Engine
//!
//! This module provides a production-grade dry-run diff engine that tracks
//! HTML mutations during fix plan execution. It produces minimal, human-readable
//! diffs that are CI-safe and deterministic.
//!
//! The diff engine is HTML-aware and mutation-aware, avoiding generic line-based
//! diffs that would introduce noise from HTML reformatting or DOM re-serialization.

use serde::Serialize;
use std::cell::RefCell;
use std::collections::HashSet;

/// Mutation event recorded during HTML rewriting
///
/// This captures exactly what changed during a single mutation operation,
/// including the issue code that triggered it and the before/after HTML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationEvent {
    pub issue_code: &'static str,
    pub before: String,
    pub after: String,
}

/// Change type classification for diff hunks
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    /// New element/attribute was inserted
    Insert,
    /// Existing element/attribute was updated
    Update,
    /// Element/attribute was removed
    Remove,
}

/// Atomic change unit in the diff
///
/// Each hunk represents a single mutation with its context (issue code,
/// change type, and before/after HTML).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffHunk {
    pub issue_code: &'static str,
    pub change_type: ChangeType,
    pub before: String,
    pub after: String,
}

/// Summary statistics for a dry-run diff
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub affected_issues: Vec<&'static str>,
}

/// Complete dry-run diff report
///
/// This is the stable output contract for dry-run operations. It provides
/// a summary and detailed hunks of all changes that would be applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DryRunDiff {
    pub url: String,
    pub summary: DiffSummary,
    pub hunks: Vec<DiffHunk>,
}

/// Thread-safe diff recorder for mutation events
///
/// This recorder collects mutation events during HTML rewriting. It uses
/// RefCell internally for single-threaded mutation tracking (lol_html is
/// single-threaded).
pub struct DiffRecorder {
    events: RefCell<Vec<MutationEvent>>,
}

impl DiffRecorder {
    /// Create a new empty diff recorder
    pub fn new() -> Self {
        Self {
            events: RefCell::new(Vec::new()),
        }
    }

    /// Record a mutation event
    ///
    /// This is called during HTML rewriting when a mutation occurs.
    /// The before/after HTML should be the outer HTML of the affected element.
    pub fn record(&self, event: MutationEvent) {
        self.events.borrow_mut().push(event);
    }

    /// Take all recorded events, consuming the recorder
    pub fn take_events(self) -> Vec<MutationEvent> {
        self.events.into_inner()
    }

    /// Get a reference to the events (for inspection without consuming)
    pub fn events(&self) -> Vec<MutationEvent> {
        self.events.borrow().clone()
    }
}

impl Default for DiffRecorder {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a dry-run diff from mutation events
///
/// This function processes mutation events and generates a structured diff
/// report with summary statistics and detailed hunks.
pub fn build_diff(url: &str, events: Vec<MutationEvent>) -> DryRunDiff {
    let mut summary = DiffSummary::default();
    let mut hunks = Vec::new();
    let mut seen_issues = HashSet::new();

    for event in events {
        // Determine change type based on before/after comparison
        let change_type = if event.before.is_empty() {
            ChangeType::Insert
        } else if event.after.is_empty() {
            ChangeType::Remove
        } else {
            ChangeType::Update
        };

        // Update summary counts
        match change_type {
            ChangeType::Insert => summary.added += 1,
            ChangeType::Remove => summary.removed += 1,
            ChangeType::Update => summary.modified += 1,
        }

        // Track unique issue codes
        if !seen_issues.contains(event.issue_code) {
            seen_issues.insert(event.issue_code);
            summary.affected_issues.push(event.issue_code);
        }

        // Create hunk
        hunks.push(DiffHunk {
            issue_code: event.issue_code,
            change_type,
            before: event.before,
            after: event.after,
        });
    }

    // Sort affected issues for deterministic output
    summary.affected_issues.sort();

    DryRunDiff {
        url: url.to_string(),
        summary,
        hunks,
    }
}

/// Generate a unified diff format string from a dry-run diff
///
/// This produces a human-readable diff format similar to `git diff` for
/// CLI display and review.
pub fn format_unified_diff(diff: &DryRunDiff) -> String {
    let mut output = String::new();
    output.push_str(&format!("--- {}\n", diff.url));
    output.push_str(&format!("+++ {}\n", diff.url));
    output.push_str(&format!(
        "Summary: {} added, {} removed, {} modified\n",
        diff.summary.added, diff.summary.removed, diff.summary.modified
    ));
    output.push_str(&format!(
        "Affected issues: {}\n\n",
        diff.summary.affected_issues.join(", ")
    ));

    for hunk in &diff.hunks {
        output.push_str(&format!("Issue: {}\n", hunk.issue_code));
        output.push_str(&format!("Type: {:?}\n", hunk.change_type));
        if !hunk.before.is_empty() {
            output.push_str(&format!("- {}\n", hunk.before));
        }
        if !hunk.after.is_empty() {
            output.push_str(&format!("+ {}\n", hunk.after));
        }
        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_recorder() {
        let recorder = DiffRecorder::new();
        recorder.record(MutationEvent {
            issue_code: "SEO-META-002",
            before: "<meta name=\"description\">".to_string(),
            after: "<meta name=\"description\" content=\"Test\">".to_string(),
        });

        let events = recorder.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].issue_code, "SEO-META-002");
    }

    #[test]
    fn test_build_diff_insert() {
        let events = vec![MutationEvent {
            issue_code: "SEO-META-002",
            before: String::new(),
            after: "<meta name=\"description\" content=\"Test\">".to_string(),
        }];

        let diff = build_diff("/test", events);
        assert_eq!(diff.summary.added, 1);
        assert_eq!(diff.summary.removed, 0);
        assert_eq!(diff.summary.modified, 0);
        assert_eq!(diff.hunks[0].change_type, ChangeType::Insert);
    }

    #[test]
    fn test_build_diff_update() {
        let events = vec![MutationEvent {
            issue_code: "SEO-META-002",
            before: "<meta name=\"description\">".to_string(),
            after: "<meta name=\"description\" content=\"Test\">".to_string(),
        }];

        let diff = build_diff("/test", events);
        assert_eq!(diff.summary.added, 0);
        assert_eq!(diff.summary.removed, 0);
        assert_eq!(diff.summary.modified, 1);
        assert_eq!(diff.hunks[0].change_type, ChangeType::Update);
    }

    #[test]
    fn test_build_diff_remove() {
        let events = vec![MutationEvent {
            issue_code: "SEO-H1-002",
            before: "<h1>Duplicate</h1>".to_string(),
            after: String::new(),
        }];

        let diff = build_diff("/test", events);
        assert_eq!(diff.summary.added, 0);
        assert_eq!(diff.summary.removed, 1);
        assert_eq!(diff.summary.modified, 0);
        assert_eq!(diff.hunks[0].change_type, ChangeType::Remove);
    }

    #[test]
    fn test_build_diff_multiple_issues() {
        let events = vec![
            MutationEvent {
                issue_code: "SEO-META-002",
                before: "<meta name=\"description\">".to_string(),
                after: "<meta name=\"description\" content=\"Test\">".to_string(),
            },
            MutationEvent {
                issue_code: "SEO-IMG-001",
                before: "<img src=\"test.jpg\">".to_string(),
                after: "<img src=\"test.jpg\" alt=\"Image\">".to_string(),
            },
        ];

        let diff = build_diff("/test", events);
        assert_eq!(diff.summary.modified, 2);
        assert_eq!(diff.summary.affected_issues.len(), 2);
        assert!(diff.summary.affected_issues.contains(&"SEO-META-002"));
        assert!(diff.summary.affected_issues.contains(&"SEO-IMG-001"));
    }

    #[test]
    fn test_format_unified_diff() {
        let events = vec![MutationEvent {
            issue_code: "SEO-META-002",
            before: "<meta name=\"description\">".to_string(),
            after: "<meta name=\"description\" content=\"Test\">".to_string(),
        }];

        let diff = build_diff("/test", events);
        let formatted = format_unified_diff(&diff);
        assert!(formatted.contains("SEO-META-002"));
        assert!(formatted.contains("- <meta name=\"description\">"));
        assert!(formatted.contains("+ <meta name=\"description\" content=\"Test\">"));
    }
}

