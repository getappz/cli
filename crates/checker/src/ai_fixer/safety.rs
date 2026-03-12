//! Safety guardrails for AI-assisted repair.
//!
//! Prevents the AI from making destructive changes, modifying protected
//! files, or exceeding change thresholds. All limits are configurable.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CheckResult, CheckerError};

use super::patch::Patch;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Safety configuration for AI repair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    /// Maximum percentage of lines that can change in a single file.
    #[serde(default = "default_max_change_pct", rename = "maxChangePct")]
    pub max_change_pct: f32,

    /// Maximum number of files a single patch can modify.
    #[serde(default = "default_max_files_per_patch", rename = "maxFilesPerPatch")]
    pub max_files_per_patch: usize,

    /// Glob patterns for files that are read-only for AI (never patched).
    #[serde(default = "default_protected_files", rename = "protectedFiles")]
    pub protected_files: Vec<String>,

    /// Glob patterns for files never sent to the AI (secrets, credentials).
    #[serde(default = "default_never_send", rename = "neverSend")]
    pub never_send: Vec<String>,

    /// Minimum confidence score (0.0-1.0) to auto-apply a patch.
    #[serde(default = "default_min_confidence", rename = "minConfidence")]
    pub min_confidence: f32,

    /// Maximum retry attempts per error batch.
    #[serde(default = "default_max_attempts", rename = "maxAttempts")]
    pub max_attempts: u32,
}

fn default_max_change_pct() -> f32 {
    40.0
}
fn default_max_files_per_patch() -> usize {
    5
}
fn default_protected_files() -> Vec<String> {
    vec![
        ".env*".into(),
        "*.lock".into(),
        "node_modules/**".into(),
        "target/**".into(),
        ".git/**".into(),
        "dist/**".into(),
        "build/**".into(),
    ]
}
fn default_never_send() -> Vec<String> {
    vec![
        ".env".into(),
        ".env.*".into(),
        "credentials.*".into(),
        "*.pem".into(),
        "*.key".into(),
        "secrets.*".into(),
        "*.secret".into(),
    ]
}
fn default_min_confidence() -> f32 {
    0.6
}
fn default_max_attempts() -> u32 {
    3
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            max_change_pct: default_max_change_pct(),
            max_files_per_patch: default_max_files_per_patch(),
            protected_files: default_protected_files(),
            never_send: default_never_send(),
            min_confidence: default_min_confidence(),
            max_attempts: default_max_attempts(),
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Check whether a file path matches any pattern in a glob list.
fn matches_any_pattern(path: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if let Ok(matcher) = glob::Pattern::new(pattern) {
            if matcher.matches(path) {
                return true;
            }
        }
        // Also check if the path starts with the pattern (for directory patterns).
        let dir_prefix = pattern.trim_end_matches("/**");
        if path.starts_with(dir_prefix) && dir_prefix != pattern {
            return true;
        }
    }
    false
}

/// Check if a file is protected (should never be patched by AI).
pub fn is_protected(path: &Path, config: &SafetyConfig) -> bool {
    let path_str = path.display().to_string();
    matches_any_pattern(&path_str, &config.protected_files)
}

/// Check if a file should never be sent to the AI.
pub fn is_never_send(path: &Path, config: &SafetyConfig) -> bool {
    let path_str = path.display().to_string();
    matches_any_pattern(&path_str, &config.never_send)
}

/// Validate a patch against all safety guardrails.
///
/// Returns `Ok(())` if the patch is safe, or an error describing
/// the first violation found.
pub fn validate_safety(patch: &Patch, config: &SafetyConfig) -> CheckResult<()> {
    // 1. Check file count.
    if patch.file_patches.len() > config.max_files_per_patch {
        return Err(CheckerError::AiFixFailed {
            reason: format!(
                "Safety: patch modifies {} files (limit: {})",
                patch.file_patches.len(),
                config.max_files_per_patch
            ),
        });
    }

    // 2. Check for protected files.
    for fp in &patch.file_patches {
        if is_protected(&fp.path, config) {
            return Err(CheckerError::AiFixFailed {
                reason: format!(
                    "Safety: patch attempts to modify protected file: {}",
                    fp.path.display()
                ),
            });
        }
    }

    // 3. Check for empty patches.
    let total_changes: usize = patch
        .file_patches
        .iter()
        .flat_map(|fp| &fp.hunks)
        .flat_map(|h| &h.lines)
        .filter(|l| !matches!(l, super::patch::HunkLine::Context(_)))
        .count();

    if total_changes == 0 {
        return Err(CheckerError::AiFixFailed {
            reason: "Safety: patch contains no actual changes".to_string(),
        });
    }

    Ok(())
}

/// Check if a confidence score meets the minimum threshold.
pub fn confidence_ok(score: f32, config: &SafetyConfig) -> bool {
    score >= config.min_confidence
}

/// Filter issues to exclude files that should never be sent to AI.
pub fn filter_sendable_issues(
    issues: &[crate::output::CheckIssue],
    config: &SafetyConfig,
) -> Vec<crate::output::CheckIssue> {
    issues
        .iter()
        .filter(|i| !is_never_send(&i.file, config))
        .cloned()
        .collect()
}
