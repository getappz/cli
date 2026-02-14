//! Response extraction helpers for LLM output.
//!
//! Parse code blocks, diffs, JSON, and markdown from raw LLM responses.

/// Extract a fenced code block from an LLM response.
///
/// Looks for the first ``` ... ``` block and returns its content.
/// If no code fence is found, returns the whole response as-is.
pub fn extract_code_block(response: &str) -> String {
    let lines: Vec<&str> = response.lines().collect();
    let mut in_code_block = false;
    let mut code_lines = Vec::new();

    for line in &lines {
        if line.starts_with("```") {
            if in_code_block {
                break;
            } else {
                in_code_block = true;
                continue;
            }
        }
        if in_code_block {
            code_lines.push(*line);
        }
    }

    if !code_lines.is_empty() {
        return code_lines.join("\n");
    }

    response.to_string()
}

/// Extract a unified diff from an LLM response.
///
/// Looks for content starting with `---` or `diff --git`, or inside a
/// code block tagged as `diff`.
pub fn extract_diff_block(response: &str) -> Option<String> {
    // First try: look for a diff-fenced code block.
    let lines: Vec<&str> = response.lines().collect();
    let mut in_diff_block = false;
    let mut diff_lines = Vec::new();

    for line in &lines {
        if line.starts_with("```diff") || line.starts_with("```patch") {
            in_diff_block = true;
            continue;
        }
        if in_diff_block {
            if line.starts_with("```") {
                break;
            }
            diff_lines.push(*line);
        }
    }

    if !diff_lines.is_empty() {
        return Some(diff_lines.join("\n"));
    }

    // Second try: look for any fenced code block containing diff markers.
    let code = extract_code_block(response);
    if code.contains("---") && code.contains("+++") {
        return Some(code);
    }

    // Third try: look for raw diff content outside of code blocks.
    let mut raw_diff = Vec::new();
    let mut found_header = false;
    for line in &lines {
        if line.starts_with("---") || line.starts_with("diff --git") {
            found_header = true;
        }
        if found_header {
            raw_diff.push(*line);
        }
    }

    if !raw_diff.is_empty() {
        return Some(raw_diff.join("\n"));
    }

    None
}

/// Extract JSON from an LLM response.
///
/// Looks for a JSON code block, or falls back to finding the first
/// `{` ... `}` pair in the response.
pub fn extract_json_block(response: &str) -> Option<String> {
    // Try a json-fenced code block first.
    let lines: Vec<&str> = response.lines().collect();
    let mut in_json_block = false;
    let mut json_lines = Vec::new();

    for line in &lines {
        if line.starts_with("```json") {
            in_json_block = true;
            continue;
        }
        if in_json_block {
            if line.starts_with("```") {
                break;
            }
            json_lines.push(*line);
        }
    }

    if !json_lines.is_empty() {
        return Some(json_lines.join("\n"));
    }

    // Try any code block that looks like JSON.
    let code = extract_code_block(response);
    if code.trim_start().starts_with('{') && code.trim_end().ends_with('}') {
        return Some(code);
    }

    // Try raw JSON in the response.
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            if end > start {
                return Some(response[start..=end].to_string());
            }
        }
    }

    None
}

/// Extract JSON from an LLM response and parse into a typed struct.
///
/// Returns `None` if no JSON block is found or parsing fails.
pub fn extract_and_parse_json<T: serde::de::DeserializeOwned>(response: &str) -> Option<T> {
    let json_str = extract_json_block(response)?;
    serde_json::from_str(&json_str).ok()
}
