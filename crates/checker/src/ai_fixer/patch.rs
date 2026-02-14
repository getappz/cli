//! Unified diff parsing and safe patch application.
//!
//! Instead of full file rewrites, patches are applied as line-level edits
//! parsed from unified diff format. Each patch is validated against safety
//! thresholds before application.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{CheckResult, CheckerError};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A complete patch that may modify multiple files.
#[derive(Debug, Clone)]
pub struct Patch {
    /// Per-file hunks.
    pub file_patches: Vec<FilePatch>,
}

/// Patch for a single file.
#[derive(Debug, Clone)]
pub struct FilePatch {
    /// The file path (from the `---`/`+++` header).
    pub path: PathBuf,
    /// The hunks (contiguous change regions).
    pub hunks: Vec<Hunk>,
}

/// A single hunk within a file patch.
#[derive(Debug, Clone)]
pub struct Hunk {
    /// Original file start line (1-based).
    pub old_start: usize,
    /// Number of lines in the original.
    pub old_count: usize,
    /// New file start line (1-based).
    pub new_start: usize,
    /// Number of lines in the new file.
    pub new_count: usize,
    /// The hunk lines (context, additions, deletions).
    pub lines: Vec<HunkLine>,
}

/// A single line within a hunk.
#[derive(Debug, Clone)]
pub enum HunkLine {
    /// Unchanged context line.
    Context(String),
    /// Line added.
    Add(String),
    /// Line removed.
    Remove(String),
}

/// Result of applying a patch to a file.
#[derive(Debug, Clone)]
pub struct PatchResult {
    /// The new file content after patching.
    pub content: String,
    /// Lines added.
    pub lines_added: usize,
    /// Lines removed.
    pub lines_removed: usize,
    /// Percentage of original lines changed.
    pub change_pct: f32,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a unified diff string into a [`Patch`].
///
/// Handles both `diff --git` and plain `---`/`+++` formats.
pub fn parse_unified_diff(diff_text: &str) -> CheckResult<Patch> {
    let lines: Vec<&str> = diff_text.lines().collect();
    let mut file_patches = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Skip `diff --git` headers.
        if line.starts_with("diff --git") {
            i += 1;
            // Skip index, old mode, new mode lines.
            while i < lines.len()
                && !lines[i].starts_with("---")
                && !lines[i].starts_with("diff --git")
            {
                i += 1;
            }
            continue;
        }

        // Parse file header.
        if line.starts_with("---") {
            let old_path = parse_file_header(line, "---");
            i += 1;
            if i >= lines.len() || !lines[i].starts_with("+++") {
                return Err(CheckerError::AiFixFailed {
                    reason: "Malformed diff: expected +++ after ---".to_string(),
                });
            }
            let new_path = parse_file_header(lines[i], "+++");
            i += 1;

            // Use the new path (or old path for deletions).
            let path = if new_path == "/dev/null" {
                PathBuf::from(old_path)
            } else {
                PathBuf::from(new_path)
            };

            // Parse hunks for this file.
            let mut hunks = Vec::new();
            while i < lines.len() && !lines[i].starts_with("diff --git") && !lines[i].starts_with("---") {
                if lines[i].starts_with("@@") {
                    let (hunk, consumed) = parse_hunk(&lines[i..])?;
                    hunks.push(hunk);
                    i += consumed;
                } else {
                    i += 1;
                }
            }

            if !hunks.is_empty() {
                file_patches.push(FilePatch { path, hunks });
            }
            continue;
        }

        i += 1;
    }

    if file_patches.is_empty() {
        return Err(CheckerError::AiFixFailed {
            reason: "No file patches found in diff output".to_string(),
        });
    }

    Ok(Patch { file_patches })
}

/// Parse a `---` or `+++` header line to extract the file path.
fn parse_file_header(line: &str, prefix: &str) -> String {
    let path = line
        .strip_prefix(prefix)
        .unwrap_or(line)
        .trim();

    // Strip `a/` or `b/` prefixes from git diffs.
    let path = path
        .strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path);

    // Strip timestamp suffix (e.g. "2024-01-01 00:00:00.000000000 +0000").
    path.split('\t').next().unwrap_or(path).to_string()
}

/// Parse a single hunk starting from a `@@` line.
///
/// Returns the parsed hunk and the number of lines consumed.
fn parse_hunk(lines: &[&str]) -> CheckResult<(Hunk, usize)> {
    let header = lines[0];

    // Parse @@ -old_start,old_count +new_start,new_count @@
    let (old_start, old_count, new_start, new_count) = parse_hunk_header(header)?;

    let mut hunk_lines = Vec::new();
    let mut consumed = 1; // The @@ line itself.

    for line in &lines[1..] {
        if line.starts_with("@@") || line.starts_with("diff --git") || line.starts_with("---") {
            break;
        }

        if let Some(content) = line.strip_prefix('+') {
            hunk_lines.push(HunkLine::Add(content.to_string()));
        } else if let Some(content) = line.strip_prefix('-') {
            hunk_lines.push(HunkLine::Remove(content.to_string()));
        } else if let Some(content) = line.strip_prefix(' ') {
            hunk_lines.push(HunkLine::Context(content.to_string()));
        } else if line.is_empty() {
            // Empty lines in diff are context lines with no leading space.
            hunk_lines.push(HunkLine::Context(String::new()));
        } else if *line == "\\ No newline at end of file" {
            // Skip this marker.
        } else {
            // Treat as context line (some diff generators omit the leading space).
            hunk_lines.push(HunkLine::Context(line.to_string()));
        }

        consumed += 1;
    }

    Ok((
        Hunk {
            old_start,
            old_count,
            new_start,
            new_count,
            lines: hunk_lines,
        },
        consumed,
    ))
}

/// Parse `@@ -a,b +c,d @@` header.
fn parse_hunk_header(header: &str) -> CheckResult<(usize, usize, usize, usize)> {
    // Find the range specs between @@ markers.
    let inner = header
        .strip_prefix("@@")
        .and_then(|s| {
            let end = s.find("@@").unwrap_or(s.len());
            Some(s[..end].trim())
        })
        .ok_or_else(|| CheckerError::AiFixFailed {
            reason: format!("Malformed hunk header: {}", header),
        })?;

    let parts: Vec<&str> = inner.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(CheckerError::AiFixFailed {
            reason: format!("Malformed hunk header: {}", header),
        });
    }

    let (old_start, old_count) = parse_range(parts[0].trim_start_matches('-'))?;
    let (new_start, new_count) = parse_range(parts[1].trim_start_matches('+'))?;

    Ok((old_start, old_count, new_start, new_count))
}

/// Parse a range spec like "10,5" or "10" (count defaults to 1).
fn parse_range(s: &str) -> CheckResult<(usize, usize)> {
    let parts: Vec<&str> = s.split(',').collect();
    let start: usize = parts[0].parse().map_err(|_| CheckerError::AiFixFailed {
        reason: format!("Invalid range number: {}", s),
    })?;
    let count: usize = if parts.len() > 1 {
        parts[1].parse().map_err(|_| CheckerError::AiFixFailed {
            reason: format!("Invalid range count: {}", s),
        })?
    } else {
        1
    };
    Ok((start, count))
}

// ---------------------------------------------------------------------------
// Fuzzy sequence matching (ported from OpenAI Codex apply-patch)
// ---------------------------------------------------------------------------

/// Find `pattern` lines within `lines` starting at or after `start`.
///
/// Uses a 4-pass strategy with decreasing strictness:
/// 1. **Exact** -- byte-for-byte match
/// 2. **Rstrip** -- ignores trailing whitespace
/// 3. **Trim** -- ignores leading and trailing whitespace
/// 4. **Unicode normalise** -- normalises typographic dashes, quotes, spaces
///
/// Returns the starting index of the match, or `None`.
fn seek_sequence(lines: &[String], pattern: &[String], start: usize) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    if pattern.len() > lines.len() {
        return None;
    }

    let end = lines.len().saturating_sub(pattern.len());

    // Pass 1: exact.
    for i in start..=end {
        if lines[i..i + pattern.len()] == *pattern {
            return Some(i);
        }
    }

    // Pass 2: rstrip.
    for i in start..=end {
        let ok = pattern.iter().enumerate().all(|(j, pat)| {
            lines[i + j].trim_end() == pat.trim_end()
        });
        if ok {
            return Some(i);
        }
    }

    // Pass 3: trim both sides.
    for i in start..=end {
        let ok = pattern.iter().enumerate().all(|(j, pat)| {
            lines[i + j].trim() == pat.trim()
        });
        if ok {
            return Some(i);
        }
    }

    // Pass 4: Unicode normalisation.
    for i in start..=end {
        let ok = pattern.iter().enumerate().all(|(j, pat)| {
            normalise_unicode(&lines[i + j]) == normalise_unicode(pat)
        });
        if ok {
            return Some(i);
        }
    }

    None
}

/// Normalise common Unicode punctuation to ASCII equivalents.
fn normalise_unicode(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            // Various dashes → ASCII '-'
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            // Fancy single quotes → '\''
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
            // Fancy double quotes → '"'
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
            // Non-breaking / special spaces → regular space
            '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
            | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
            | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

/// Apply a patch to file contents.
///
/// `files` maps file paths to their current content. Returns a map of
/// file paths to their patched content + statistics.
pub fn apply_patch(
    patch: &Patch,
    files: &HashMap<PathBuf, String>,
) -> CheckResult<HashMap<PathBuf, PatchResult>> {
    let mut results = HashMap::new();

    for fp in &patch.file_patches {
        let original = files.get(&fp.path).ok_or_else(|| CheckerError::AiFixFailed {
            reason: format!("Patch references unknown file: {}", fp.path.display()),
        })?;

        let result = apply_file_patch(original, &fp.hunks)?;
        results.insert(fp.path.clone(), result);
    }

    Ok(results)
}

/// Apply hunks to a single file's content using fuzzy content matching.
///
/// Instead of trusting line numbers from the diff, each hunk's removed +
/// context lines are located in the file via [`seek_sequence`] (4-pass
/// fuzzy matching). Replacements are collected and then applied in
/// **reverse order** to avoid index shifts.
fn apply_file_patch(original: &str, hunks: &[Hunk]) -> CheckResult<PatchResult> {
    let original_lines: Vec<String> = original.lines().map(String::from).collect();
    let total_original = original_lines.len();

    // Build a (start_idx, old_len, new_lines) replacement for each hunk.
    let mut replacements: Vec<(usize, usize, Vec<String>)> = Vec::new();

    // Track the minimum search position so we don't match the same region
    // twice when hunks are ordered top-to-bottom.
    let mut search_from = 0usize;

    // Sort hunks by old_start for deterministic ordering.
    let mut sorted_hunks: Vec<&Hunk> = hunks.iter().collect();
    sorted_hunks.sort_by_key(|h| h.old_start);

    for hunk in &sorted_hunks {
        // Collect the "old" lines for this hunk (context + remove lines, in order).
        let old_lines: Vec<String> = hunk
            .lines
            .iter()
            .filter_map(|hl| match hl {
                HunkLine::Context(c) => Some(c.clone()),
                HunkLine::Remove(c) => Some(c.clone()),
                HunkLine::Add(_) => None,
            })
            .collect();

        // Collect the "new" lines (context + add lines, in order).
        let new_lines: Vec<String> = hunk
            .lines
            .iter()
            .filter_map(|hl| match hl {
                HunkLine::Context(c) => Some(c.clone()),
                HunkLine::Add(c) => Some(c.clone()),
                HunkLine::Remove(_) => None,
            })
            .collect();

        if old_lines.is_empty() {
            // Pure addition: insert at the hunk's target position.
            let insert_at = if hunk.old_start > 0 {
                (hunk.old_start - 1).min(total_original)
            } else {
                0
            };
            replacements.push((insert_at, 0, new_lines));
            continue;
        }

        // Try fuzzy content matching first.
        let match_start = seek_sequence(&original_lines, &old_lines, search_from);

        // Fallback: try from the hunk's stated line number.
        let match_start = match_start.or_else(|| {
            let hint = if hunk.old_start > 0 {
                hunk.old_start - 1
            } else {
                0
            };
            seek_sequence(&original_lines, &old_lines, hint)
        });

        // Last resort: try from the very beginning.
        let match_start = match_start.or_else(|| {
            if search_from > 0 {
                seek_sequence(&original_lines, &old_lines, 0)
            } else {
                None
            }
        });

        match match_start {
            Some(idx) => {
                replacements.push((idx, old_lines.len(), new_lines));
                search_from = idx + old_lines.len();
            }
            None => {
                return Err(CheckerError::AiFixFailed {
                    reason: format!(
                        "Could not locate hunk in file (expected near line {}). \
                         First old line: {:?}",
                        hunk.old_start,
                        old_lines.first().map(|s| s.as_str()).unwrap_or("(empty)")
                    ),
                });
            }
        }
    }

    // Sort by start index ascending, then apply in reverse to preserve indices.
    replacements.sort_by_key(|(start, _, _)| *start);

    let mut lines = original_lines;

    for (start, old_len, new_lines) in replacements.iter().rev() {
        // Remove old lines.
        let end = (*start + old_len).min(lines.len());
        lines.drain(*start..end);

        // Insert new lines.
        for (j, line) in new_lines.iter().enumerate() {
            lines.insert(start + j, line.clone());
        }
    }

    // Count actual add/remove lines from the hunks (excluding context).
    let actual_removed: usize = hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| matches!(l, HunkLine::Remove(_)))
        .count();
    let actual_added: usize = hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| matches!(l, HunkLine::Add(_)))
        .count();

    let change_pct = if total_original == 0 {
        if actual_added > 0 { 100.0 } else { 0.0 }
    } else {
        ((actual_added + actual_removed) as f32 / total_original as f32) * 100.0
    };

    Ok(PatchResult {
        content: lines.join("\n"),
        lines_added: actual_added,
        lines_removed: actual_removed,
        change_pct,
    })
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate a patch against safety thresholds.
///
/// Returns `Ok(())` if the patch is safe to apply, or an error describing
/// why it was rejected.
pub fn validate_patch(
    patch: &Patch,
    files: &HashMap<PathBuf, String>,
    max_change_pct: f32,
    max_files: usize,
) -> CheckResult<()> {
    // Check file count.
    if patch.file_patches.len() > max_files {
        return Err(CheckerError::AiFixFailed {
            reason: format!(
                "Patch modifies {} files (max allowed: {})",
                patch.file_patches.len(),
                max_files
            ),
        });
    }

    // Check per-file change percentage.
    for fp in &patch.file_patches {
        if let Some(original) = files.get(&fp.path) {
            let result = apply_file_patch(original, &fp.hunks)?;
            if result.change_pct > max_change_pct {
                return Err(CheckerError::AiFixFailed {
                    reason: format!(
                        "Patch changes {:.1}% of {} (max allowed: {:.1}%)",
                        result.change_pct,
                        fp.path.display(),
                        max_change_pct
                    ),
                });
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

/// Display a colourful diff preview of the patch.
pub fn display_patch_preview(patch: &Patch) {
    for fp in &patch.file_patches {
        let _ = ui::status::info(&format!("--- {}", fp.path.display()));

        for hunk in &fp.hunks {
            let _ = ui::status::info(&format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
            ));

            let mut shown = 0;
            for line in &hunk.lines {
                if shown >= 60 {
                    let _ = ui::status::info("  ... (more lines)");
                    break;
                }
                match line {
                    HunkLine::Context(c) => {
                        let _ = ui::status::info(&format!("  {}", c));
                    }
                    HunkLine::Add(c) => {
                        let _ = ui::status::success(&format!("+ {}", c));
                    }
                    HunkLine::Remove(c) => {
                        let _ = ui::status::error(&format!("- {}", c));
                    }
                }
                shown += 1;
            }
        }
    }
}
