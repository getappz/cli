//! Pack options and result types for appz-code-mix.

use std::path::PathBuf;

/// Options for the pack command, mapped to Repomix CLI and pre-filter behavior.
#[derive(Debug, Clone, Default)]
pub struct PackOptions {
    /// Working directory (default: current)
    pub workdir: PathBuf,
    /// Output file path
    pub output: Option<PathBuf>,
    /// Output style: xml, markdown, json, plain
    pub style: Option<String>,
    /// Include glob patterns (comma-joined for Repomix)
    pub include: Vec<String>,
    /// Ignore glob patterns
    pub ignore: Vec<String>,
    /// Compress (Tree-sitter extraction)
    pub compress: bool,
    /// Remove comments
    pub remove_comments: bool,
    /// Remove empty lines
    pub remove_empty_lines: bool,
    /// Split output by size (e.g. "500kb", "1mb")
    pub split_output: Option<String>,
    /// Instruction file path for Repomix
    pub instruction: Option<PathBuf>,
    /// Custom header text
    pub header: Option<String>,
    /// Copy to clipboard
    pub copy: bool,
    /// Stdout instead of file
    pub stdout: bool,
    /// Content search strings (triggers ripgrep pre-filter)
    pub strings: Vec<String>,
    /// Exclude files containing these strings
    pub exclude_strings: Vec<String>,
    /// Git staged files only
    pub staged: bool,
    /// Git dirty (modified/untracked) only
    pub dirty: bool,
    /// Git diff from base branch
    pub diff: bool,
    /// Base branch for --diff (default: main)
    pub diff_base: Option<String>,
    /// Load bundle by name (skips TUI, uses saved paths)
    pub bundle: Option<String>,
    /// Use built-in template (writes to temp, passes as instruction)
    pub template: Option<String>,
    /// Select monorepo workspace (e.g. @org/pkg or packages/foo)
    pub workspace: Option<String>,
    /// Disable SHA-256 output cache
    pub no_cache: bool,
}
