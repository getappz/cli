use biome_js_parser::{parse, JsParserOptions};
use biome_js_syntax::{
    JsFileSource, JsxElement, JsxSelfClosingElement, AnyJsxAttribute,
    JsImport, JsLanguage
};
use biome_rowan::{AstNode, SyntaxNode, WalkEvent, Direction};
use miette::Result;
use regex::Regex;

/// Edit to apply to source code
#[derive(Debug, Clone)]
struct Edit {
    start: usize,
    end: usize,
    replacement: String,
}

/// Transform React code using Biome's AST with edit-based approach
pub fn transform_with_ast(content: &str) -> Result<String> {
    let parse_result = parse(content, JsFileSource::tsx(), JsParserOptions::default());

    let root = parse_result.tree();
    let syntax = root.syntax();

    let mut transformer = AstTransformer::new();
    transformer.collect_edits(syntax);

    let result = transformer.apply_edits(content);

    Ok(transform_with_regex_fallback(&result))
}

struct AstTransformer {
    edits: Vec<Edit>,
}

impl AstTransformer {
    fn new() -> Self {
        Self {
            edits: Vec::new(),
        }
    }

    fn collect_edits(&mut self, node: &SyntaxNode<JsLanguage>) {
        for event in node.preorder_with_tokens(Direction::Next) {
            match event {
                WalkEvent::Enter(entry) => {
                    if let Some(node) = entry.as_node() {
                        if let Some(jsx_element) = JsxElement::cast(node.clone()) {
                            self.collect_jsx_element_edits(&jsx_element);
                        } else if let Some(jsx_self_closing) = JsxSelfClosingElement::cast(node.clone()) {
                            self.collect_jsx_self_closing_edits(&jsx_self_closing);
                        }

                        if let Some(import_decl) = JsImport::cast(node.clone()) {
                            self.collect_import_edits(&import_decl);
                        }
                    }
                }
                WalkEvent::Leave(_) => {}
            }
        }
    }

    fn collect_jsx_element_edits(&mut self, element: &JsxElement) {
        if let Ok(opening) = element.opening_element() {
            if let Ok(name) = opening.name() {
                if let Ok(name_token) = name.name_value_token() {
                    if name_token.text_trimmed() == "Link" {
                        let attributes = opening.attributes();
                        for attr in attributes {
                            if let AnyJsxAttribute::JsxAttribute(jsx_attr) = attr {
                                if let Ok(attr_name) = jsx_attr.name() {
                                    if let Ok(attr_name_token) = attr_name.name_token() {
                                        if attr_name_token.text_trimmed() == "to" {
                                            let range = attr_name_token.text_trimmed_range();
                                            self.edits.push(Edit {
                                                start: range.start().into(),
                                                end: range.end().into(),
                                                replacement: "href".to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                        }

                        let range = name_token.text_trimmed_range();
                        self.edits.push(Edit {
                            start: range.start().into(),
                            end: range.end().into(),
                            replacement: "a".to_string(),
                        });
                    }
                }
            }
        }

        if let Ok(closing) = element.closing_element() {
            if let Ok(name) = closing.name() {
                if let Ok(name_token) = name.name_value_token() {
                    if name_token.text_trimmed() == "Link" {
                        let range = name_token.text_trimmed_range();
                        self.edits.push(Edit {
                            start: range.start().into(),
                            end: range.end().into(),
                            replacement: "a".to_string(),
                        });
                    }
                }
            }
        }
    }

    fn collect_jsx_self_closing_edits(&mut self, element: &JsxSelfClosingElement) {
        if let Ok(name) = element.name() {
            if let Ok(name_token) = name.name_value_token() {
                if name_token.text_trimmed() == "Link" {
                    let attributes = element.attributes();
                    for attr in attributes {
                        if let AnyJsxAttribute::JsxAttribute(jsx_attr) = attr {
                            if let Ok(attr_name) = jsx_attr.name() {
                                if let Ok(attr_name_token) = attr_name.name_token() {
                                    if attr_name_token.text_trimmed() == "to" {
                                        let range = attr_name_token.text_trimmed_range();
                                        self.edits.push(Edit {
                                            start: range.start().into(),
                                            end: range.end().into(),
                                            replacement: "href".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }

                    let range = name_token.text_trimmed_range();
                    self.edits.push(Edit {
                        start: range.start().into(),
                        end: range.end().into(),
                        replacement: "a".to_string(),
                    });
                }
            }
        }
    }

    fn collect_import_edits(&mut self, import: &JsImport) {
        if let Ok(import_clause) = import.import_clause() {
            if let Ok(source) = import_clause.source() {
                if let Ok(source_text) = source.inner_string_text() {
                    let source_str = source_text.text();

                    if source_str == "react-router-dom" {
                        let range = import.syntax().text_trimmed_range();
                        self.edits.push(Edit {
                            start: range.start().into(),
                            end: range.end().into(),
                            replacement: String::new(),
                        });
                    }
                }
            }
        }
    }

    fn apply_edits(&self, content: &str) -> String {
        if self.edits.is_empty() {
            return content.to_string();
        }

        let mut sorted_edits = self.edits.clone();
        sorted_edits.sort_by(|a, b| b.start.cmp(&a.start));

        let mut result = content.to_string();

        for edit in sorted_edits {
            if edit.start <= result.len() && edit.end <= result.len() && edit.start <= edit.end {
                result.replace_range(edit.start..edit.end, &edit.replacement);
            }
        }

        result
    }
}

fn transform_with_regex_fallback(content: &str) -> String {
    let mut fixed = content.to_string();

    let use_effect_location_pattern = Regex::new(r#"(?s)useEffect\s*\(\s*\(\)\s*=>\s*\{[^}]*location[^}]*\},\s*\[location\.pathname\]\s*\)\s*;"#).unwrap();
    fixed = use_effect_location_pattern.replace_all(&fixed, "").to_string();

    let location_pattern = Regex::new(r#"const\s+location\s*=\s*useLocation\(\)\s*;"#).unwrap();
    fixed = location_pattern.replace_all(&fixed, "").to_string();

    // Use ast-grep for precise location.pathname replacement (native only)
    #[cfg(feature = "native")]
    {
        fixed = replace_location_pathname_ast_grep(&fixed);
    }
    #[cfg(not(feature = "native"))]
    {
        fixed = replace_location_pathname_regex(&fixed);
    }

    fixed
}

#[cfg(feature = "native")]
fn replace_location_pathname_ast_grep(content: &str) -> String {
    use ast_grep_core::{AstGrep, Pattern};
    use ast_grep_language::SupportLang;

    let grep = AstGrep::new(content, SupportLang::Tsx);
    let root = grep.root();

    let location_pathname_pattern = Pattern::new("location.pathname", SupportLang::Tsx);
    let matches: Vec<_> = root.find_all(&location_pathname_pattern).collect();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();

    for node_match in matches {
        let range = node_match.range();
        let matched_text = &content[range.start..range.end.min(content.len())];
        if matched_text == "location.pathname" {
            edits.push((
                range.start,
                range.end,
                "(typeof window !== 'undefined' ? window.location.pathname : '/')".to_string(),
            ));
        }
    }

    edits.sort_by(|a, b| b.0.cmp(&a.0));
    let mut result = content.to_string();
    for (start, end, replacement) in edits {
        if start <= result.len() && end <= result.len() && start <= end {
            result.replace_range(start..end, &replacement);
        }
    }

    result
}

#[cfg(not(feature = "native"))]
fn replace_location_pathname_regex(content: &str) -> String {
    let re = Regex::new(r"\blocation\.pathname\b").unwrap();
    re.replace_all(
        content,
        "(typeof window !== 'undefined' ? window.location.pathname : '/')",
    )
    .to_string()
}
