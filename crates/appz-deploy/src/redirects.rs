//! Redirect rules, staged versions, and local validation of import files.

use serde::{Deserialize, Serialize};

pub const REDIRECT_STATUSES: [u16; 4] = [301, 302, 307, 308];
/// Status used when a rule or import row does not give one.
pub const DEFAULT_REDIRECT_STATUS: u16 = 307;
pub const MAX_IMPORT_RULES: usize = 1000;
const MAX_FIELD_LEN: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedirectInput {
    pub source: String,
    pub destination: String,
    pub status: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedirectRule {
    pub id: String,
    pub source: String,
    pub destination: String,
    pub status: u16,
    pub position: u32,
}

/// One redirect version's rules. `version` is null when nothing was ever staged or live.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedirectList {
    pub version: Option<u64>,
    pub rules: Vec<RedirectRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectVersion {
    pub version: u64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectVersions {
    pub live_version: Option<u64>,
    pub staged_version: Option<u64>,
    pub versions: Vec<RedirectVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedirectVersionRef {
    pub version: u64,
}

/// A problem found in an import file; `line` is 1-based (CSV file line, or
/// the position of the entry in a JSON array).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for RowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.line, self.message)
    }
}

/// Validate one rule. Returns the problem, if any.
pub fn validate_redirect(source: &str, destination: &str, status: u16) -> Option<String> {
    if !source.starts_with('/') {
        return Some(format!("source must start with '/' (got '{source}')"));
    }
    if source.len() > MAX_FIELD_LEN {
        return Some(format!("source is longer than {MAX_FIELD_LEN} characters"));
    }
    if destination.is_empty() {
        return Some("destination is empty".into());
    }
    if destination.len() > MAX_FIELD_LEN {
        return Some(format!(
            "destination is longer than {MAX_FIELD_LEN} characters"
        ));
    }
    if !REDIRECT_STATUSES.contains(&status) {
        return Some(format!(
            "status must be 301, 302, 307 or 308 (got {status})"
        ));
    }
    None
}

/// Parse and validate an import file. `csv` selects the CSV format; anything
/// else is a JSON array of `{source, destination, status}` objects.
/// Reports every row problem, not just the first.
pub fn parse_redirect_import(text: &str, csv: bool) -> Result<Vec<RedirectInput>, Vec<RowError>> {
    let rows = if csv {
        parse_csv_rows(text)?
    } else {
        parse_json_rows(text)?
    };
    let mut errors = Vec::new();
    let mut out: Vec<RedirectInput> = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (line, source, destination, status) in rows {
        let status = match status {
            Ok(s) => s,
            Err(message) => {
                errors.push(RowError { line, message });
                continue;
            }
        };
        if let Some(message) = validate_redirect(&source, &destination, status) {
            errors.push(RowError { line, message });
        } else if let Some(first) = seen.get(&source) {
            errors.push(RowError {
                line,
                message: format!("duplicate source '{source}' (first at {first})"),
            });
        } else {
            seen.insert(source.clone(), line);
            out.push(RedirectInput {
                source,
                destination,
                status,
            });
        }
    }
    if errors.is_empty() && out.is_empty() {
        errors.push(RowError {
            line: 1,
            message: "no redirects found".into(),
        });
    }
    if errors.is_empty() && out.len() > MAX_IMPORT_RULES {
        errors.push(RowError {
            line: MAX_IMPORT_RULES + 1,
            message: format!(
                "too many redirects: {} (maximum {MAX_IMPORT_RULES})",
                out.len()
            ),
        });
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

type Row = (usize, String, String, Result<u16, String>);

fn parse_status(raw: &str) -> Result<u16, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(DEFAULT_REDIRECT_STATUS);
    }
    raw.parse::<u16>()
        .map_err(|_| format!("status must be 301, 302, 307 or 308 (got '{raw}')"))
}

fn parse_json_rows(text: &str) -> Result<Vec<Row>, Vec<RowError>> {
    let fail = |message: String| vec![RowError { line: 1, message }];
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| fail(format!("invalid JSON: {e}")))?;
    let items = value
        .as_array()
        .ok_or_else(|| fail("expected a JSON array of redirects".into()))?;
    Ok(items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let s = |k: &str| {
                item.get(k)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let status = match item.get("status") {
                None | Some(serde_json::Value::Null) => Ok(DEFAULT_REDIRECT_STATUS),
                Some(v) => v
                    .as_u64()
                    .and_then(|n| u16::try_from(n).ok())
                    .ok_or_else(|| format!("status must be 301, 302, 307 or 308 (got {v})")),
            };
            (i + 1, s("source"), s("destination"), status)
        })
        .collect())
}

/// Split CSV text into records, each with the file line it starts on.
/// Handles quoted fields, doubled quotes, embedded newlines, CRLF and a BOM.
fn csv_records(text: &str) -> Result<Vec<(usize, Vec<String>)>, Vec<RowError>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut records = Vec::new();
    let (mut fields, mut field) = (Vec::new(), String::new());
    let (mut in_quotes, mut quoted) = (false, false);
    let (mut line, mut start) = (1usize, 1usize);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => in_quotes = false,
                '\n' => {
                    line += 1;
                    field.push(c);
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() && !quoted => (in_quotes, quoted) = (true, true),
            ',' => {
                fields.push(std::mem::take(&mut field));
                quoted = false;
            }
            '\r' => {}
            '\n' => {
                fields.push(std::mem::take(&mut field));
                quoted = false;
                if fields.iter().any(|f| !f.trim().is_empty()) {
                    records.push((start, std::mem::take(&mut fields)));
                }
                fields.clear();
                line += 1;
                start = line;
            }
            _ => field.push(c),
        }
    }
    if in_quotes {
        return Err(vec![RowError {
            line: start,
            message: "unterminated quoted field".into(),
        }]);
    }
    fields.push(field);
    if fields.iter().any(|f| !f.trim().is_empty()) {
        records.push((start, fields));
    }
    Ok(records)
}

fn parse_csv_rows(text: &str) -> Result<Vec<Row>, Vec<RowError>> {
    let mut records = csv_records(text)?.into_iter();
    let Some((hline, header)) = records.next() else {
        return Err(vec![RowError {
            line: 1,
            message: "empty CSV; expected a header row: source,destination,status".into(),
        }]);
    };
    let col = |name: &str| {
        header
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(name))
    };
    let (Some(si), Some(di)) = (col("source"), col("destination")) else {
        return Err(vec![RowError {
            line: hline,
            message:
                "header must contain 'source' and 'destination' columns (and optionally 'status')"
                    .into(),
        }]);
    };
    let sti = col("status");
    Ok(records
        .map(|(line, f)| {
            let get = |i: usize| f.get(i).map_or("", |s| s.trim()).to_string();
            let status = sti.map_or(Ok(DEFAULT_REDIRECT_STATUS), |i| parse_status(&get(i)));
            (line, get(si), get(di), status)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_reports_line_numbers_and_defaults_status() {
        let csv = "source,destination,status\n/a,/b,301\n\n/c,/d\nbad,/e,302\n/f,/g,999\n";
        let errs = parse_redirect_import(csv, true).unwrap_err();
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line, 5);
        assert!(errs[0].message.contains("start with '/'"));
        assert_eq!(errs[1].line, 6);
        let ok = parse_redirect_import("Source,Destination\n/a,\"/b,c\"\n", true).unwrap();
        assert_eq!(ok[0].destination, "/b,c");
        assert_eq!(ok[0].status, 307);
    }

    #[test]
    fn csv_handles_quotes_newlines_and_bom() {
        let csv = "\u{feff}source,destination,status\r\n\"/a\",\"/x\ny\",308\r\n/c,/d,302\r\n";
        let rows = parse_redirect_import(csv, true).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].destination, "/x\ny");
        assert_eq!(rows[1].status, 302);
        let errs = parse_redirect_import("source,destination\n/a,\"x\n", true).unwrap_err();
        assert!(errs[0].message.contains("unterminated"));
        let errs = parse_redirect_import("a,b\n/a,/b\n", true).unwrap_err();
        assert!(errs[0].message.contains("header"));
    }

    #[test]
    fn json_validates_each_entry_and_duplicates() {
        let json = r#"[{"source":"/a","destination":"/b","status":301},
            {"source":"/a","destination":"/c"},{"source":"x","destination":"/c"},
            {"source":"/d","destination":"","status":"301"}]"#;
        let errs = parse_redirect_import(json, false).unwrap_err();
        let lines: Vec<usize> = errs.iter().map(|e| e.line).collect();
        assert_eq!(lines, vec![2, 3, 4]);
        assert!(errs[0].message.contains("duplicate"));
        assert!(parse_redirect_import("{}", false).is_err());
        assert!(parse_redirect_import("[]", false).is_err());
        assert!(parse_redirect_import("nope", false).is_err());
    }

    #[test]
    fn rejects_more_than_the_maximum() {
        let rows: Vec<_> = (0..=MAX_IMPORT_RULES)
            .map(|i| serde_json::json!({"source": format!("/{i}"), "destination": "/x"}))
            .collect();
        let text = serde_json::to_string(&rows).unwrap();
        let errs = parse_redirect_import(&text, false).unwrap_err();
        assert!(errs[0].message.contains("too many"));
    }
}
