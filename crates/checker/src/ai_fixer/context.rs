//! Context collection for AI repair.
//!
//! Gathers the minimal set of files, imports, and configuration that the
//! AI model needs to understand and fix an error. Caps total context size
//! to avoid exceeding model token limits.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::output::CheckIssue;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum context size in bytes (60 KB).
const DEFAULT_MAX_CONTEXT_BYTES: usize = 60 * 1024;

/// Number of lines of context around the error region in large files.
const ERROR_REGION_PADDING: usize = 50;

/// Maximum file size (in lines) before we switch to region-only extraction.
const LARGE_FILE_THRESHOLD: usize = 500;

/// Config files that are always included if they exist.
const CONFIG_FILES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "tsconfig.json",
    "pyproject.toml",
    "ruff.toml",
    "biome.json",
    ".eslintrc.json",
    "composer.json",
    "phpstan.neon",
];

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Collected context for an AI repair session.
#[derive(Debug, Clone)]
pub struct RepairContext {
    /// The primary files where errors occur, with their content.
    pub error_files: HashMap<PathBuf, FileContext>,
    /// Related files (imports, configs) with their content.
    pub related_files: HashMap<PathBuf, String>,
    /// The project file tree (top-level listing).
    pub file_tree: String,
    /// Total bytes of context collected.
    pub total_bytes: usize,
    /// Maximum context bytes allowed.
    pub max_bytes: usize,
}

/// Context for a single file that has errors.
#[derive(Debug, Clone)]
pub struct FileContext {
    /// Full file content (or region if file is large).
    pub content: String,
    /// Whether this is a region extraction (not the full file).
    pub is_region: bool,
    /// If region: the starting line number (1-based) of the region.
    pub region_start_line: Option<u32>,
    /// Issues in this file.
    pub issues: Vec<CheckIssue>,
}

// ---------------------------------------------------------------------------
// Import extraction (regex-based, no AST)
// ---------------------------------------------------------------------------

/// Extract import paths from file content.
///
/// Supports:
/// - Rust: `use crate::foo::bar;`, `mod foo;`
/// - JS/TS: `import ... from "..."`, `require("...")`
/// - Python: `import foo`, `from foo import bar`
pub fn extract_imports(content: &str, file_path: &Path) -> Vec<String> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" => extract_rust_imports(content),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => extract_js_imports(content),
        "py" => extract_python_imports(content),
        "php" => extract_php_imports(content),
        _ => Vec::new(),
    }
}

fn extract_rust_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        // `use crate::module::...;`
        if trimmed.starts_with("use crate::") {
            if let Some(path) = trimmed
                .strip_prefix("use crate::")
                .and_then(|s| s.split("::").next())
            {
                imports.push(path.trim_end_matches(';').to_string());
            }
        }
        // `use super::...;`
        if trimmed.starts_with("use super::") {
            if let Some(path) = trimmed
                .strip_prefix("use super::")
                .and_then(|s| s.split("::").next())
            {
                imports.push(path.trim_end_matches(';').to_string());
            }
        }
        // `mod foo;`
        if trimmed.starts_with("mod ") && trimmed.ends_with(';') {
            let module = trimmed
                .strip_prefix("mod ")
                .unwrap_or("")
                .trim_end_matches(';')
                .trim();
            if !module.is_empty() {
                imports.push(module.to_string());
            }
        }
    }
    imports
}

fn extract_js_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        // `import ... from "..."` or `import ... from '...'`
        if trimmed.starts_with("import ") {
            if let Some(path) = extract_quoted_path(trimmed, "from ") {
                imports.push(path);
            }
        }
        // `require("...")` or `require('...')`
        if trimmed.contains("require(") {
            if let Some(start) = trimmed.find("require(") {
                let rest = &trimmed[start + 8..];
                if let Some(path) = extract_quoted_string(rest) {
                    imports.push(path);
                }
            }
        }
    }
    imports
}

fn extract_python_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        // `import foo.bar`
        if trimmed.starts_with("import ") {
            let module = trimmed
                .strip_prefix("import ")
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("");
            if !module.is_empty() {
                imports.push(module.split('.').next().unwrap_or(module).to_string());
            }
        }
        // `from foo.bar import baz`
        if trimmed.starts_with("from ") {
            let module = trimmed
                .strip_prefix("from ")
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("");
            if !module.is_empty() && module != "." && module != ".." {
                imports.push(module.split('.').next().unwrap_or(module).to_string());
            }
        }
    }
    imports
}

fn extract_php_imports(content: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        // `use App\Models\User;`
        if trimmed.starts_with("use ") && trimmed.contains('\\') {
            let path = trimmed
                .strip_prefix("use ")
                .unwrap_or("")
                .trim_end_matches(';')
                .trim();
            if !path.is_empty() {
                imports.push(path.replace('\\', "/"));
            }
        }
        // `require_once '...'` / `include '...'`
        if trimmed.starts_with("require") || trimmed.starts_with("include") {
            if let Some(path) = extract_quoted_string(trimmed) {
                imports.push(path);
            }
        }
    }
    imports
}

/// Extract a quoted string path after a keyword (e.g., `from "..."`)
fn extract_quoted_path(line: &str, keyword: &str) -> Option<String> {
    let idx = line.find(keyword)?;
    let rest = &line[idx + keyword.len()..];
    extract_quoted_string(rest)
}

/// Extract content of the first quoted string (single or double quotes).
fn extract_quoted_string(s: &str) -> Option<String> {
    let trimmed = s.trim();
    for quote in ['"', '\''] {
        if let Some(start) = trimmed.find(quote) {
            if let Some(end) = trimmed[start + 1..].find(quote) {
                return Some(trimmed[start + 1..start + 1 + end].to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Context collection
// ---------------------------------------------------------------------------

/// Collect context for AI repair.
///
/// Gathers error files, their imports, and relevant config files,
/// capping total size at `max_context_bytes`.
pub fn collect_context(
    fs: &sandbox::ScopedFs,
    issues: &[CheckIssue],
    max_context_bytes: Option<usize>,
) -> RepairContext {
    let max_bytes = max_context_bytes.unwrap_or(DEFAULT_MAX_CONTEXT_BYTES);
    let mut ctx = RepairContext {
        error_files: HashMap::new(),
        related_files: HashMap::new(),
        file_tree: String::new(),
        total_bytes: 0,
        max_bytes,
    };

    // 1. Collect the project file tree (top-level).
    ctx.file_tree = build_file_tree(fs);
    ctx.total_bytes += ctx.file_tree.len();

    // 2. Group issues by file.
    let mut by_file: HashMap<PathBuf, Vec<CheckIssue>> = HashMap::new();
    for issue in issues {
        by_file
            .entry(issue.file.clone())
            .or_default()
            .push(issue.clone());
    }

    // 3. Read error files.
    let mut seen_imports: HashSet<String> = HashSet::new();

    for (file_path, file_issues) in &by_file {
        if ctx.total_bytes >= max_bytes {
            break;
        }

        let file_name = file_path.display().to_string();
        let content = match fs.read_to_string(&file_name) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Extract imports before potentially truncating.
        let imports = extract_imports(&content, file_path);
        for imp in &imports {
            seen_imports.insert(imp.clone());
        }

        let lines: Vec<&str> = content.lines().collect();
        let file_ctx = if lines.len() > LARGE_FILE_THRESHOLD {
            // Large file: extract only the error region.
            extract_error_region(&content, file_issues)
        } else {
            FileContext {
                content: content.clone(),
                is_region: false,
                region_start_line: None,
                issues: file_issues.clone(),
            }
        };

        ctx.total_bytes += file_ctx.content.len();
        ctx.error_files.insert(file_path.clone(), file_ctx);
    }

    // 4. Read imported files (first N lines for type context).
    for import_path in &seen_imports {
        if ctx.total_bytes >= max_bytes {
            break;
        }

        // Try to resolve the import to a file.
        let candidates = resolve_import_path(import_path, &by_file.keys().collect::<Vec<_>>());
        for candidate in candidates {
            if ctx.error_files.contains_key(&candidate)
                || ctx.related_files.contains_key(&candidate)
            {
                continue;
            }

            let name = candidate.display().to_string();
            if let Ok(content) = fs.read_to_string(&name) {
                // Only include first 80 lines for imported files.
                let truncated: String = content
                    .lines()
                    .take(80)
                    .collect::<Vec<_>>()
                    .join("\n");
                let bytes = truncated.len();
                if ctx.total_bytes + bytes <= max_bytes {
                    ctx.total_bytes += bytes;
                    ctx.related_files.insert(candidate, truncated);
                }
            }
        }
    }

    // 5. Include config files.
    for config_file in CONFIG_FILES {
        if ctx.total_bytes >= max_bytes {
            break;
        }

        let path = PathBuf::from(config_file);
        if ctx.error_files.contains_key(&path) || ctx.related_files.contains_key(&path) {
            continue;
        }

        if let Ok(content) = fs.read_to_string(config_file) {
            // Config files are usually small, but cap at 4KB.
            let truncated = if content.len() > 4096 {
                format!("{}...(truncated)", &content[..4096])
            } else {
                content
            };
            let bytes = truncated.len();
            if ctx.total_bytes + bytes <= max_bytes {
                ctx.total_bytes += bytes;
                ctx.related_files.insert(path, truncated);
            }
        }
    }

    ctx
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a simple file tree string from the project root.
fn build_file_tree(fs: &sandbox::ScopedFs) -> String {
    // List top-level entries.
    match fs.list_dir(".") {
        Ok(entries) => {
            let mut tree = String::from("Project files:\n");
            for entry in entries {
                let suffix = if entry.is_dir { "/" } else { "" };
                tree.push_str(&format!("  {}{}\n", entry.name, suffix));
            }
            tree
        }
        Err(_) => String::from("(file tree unavailable)\n"),
    }
}

/// Extract the error region from a large file.
///
/// Returns the lines around the earliest and latest error locations,
/// with `ERROR_REGION_PADDING` lines of surrounding context.
fn extract_error_region(content: &str, issues: &[CheckIssue]) -> FileContext {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    // Find the range of error lines.
    let error_lines: Vec<usize> = issues
        .iter()
        .filter_map(|i| i.line.map(|l| l as usize))
        .collect();

    if error_lines.is_empty() {
        // No line information; return first 100 lines.
        let region: String = lines.iter().take(100).copied().collect::<Vec<_>>().join("\n");
        return FileContext {
            content: region,
            is_region: true,
            region_start_line: Some(1),
            issues: issues.to_vec(),
        };
    }

    let min_line = error_lines.iter().min().copied().unwrap_or(1);
    let max_line = error_lines.iter().max().copied().unwrap_or(min_line);

    let start = min_line.saturating_sub(ERROR_REGION_PADDING).max(1);
    let end = (max_line + ERROR_REGION_PADDING).min(total_lines);

    let region: String = lines[start - 1..end].join("\n");

    FileContext {
        content: format!(
            "// ... (lines 1-{} omitted) ...\n{}\n// ... (lines {}-{} omitted) ...",
            start - 1,
            region,
            end + 1,
            total_lines
        ),
        is_region: true,
        region_start_line: Some(start as u32),
        issues: issues.to_vec(),
    }
}

/// Attempt to resolve an import string to file path candidates.
///
/// This is a best-effort heuristic—not full module resolution.
fn resolve_import_path(import: &str, known_files: &[&PathBuf]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // For relative JS/TS imports.
    if import.starts_with('.') {
        for ext in &["", ".ts", ".tsx", ".js", ".jsx", "/index.ts", "/index.js"] {
            candidates.push(PathBuf::from(format!("{}{}", import, ext)));
        }
    }

    // For Rust module names, look for src/<module>.rs or src/<module>/mod.rs
    if !import.contains('/') && !import.contains('.') {
        candidates.push(PathBuf::from(format!("src/{}.rs", import)));
        candidates.push(PathBuf::from(format!("src/{}/mod.rs", import)));
    }

    // Check against known files.
    for known in known_files {
        let name = known.display().to_string();
        if name.contains(import) {
            candidates.push((*known).clone());
        }
    }

    candidates
}

// ---------------------------------------------------------------------------
// Formatting helpers for prompts
// ---------------------------------------------------------------------------

/// Format the collected context into a string suitable for an AI prompt.
pub fn format_context_for_prompt(ctx: &RepairContext) -> String {
    let mut output = String::new();

    // File tree.
    output.push_str(&ctx.file_tree);
    output.push('\n');

    // Error files.
    for (path, file_ctx) in &ctx.error_files {
        output.push_str(&format!("FILE: {}", path.display()));
        if file_ctx.is_region {
            if let Some(start) = file_ctx.region_start_line {
                output.push_str(&format!(" (region starting at line {})", start));
            }
        }
        output.push('\n');
        output.push_str(&file_ctx.content);
        output.push_str("\n\n");
    }

    // Related files.
    for (path, content) in &ctx.related_files {
        output.push_str(&format!("RELATED FILE: {}\n", path.display()));
        output.push_str(content);
        output.push_str("\n\n");
    }

    output
}
