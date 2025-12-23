
use lol_html::{element, text, HtmlRewriter, Settings};
use std::cell::RefCell;
use std::rc::Rc;
use crate::models::*;
use crate::rules::apply_rules;
use crate::rules::rule_trait::RuleContext;
use crate::rules::tier_a_streaming::get_tier_a_rules;
use crate::rules::tier_b_document::get_tier_b_rules;
use crate::rules::tier_c_live::get_tier_c_rules;
use crate::scoring::compute_score;
use rayon::prelude::*;

// We aren't transforming HTML, just parsing, so we dump the output.
#[derive(Default)]
struct EmptySink;
impl lol_html::OutputSink for EmptySink {
    fn handle_chunk(&mut self, _: &[u8]) {}
}

// Some shorthand to clean up our use of Rc<RefCell<*>> in the lol_html macros
// From https://github.com/rust-lang/rfcs/issues/2407#issuecomment-385291238
macro_rules! enclose {
    ( ($( $x:ident ),*) $y:expr ) => {
        {
            $(let $x = $x.clone();)*
            $y
        }
    };
}

/// Active element being tracked
#[derive(Clone, Debug)]
enum ActiveElement {
    Title(String),
    Heading { level: u8, text: String },
    Link { href: String, text: String },
}

/// Internal state for SEO parsing
#[derive(Default, Debug)]
struct SeoParserData {
    title: Option<String>,
    meta_description: Option<String>,
    canonical: Option<String>,
    word_count: usize,
    headings: Vec<Heading>,
    images: Vec<ImageInfo>,
    links: Vec<LinkInfo>,
    // Additional metadata for new rules
    charset: Option<String>,
    viewport: Option<String>,
    lang: Option<String>,
    robots_meta: Option<String>,
    favicon: bool,
    og_tags: std::collections::HashMap<String, String>,
    twitter_card: Option<String>,
    json_ld_scripts: Vec<String>,
    // Stack of active elements (for nested elements)
    active_elements: Vec<ActiveElement>,
    // Track if we're in script/style/noscript to skip word counting
    // Note: This is a heuristic - without end tag callbacks, we can't perfectly track
    // when we exit these elements, but for SEO word counting this is acceptable.
    skip_text_counting: bool,
}

/// SEO HTML parser using lol_html streaming parser
/// 
/// This parser uses lol_html 2.7.0's streaming API. Since end tag callbacks
/// are not available in this version, we use scoped text handlers to accumulate
/// content and process elements when siblings appear or at the end of parsing.
/// 
/// Reference: https://docs.rs/lol_html/2.7.0/lol_html/all.html
pub struct SeoParser<'a> {
    rewriter: HtmlRewriter<'a, EmptySink>,
    data: Rc<RefCell<SeoParserData>>,
}

impl<'a> SeoParser<'a> {
    pub fn new() -> Self {
        let data = Rc::new(RefCell::new(SeoParserData::default()));

        let rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers: vec![
                    // <html> element handler for lang attribute
                    enclose! { (data) element!("html", move |el| {
                        if let Some(lang) = el.get_attribute("lang") {
                            let mut data = data.borrow_mut();
                            if data.lang.is_none() {
                                data.lang = Some(lang);
                            }
                        }
                        Ok(())
                    })},
                    
                    // <title> element handler
                    enclose! { (data) element!("title", move |_el| {
                        let mut data = data.borrow_mut();
                        // Process any previous title (shouldn't happen in valid HTML, but handle it)
                        let mut title_text = None;
                        data.active_elements.retain(|e| {
                            if let ActiveElement::Title(text) = e {
                                let trimmed = text.trim().to_string();
                                if !trimmed.is_empty() {
                                    title_text = Some(trimmed);
                                }
                                false // Remove from active
                            } else {
                                true // Keep other elements
                            }
                        });
                        if let Some(text) = title_text {
                            if data.title.is_none() {
                                data.title = Some(text);
                            }
                        }
                        // Add new title element
                        data.active_elements.push(ActiveElement::Title(String::new()));
                        Ok(())
                    })},
                    
                    // <meta> element handler
                    enclose! { (data) element!("meta", move |el| {
                        let mut data = data.borrow_mut();
                        
                        // Check for charset
                        if let Some(charset) = el.get_attribute("charset") {
                            if data.charset.is_none() {
                                data.charset = Some(charset);
                            }
                        }
                        
                        // Check for http-equiv charset
                        if let Some(http_equiv) = el.get_attribute("http-equiv") {
                            if http_equiv.eq_ignore_ascii_case("content-type") {
                                if let Some(content) = el.get_attribute("content") {
                                    if data.charset.is_none() && content.to_lowercase().contains("charset") {
                                        // Extract charset from content="text/html; charset=UTF-8"
                                        if let Some(charset_part) = content.split(';').nth(1) {
                                            if let Some(charset_val) = charset_part.split('=').nth(1) {
                                                data.charset = Some(charset_val.trim().to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Check for meta description
                        if let Some(name) = el.get_attribute("name") {
                            if name.eq_ignore_ascii_case("description") {
                                if data.meta_description.is_none() {
                                    data.meta_description = el.get_attribute("content");
                                }
                            } else if name.eq_ignore_ascii_case("viewport") {
                                if data.viewport.is_none() {
                                    data.viewport = el.get_attribute("content");
                                }
                            } else if name.eq_ignore_ascii_case("robots") || name.eq_ignore_ascii_case("googlebot") {
                                if data.robots_meta.is_none() {
                                    data.robots_meta = el.get_attribute("content");
                                }
                            }
                        }
                        
                        // Check for Open Graph tags
                        if let Some(property) = el.get_attribute("property") {
                            if property.starts_with("og:") {
                                if let Some(content) = el.get_attribute("content") {
                                    data.og_tags.insert(property, content);
                                }
                            }
                        }
                        
                        // Check for Twitter Card tags
                        if let Some(name) = el.get_attribute("name") {
                            if name.starts_with("twitter:") {
                                if name.eq_ignore_ascii_case("twitter:card") {
                                    if let Some(content) = el.get_attribute("content") {
                                        data.twitter_card = Some(content);
                                    }
                                }
                            }
                        }
                        
                        Ok(())
                    })},
                    
                    // <link> element handler for canonical and favicon
                    enclose! { (data) element!("link", move |el| {
                        let mut data = data.borrow_mut();
                        
                        if let Some(rel) = el.get_attribute("rel") {
                            if rel.eq_ignore_ascii_case("canonical") {
                                if let Some(href) = el.get_attribute("href") {
                                    if data.canonical.is_none() {
                                        data.canonical = Some(href);
                                    }
                                }
                            } else if rel.eq_ignore_ascii_case("icon") || 
                                     rel.eq_ignore_ascii_case("shortcut icon") ||
                                     rel.eq_ignore_ascii_case("apple-touch-icon") {
                                data.favicon = true;
                            }
                        }
                        Ok(())
                    })},
                    
                    // Heading handlers (h1-h6)
                    enclose! { (data) element!("h1, h2, h3, h4, h5, h6", move |el| {
                        let tag_name = el.tag_name();
                        let level = tag_name.as_bytes()[1] - b'0'; // Extract level from tag name
                        
                        let mut data = data.borrow_mut();
                        // Process any previous heading when we see a new one
                        // (this handles the case where a heading closes and a new one starts)
                        let mut headings_to_add = Vec::new();
                        data.active_elements.retain(|e| {
                            if let ActiveElement::Heading { level: prev_level, text } = e {
                                let trimmed = text.trim().to_string();
                                if !trimmed.is_empty() {
                                    headings_to_add.push(Heading {
                                        level: *prev_level,
                                        text: trimmed,
                                    });
                                }
                                false // Remove from active
                            } else {
                                true // Keep other elements
                            }
                        });
                        // Add the processed headings
                        data.headings.extend(headings_to_add);
                        // Add new heading
                        data.active_elements.push(ActiveElement::Heading {
                            level,
                            text: String::new(),
                        });
                        Ok(())
                    })},
                    
                    // <img> element handler
                    enclose! { (data) element!("img", move |el| {
                        let src = el.get_attribute("src");
                        let alt = el.get_attribute("alt");
                        let loading = el.get_attribute("loading");
                        // Note: width/height extraction available but not stored in ImageInfo yet
                        // _ = el.get_attribute("width");
                        // _ = el.get_attribute("height");
                        
                        if let Some(src) = src {
                            let mut data = data.borrow_mut();
                            // Note: ImageInfo doesn't have width/height yet
                            // We'll add that to the model if needed
                            data.images.push(ImageInfo {
                                src,
                                alt,
                                loading,
                            });
                        }
                        Ok(())
                    })},
                    
                    // <a> element handler
                    enclose! { (data) element!("a", move |el| {
                        if let Some(href) = el.get_attribute("href") {
                            let mut data = data.borrow_mut();
                            // Add new link to active elements
                            // We'll process all links at the end to handle nested links correctly
                            data.active_elements.push(ActiveElement::Link {
                                href,
                                text: String::new(),
                            });
                        }
                        Ok(())
                    })},
                    
                    // <script> element handler for JSON-LD
                    enclose! { (data) element!("script", move |el| {
                        let script_type = el.get_attribute("type");
                        if let Some(typ) = script_type {
                            if typ.eq_ignore_ascii_case("application/ld+json") {
                                // We'll extract JSON-LD content in text handler
                                let mut data = data.borrow_mut();
                                data.json_ld_scripts.push(String::new());
                            }
                        }
                        let mut data = data.borrow_mut();
                        data.skip_text_counting = true;
                        Ok(())
                    })},
                    
                    // script, style, noscript - mark for skipping word count
                    enclose! { (data) element!("style, noscript", move |_el| {
                        let mut data = data.borrow_mut();
                        data.skip_text_counting = true;
                        Ok(())
                    })},
                    
                    // Text handler for JSON-LD scripts
                    enclose! { (data) text!("script[type=\"application/ld+json\"]", move |text| {
                        let mut data = data.borrow_mut();
                        if let Some(last) = data.json_ld_scripts.last_mut() {
                            last.push_str(text.as_str());
                        }
                        Ok(())
                    })},
                    
                    // Reset skip flag when we see body or other content elements
                    // This is a heuristic to handle cases where script/style ends
                    enclose! { (data) element!("body, main, article, section, div, p, h1, h2, h3, h4, h5, h6", move |_el| {
                        let mut data = data.borrow_mut();
                        // Optimistically reset - we might be out of script/style now
                        data.skip_text_counting = false;
                        Ok(())
                    })},
                    
                    // Scoped text handlers for element content extraction
                    // Note: These only accumulate text, word counting is done in the general handler
                    enclose! { (data) text!("title", move |text| {
                        let mut data = data.borrow_mut();
                        let text_str = text.as_str();
                        // Accumulate for title
                        for element in data.active_elements.iter_mut() {
                            if let ActiveElement::Title(t) = element {
                                t.push_str(text_str);
                            }
                        }
                        Ok(())
                    })},
                    
                    enclose! { (data) text!("h1, h2, h3, h4, h5, h6", move |text| {
                        let mut data = data.borrow_mut();
                        let text_str = text.as_str();
                        // Accumulate for headings
                        for element in data.active_elements.iter_mut() {
                            if let ActiveElement::Heading { text: t, .. } = element {
                                t.push_str(text_str);
                            }
                        }
                        Ok(())
                    })},
                    
                    enclose! { (data) text!("a", move |text| {
                        let mut data = data.borrow_mut();
                        let text_str = text.as_str();
                        // Accumulate for links
                        for element in data.active_elements.iter_mut() {
                            if let ActiveElement::Link { text: t, .. } = element {
                                t.push_str(text_str);
                            }
                        }
                        Ok(())
                    })},
                    
                    // General text handler for word counting (all text, including in title/heading/link)
                    enclose! { (data) text!("*", move |text| {
                        let mut data = data.borrow_mut();
                        let text_str = text.as_str();
                        // Count words if not in script/style/noscript
                        if !data.skip_text_counting {
                            data.word_count += fast_word_count(text_str);
                        }
                        Ok(())
                    })},
                ],
                strict: false,
                ..Settings::default()
            },
            EmptySink::default(),
        );

        Self { rewriter, data }
    }

    /// Write HTML chunk to parser
    pub fn write(&mut self, data: &[u8]) -> Result<(), lol_html::errors::RewritingError> {
        self.rewriter.write(data)
    }

    /// Finalize parsing and extract results
    pub fn finish(self) -> Result<SeoParserData, lol_html::errors::RewritingError> {
        self.rewriter.end()?;
        // rewriter will be dropped here, clearing extra Rcs on data
        
        let mut data = Rc::try_unwrap(self.data)
            .expect("SeoParserData should have only one reference at finish()")
            .into_inner();
        
        // Process all remaining active elements
        let active_elements = std::mem::take(&mut data.active_elements);
        for element in active_elements {
            match element {
                ActiveElement::Title(text) => {
                    if data.title.is_none() {
                        let trimmed = text.trim().to_string();
                        if !trimmed.is_empty() {
                            data.title = Some(trimmed);
                        }
                    }
                }
                ActiveElement::Heading { level, text } => {
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        data.headings.push(Heading {
                            level,
                            text: trimmed,
                        });
                    }
                }
                ActiveElement::Link { href, text } => {
                    let trimmed = text.trim().to_string();
                    data.links.push(LinkInfo {
                        href,
                        text: trimmed,
                    });
                }
            }
        }
        
        Ok(data)
    }
}

pub fn analyze_html(html: &str, url: &str) -> SeoReport {
    analyze_html_with_options(html, url, false, false)
}

/// Analyze HTML with options for HTTP and AI
pub fn analyze_html_with_options(
    html: &str,
    url: &str,
    http_enabled: bool,
    ai_enabled: bool,
) -> SeoReport {
    let mut parser = SeoParser::new();
    parser.write(html.as_bytes()).unwrap();
    let parser_data = parser.finish().unwrap();

    let mut report = SeoReport {
        url: url.to_string(),
        title: parser_data.title,
        meta_description: parser_data.meta_description,
        canonical: parser_data.canonical,
        word_count: parser_data.word_count,
        headings: parser_data.headings,
        images: parser_data.images,
        links: parser_data.links,
        issues: Vec::new(),
        score: SeoScore::default(),
        charset: parser_data.charset,
        viewport: parser_data.viewport,
        lang: parser_data.lang,
        robots_meta: parser_data.robots_meta,
        favicon: parser_data.favicon,
        og_tags: parser_data.og_tags,
        twitter_card: parser_data.twitter_card,
        json_ld_scripts: parser_data.json_ld_scripts,
    };

    // Create rule context
    let context = RuleContext::new(url.to_string())
        .with_http(http_enabled)
        .with_ai(ai_enabled);

    // Phase 1: Execute Tier A (streaming) and Tier B (full document) rules in parallel
    let tier_a_rules = get_tier_a_rules();
    let tier_b_rules = get_tier_b_rules();

    // Parallel rule execution using Rayon
    // Rules are CPU-bound, pure (read-only), and thread-safe (Send + Sync)
    let issues: Vec<SeoIssue> = tier_a_rules
        .iter()
        .chain(tier_b_rules.iter())
        .filter(|rule| rule.can_run(&context))
        .par_bridge()
        .flat_map(|rule| rule.check(&report, &context))
        .collect();

    report.issues.extend(issues);

    // Phase 2: Execute Tier C (live HTTP) rules if enabled
    // Note: Tier C rules may have network I/O, so we keep them sequential for now
    // If they become CPU-bound in the future, we can parallelize them too
    if http_enabled {
        let tier_c_rules = get_tier_c_rules();
        for rule in tier_c_rules.iter() {
            if rule.can_run(&context) {
                let rule_issues = rule.check(&report, &context);
                report.issues.extend(rule_issues);
            }
        }
    }

    // Also run legacy rules for backward compatibility
    // This ensures existing code continues to work
    apply_rules(&mut report);

    report.score = compute_score(&report.issues);
    report
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
