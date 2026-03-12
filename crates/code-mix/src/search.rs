//! Search packed code — safe ripgrep over cached pack with line mapping.

use std::path::Path;

use code_grep::{execute, SearchRequest, SearchResult};
use regex::Regex;

use crate::repomix::RepomixError;

/// Section: (content_start_line 1-based, file_path)
struct Section {
    content_start_line: u64,
    file_path: String,
}

/// Build mapping from pack line number to (file_path, line_in_file).
/// Pack format: ## File: path on line N, content on N+1.. until next ## File.
fn build_line_mapping(content: &str) -> Vec<Section> {
    let re = Regex::new(r"(?m)^## File: (.+)$").unwrap();
    let mut sections = Vec::new();

    for cap in re.captures_iter(content) {
        let path = cap[1].trim().to_string();
        let match_start = cap.get(0).unwrap().start();
        let header_line = 1 + content[..match_start].matches('\n').count() as u64;
        let content_start = header_line + 1;

        sections.push(Section {
            content_start_line: content_start,
            file_path: path,
        });
    }

    sections
}

/// Find section containing line L. Sections are ordered by content_start_line.
fn find_section(sections: &[Section], line: u64) -> Option<&Section> {
    for s in sections.iter().rev() {
        if line >= s.content_start_line {
            return Some(s);
        }
    }
    None
}

/// Check if file path matches glob. Simple prefix/suffix for common cases.
fn matches_glob(file_path: &str, glob: &str) -> bool {
    if glob.is_empty() {
        return true;
    }
    if glob.contains('*') {
        let pattern = glob.replace('.', "\\.");
        let pattern = pattern.replace('*', ".*");
        if let Ok(re) = Regex::new(&format!("^{}$", pattern)) {
            return re.is_match(file_path);
        }
    }
    file_path.ends_with(glob) || file_path.contains(glob)
}

/// Search packed content at path. Maps line numbers to source files via ## File: headers.
pub fn search_packed(
    req: &SearchRequest,
    pack_path: &Path,
) -> Result<Vec<SearchResult>, RepomixError> {
    let content = std::fs::read_to_string(pack_path)
        .map_err(|e| RepomixError(format!("Failed to read pack: {}", e)))?;

    let temp = tempfile::NamedTempFile::new()
        .map_err(|e| RepomixError(format!("Failed to create temp file: {}", e)))?;
    let temp_path = temp.path();
    std::fs::write(temp_path, &content)
        .map_err(|e| RepomixError(format!("Failed to write temp file: {}", e)))?;

    let raw_matches = execute(req, temp_path).map_err(|e| RepomixError(e.to_string()))?;

    let sections = build_line_mapping(&content);
    let mut results = Vec::with_capacity(raw_matches.len());

    for m in raw_matches {
        let Some(section) = find_section(&sections, m.line) else {
            continue;
        };
        let line_in_file = m.line.saturating_sub(section.content_start_line) + 1;

        if let Some(ref glob) = req.file_glob {
            if !matches_glob(&section.file_path, glob) {
                continue;
            }
        }

        results.push(SearchResult {
            file: section.file_path.clone(),
            line: line_in_file,
            column: m.column,
            snippet: m.snippet,
        });
    }

    Ok(results)
}
