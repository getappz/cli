//! Parse ripgrep --json output lines.

use anyhow::Result;

use crate::schema::RawMatch;

/// Parse one line of ripgrep JSON output. Returns None for non-match events.
pub fn parse_rg_json_line(line: &str) -> Result<Option<RawMatch>> {
    let v: serde_json::Value = serde_json::from_str(line)?;
    if v["type"] != "match" {
        return Ok(None);
    }
    let data = &v["data"];
    let line_number = data["line_number"].as_u64().unwrap_or(0);
    let snippet = data["lines"]["text"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    let column = data["submatches"]
        .get(0)
        .and_then(|m| m["start"].as_u64());
    Ok(Some(RawMatch {
        line: line_number,
        column,
        snippet,
    }))
}
