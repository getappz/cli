//! Unified output model for all check providers.
//!
//! Every provider parses its tool-specific output into [`CheckIssue`] structs,
//! enabling unified display, filtering, caching, and fix orchestration.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Severity level of a check issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Hints and suggestions for improvement.
    Hint,
    /// Informational messages.
    Info,
    /// Warnings that may indicate problems.
    Warning,
    /// Errors that must be fixed.
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hint => write!(f, "hint"),
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Kind of fix available for an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FixKind {
    /// Safe to apply automatically — no semantic changes.
    Safe,
    /// May change semantics — needs user confirmation.
    Unsafe,
    /// Suggested by AI — always requires human review.
    AiSuggested,
}

/// A suggested fix for an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSuggestion {
    /// Whether this fix is safe, unsafe, or AI-suggested.
    pub kind: FixKind,
    /// Human-readable description of what the fix does.
    pub description: String,
    /// The replacement text (if available for inline fixes).
    pub replacement: Option<String>,
}

/// A single issue found by a check provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckIssue {
    /// File path relative to project root.
    pub file: PathBuf,
    /// Starting line number (1-based).
    pub line: Option<u32>,
    /// Starting column number (1-based).
    pub column: Option<u32>,
    /// Ending line number (1-based).
    pub end_line: Option<u32>,
    /// Ending column number (1-based).
    pub end_column: Option<u32>,
    /// Issue severity.
    pub severity: Severity,
    /// Rule/error code (e.g. "no-unused-vars", "E501", "E0382").
    pub code: Option<String>,
    /// Human-readable description of the issue.
    pub message: String,
    /// Provider slug that found this issue (e.g. "biome", "ruff").
    pub source: String,
    /// Available fix, if any.
    pub fix: Option<FixSuggestion>,
}

impl CheckIssue {
    /// Create a new issue with required fields.
    pub fn new(
        file: impl Into<PathBuf>,
        severity: Severity,
        message: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            file: file.into(),
            line: None,
            column: None,
            end_line: None,
            end_column: None,
            severity,
            code: None,
            message: message.into(),
            source: source.into(),
            fix: None,
        }
    }

    /// Set line/column location.
    pub fn at(mut self, line: u32, column: u32) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    /// Set end location.
    pub fn to(mut self, end_line: u32, end_column: u32) -> Self {
        self.end_line = Some(end_line);
        self.end_column = Some(end_column);
        self
    }

    /// Set the rule/error code.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Attach a fix suggestion.
    pub fn with_fix(mut self, fix: FixSuggestion) -> Self {
        self.fix = Some(fix);
        self
    }

    /// Whether this issue has a safe auto-fix available.
    pub fn has_safe_fix(&self) -> bool {
        self.fix
            .as_ref()
            .map(|f| f.kind == FixKind::Safe)
            .unwrap_or(false)
    }
}

/// Report of fixes applied by a provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FixReport {
    /// Number of issues fixed.
    pub fixed_count: usize,
    /// Number of issues remaining after fix.
    pub remaining_count: usize,
    /// Issues that were fixed (for reporting).
    pub fixed_issues: Vec<String>,
    /// Issues that could not be fixed.
    pub remaining_issues: Vec<CheckIssue>,
}

/// Aggregated report from a full check run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReport {
    /// All issues found across all providers.
    pub issues: Vec<CheckIssue>,
    /// Number of files checked.
    pub files_checked: usize,
    /// Total duration of the check run.
    #[serde(with = "duration_serde")]
    pub duration: Duration,
    /// Provider slugs that were executed.
    pub providers_run: Vec<String>,
    /// Number of errors.
    pub error_count: usize,
    /// Number of warnings.
    pub warning_count: usize,
    /// Fix report, if --fix was used.
    pub fix_report: Option<FixReport>,
}

impl CheckReport {
    /// Create a new empty report.
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            files_checked: 0,
            duration: Duration::ZERO,
            providers_run: Vec::new(),
            error_count: 0,
            warning_count: 0,
            fix_report: None,
        }
    }

    /// Recompute error/warning counts from the issues list.
    pub fn recount(&mut self) {
        self.error_count = self.issues.iter().filter(|i| i.severity == Severity::Error).count();
        self.warning_count = self
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count();
    }

    /// Whether the check passed (no errors; warnings allowed unless strict).
    pub fn passed(&self, strict: bool) -> bool {
        if strict {
            self.error_count == 0 && self.warning_count == 0
        } else {
            self.error_count == 0
        }
    }
}

impl Default for CheckReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Serde support for `Duration` as milliseconds.
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        d.as_millis().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let ms = u64::deserialize(d)?;
        Ok(Duration::from_millis(ms))
    }
}
