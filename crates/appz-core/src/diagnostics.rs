use serde::Deserialize;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Diagnostic {
    pub severity: String,
    pub message: String,
    pub code: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Deserialize)]
struct CargoMessage {
    reason: String,
    message: Option<CargoDiagnosticBody>,
}

#[derive(Deserialize)]
struct CargoDiagnosticBody {
    message: String,
    level: String,
    code: Option<CargoCode>,
    spans: Vec<CargoSpan>,
}

#[derive(Deserialize)]
struct CargoCode {
    code: String,
}

#[derive(Deserialize)]
struct CargoSpan {
    file_name: String,
    is_primary: bool,
    line_start: u32,
    column_start: u32,
}

#[must_use]
pub fn parse_cargo_diagnostics(ndjson: &str) -> Vec<Diagnostic> {
    ndjson
        .lines()
        .filter_map(|line| serde_json::from_str::<CargoMessage>(line).ok())
        .filter(|msg| msg.reason == "compiler-message")
        .filter_map(|msg| msg.message)
        .map(|body| {
            let primary = body.spans.iter().find(|s| s.is_primary);
            Diagnostic {
                severity: body.level,
                message: body.message,
                code: body.code.map(|c| c.code),
                file: primary.map(|s| s.file_name.clone()),
                line: primary.map(|s| s.line_start),
                column: primary.map(|s| s.column_start),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ERROR_LINE: &str = r#"{"reason":"compiler-message","package_id":"path+file:///tmp/fixture#fixture_crate@0.1.0","manifest_path":"/tmp/fixture/Cargo.toml","target":{"kind":["bin"],"crate_types":["bin"],"name":"fixture_crate","src_path":"/tmp/fixture/src/main.rs","edition":"2024","doc":true,"doctest":false,"test":true},"message":{"rendered":"error[E0308]: mismatched types\n --> src/main.rs:2:18\n  |\n2 |     let x: i32 = \"not a number\";\n  |            ---   ^^^^^^^^^^^^^^ expected `i32`, found `&str`\n  |            |\n  |            expected due to this\n\n","$message_type":"diagnostic","children":[],"level":"error","message":"mismatched types","spans":[{"byte_end":43,"byte_start":29,"column_end":32,"column_start":18,"expansion":null,"file_name":"src/main.rs","is_primary":true,"label":"expected `i32`, found `&str`","line_end":2,"line_start":2,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"highlight_end":32,"highlight_start":18,"text":"    let x: i32 = \"not a number\";"}]},{"byte_end":26,"byte_start":23,"column_end":15,"column_start":12,"expansion":null,"file_name":"src/main.rs","is_primary":false,"label":"expected due to this","line_end":2,"line_start":2,"suggested_replacement":null,"suggestion_applicability":null,"text":[{"highlight_end":15,"highlight_start":12,"text":"    let x: i32 = \"not a number\";"}]}],"code":{"code":"E0308","explanation":"Expected type did not match the received type.\n"}}}"#;

    const FAILURE_NOTE_LINE: &str = r#"{"reason":"compiler-message","package_id":"path+file:///tmp/fixture#fixture_crate@0.1.0","manifest_path":"/tmp/fixture/Cargo.toml","target":{"kind":["bin"],"crate_types":["bin"],"name":"fixture_crate","src_path":"/tmp/fixture/src/main.rs","edition":"2024","doc":true,"doctest":false,"test":true},"message":{"rendered":"For more information about this error, try `rustc --explain E0308`.\n","$message_type":"diagnostic","children":[],"level":"failure-note","message":"For more information about this error, try `rustc --explain E0308`.","spans":[],"code":null}}"#;

    const ARTIFACT_LINE: &str = r#"{"reason":"compiler-artifact","package_id":"path+file:///tmp/fixture#fixture_crate@0.1.0","manifest_path":"/tmp/fixture/Cargo.toml","target":{"kind":["bin"],"crate_types":["bin"],"name":"fixture_crate","src_path":"/tmp/fixture/src/main.rs","edition":"2024","doc":true,"doctest":false,"test":true},"profile":{"opt_level":"0","debuginfo":2,"debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["/tmp/fixture/target/debug/fixture_crate"],"executable":"/tmp/fixture/target/debug/fixture_crate","fresh":false}"#;

    const BUILD_FINISHED_LINE: &str = r#"{"reason":"build-finished","success":true}"#;

    #[test]
    fn test_parses_compiler_message_with_primary_span() {
        let diags = parse_cargo_diagnostics(ERROR_LINE);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.severity, "error");
        assert_eq!(d.message, "mismatched types");
        assert_eq!(d.code.as_deref(), Some("E0308"));
        assert_eq!(d.file.as_deref(), Some("src/main.rs"));
        assert_eq!(d.line, Some(2));
        assert_eq!(d.column, Some(18));
    }

    #[test]
    fn test_compiler_message_without_primary_span_has_no_location() {
        let diags = parse_cargo_diagnostics(FAILURE_NOTE_LINE);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.severity, "failure-note");
        assert_eq!(
            d.message,
            "For more information about this error, try `rustc --explain E0308`."
        );
        assert_eq!(d.code, None);
        assert_eq!(d.file, None);
        assert_eq!(d.line, None);
        assert_eq!(d.column, None);
    }

    #[test]
    fn test_non_compiler_message_reasons_are_ignored() {
        assert_eq!(parse_cargo_diagnostics(ARTIFACT_LINE).len(), 0);
        assert_eq!(parse_cargo_diagnostics(BUILD_FINISHED_LINE).len(), 0);
    }

    #[test]
    fn test_invalid_json_line_is_skipped_not_fatal() {
        let mixed = format!("not json at all\n{ERROR_LINE}\n{{also not json");
        let diags = parse_cargo_diagnostics(&mixed);
        assert_eq!(diags.len(), 1, "the one valid line must still parse");
    }

    #[test]
    fn test_empty_input_returns_empty_vec() {
        assert_eq!(parse_cargo_diagnostics("").len(), 0);
        assert_eq!(parse_cargo_diagnostics("\n\n").len(), 0);
    }
}
