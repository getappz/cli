
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use crate::models::*;
use crate::rules::apply_rules;
use crate::scoring::compute_score;

pub fn analyze_html(html: &str, url: &str) -> SeoReport {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap();

    let mut report = SeoReport {
        url: url.to_string(),
        title: None,
        meta_description: None,
        canonical: None,
        word_count: 0,
        headings: Vec::with_capacity(8),
        images: Vec::with_capacity(16),
        links: Vec::with_capacity(32),
        issues: Vec::new(),
        score: SeoScore::default(),
    };

    walk_iterative(&dom.document, &mut report);

    // DOM no longer needed beyond this point
    drop(dom);

    apply_rules(&mut report);
    report.score = compute_score(&report.issues);
    report
}

fn walk_iterative(root: &Handle, report: &mut SeoReport) {
    // Stack stores (node, skip_text_counting)
    let mut stack = Vec::with_capacity(256);
    stack.push((root.clone(), false));

    while let Some((node, skip_text)) = stack.pop() {
        match &node.data {
            NodeData::Element { name, attrs, .. } => {
                let tag = name.local.as_ref();
                
                // Fast guard: skip word counting in script/style/noscript
                let skip_text_counting = matches!(tag, "script" | "style" | "noscript") || skip_text;

                match tag {
                    "title" => {
                        if report.title.is_none() {
                            report.title = extract_text_fast(&node);
                        }
                    }
                    "meta" => {
                        let mut is_desc = false;
                        let mut content = None;

                        for attr in attrs.borrow().iter() {
                            match attr.name.local.as_ref() {
                                "name" if attr.value.eq_ignore_ascii_case("description") => {
                                    is_desc = true;
                                }
                                "content" => {
                                    content = Some(attr.value.to_string());
                                }
                                _ => {}
                            }
                        }

                        if is_desc {
                            report.meta_description = content;
                        }
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        // Fast heading level extraction using byte arithmetic
                        let level = tag.as_bytes()[1] - b'0';
                        if let Some(text) = extract_text_fast(&node) {
                            report.headings.push(Heading {
                                level,
                                text,
                            });
                        }
                    }
                    "img" => {
                        let mut src = None;
                        let mut alt = None;
                        let mut loading = None;

                        for attr in attrs.borrow().iter() {
                            match attr.name.local.as_ref() {
                                "src" => src = Some(attr.value.to_string()),
                                "alt" => alt = Some(attr.value.to_string()),
                                "loading" => loading = Some(attr.value.to_string()),
                                _ => {}
                            }
                        }

                        if let Some(src) = src {
                            report.images.push(ImageInfo { src, alt, loading });
                        }
                    }
                    "a" => {
                        let mut href = None;
                        for attr in attrs.borrow().iter() {
                            if attr.name.local.as_ref() == "href" {
                                href = Some(attr.value.to_string());
                                break; // Early exit once href is found
                            }
                        }

                        if let Some(href) = href {
                            let text = extract_text_fast(&node).unwrap_or_default();
                            report.links.push(LinkInfo { href, text });
                        }
                    }
                    _ => {}
                }

                // Push children with skip_text_counting flag
                for child in node.children.borrow().iter().rev() {
                    stack.push((child.clone(), skip_text_counting));
                }
            }
            NodeData::Text { contents } => {
                if !skip_text {
                    let text = contents.borrow();
                    report.word_count += fast_word_count(&text);
                }
            }
            _ => {
                // For non-element, non-text nodes, still push children with same skip flag
                for child in node.children.borrow().iter().rev() {
                    stack.push((child.clone(), skip_text));
                }
            }
        }
    }
}

#[inline]
fn extract_text_fast(node: &Handle) -> Option<String> {
    let mut buf = String::new();

    for child in node.children.borrow().iter() {
        if let NodeData::Text { contents } = &child.data {
            buf.push_str(&contents.borrow());
        }
    }

    let text = buf.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Zero-allocation, zero-copy word counter optimized for SEO analysis.
/// Uses byte-level FSM for maximum performance at scale (10k-100k pages).
/// SEO does not require full Unicode word boundaries - ASCII whitespace
/// and punctuation boundaries are sufficient for word count heuristics.
#[inline(always)]
fn fast_word_count(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut count = 0;
    let mut in_word = false;

    for &b in bytes {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => {
                if !in_word {
                    count += 1;
                    in_word = true;
                }
            }
            _ => {
                in_word = false;
            }
        }
    }

    count
}
