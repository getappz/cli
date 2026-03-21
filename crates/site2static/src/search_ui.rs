//! Search UI injection into HTML pages.
//!
//! Runs a separate lol_html rewrite pass to:
//! 1. Inject Pagefind CSS/JS references into <head>
//! 2. Replace WordPress search forms with Pagefind UI widgets
//! 3. Inject a floating Cmd+K search modal before </body>

use std::cell::RefCell;

use lol_html::{element, rewrite_str, RewriteStrSettings};

use crate::SearchUiConfig;

/// Pagefind UI CSS link tag.
const PAGEFIND_CSS: &str =
    r#"<link rel="stylesheet" href="/pagefind/pagefind-ui.css">"#;

/// Pagefind UI JS script tag.
const PAGEFIND_JS: &str =
    r#"<script src="/pagefind/pagefind-ui.js"></script>"#;

/// Inject search UI markup into an HTML page.
///
/// Runs its own `lol_html::rewrite_str` pass.
/// Input: UTF-8 HTML (after dom.rs URL rewriting).
/// Output: UTF-8 HTML with search UI injected.
pub fn inject_search_ui(html: &str, config: &SearchUiConfig) -> Result<String, lol_html::errors::RewritingError> {
    let head_injected = RefCell::new(false);
    let form_counter = RefCell::new(0u32);
    // Dedup guard: a single element matching multiple selectors (e.g. a
    // <form class="search-form" role="search">) must only be replaced once.
    // We build a stable string key from tag name + all attribute name=value
    // pairs so the same element always maps to the same key within one page.
    let replaced_forms: RefCell<std::collections::HashSet<String>> = RefCell::new(Default::default());
    let body_injected = RefCell::new(false);
    let content_marked = RefCell::new(false);

    let mut element_handlers: Vec<_> = vec![
        // Inject Pagefind CSS/JS into <head>
        element!("head", |el| {
            if !*head_injected.borrow() {
                el.append(&format!("\n{}\n{}", PAGEFIND_CSS, PAGEFIND_JS), lol_html::html_content::ContentType::Html);
                *head_injected.borrow_mut() = true;
            }
            Ok(())
        }),
    ];

    // Content hinting: mark first main content element with data-pagefind-body
    for selector in &["main", "article", ".entry-content", "#content"] {
        let content_marked = &content_marked;
        element_handlers.push(
            element!(selector, |el| {
                if !*content_marked.borrow() {
                    el.set_attribute("data-pagefind-body", "")?;
                    *content_marked.borrow_mut() = true;
                }
                Ok(())
            })
        );
    }

    // WordPress search form replacement.
    // Guard: an element matching multiple selectors (e.g. <form class="search-form" role="search">)
    // is only replaced once. We use a stable string key (tag + attributes) for dedup.
    if config.replace_existing {
        for selector in &["form.search-form", "form[role=\"search\"]", ".wp-block-search"] {
            let form_counter = &form_counter;
            let replaced_forms = &replaced_forms;
            element_handlers.push(
                element!(selector, |el| {
                    let key = format!(
                        "{}{}",
                        el.tag_name(),
                        el.attributes().iter().map(|a| format!("{}={}", a.name(), a.value())).collect::<String>()
                    );
                    if !replaced_forms.borrow_mut().insert(key) {
                        return Ok(()); // Already replaced by a prior selector
                    }
                    let mut counter = form_counter.borrow_mut();
                    let id = *counter;
                    *counter += 1;
                    let replacement = format!(
                        r#"<div id="pagefind-replace-{id}" data-pagefind-ignore></div><script>new PagefindUI({{ element: "#pagefind-replace-{id}", showSubResults: true }});</script>"#,
                    );
                    el.set_inner_content(&replacement, lol_html::html_content::ContentType::Html);
                    Ok(())
                })
            );
        }
    }

    // Floating modal injection before </body>
    if config.keyboard_shortcut {
        let body_injected = &body_injected;
        element_handlers.push(
            element!("body", |el| {
                if !*body_injected.borrow() {
                    el.append(SEARCH_MODAL_HTML, lol_html::html_content::ContentType::Html);
                    *body_injected.borrow_mut() = true;
                }
                Ok(())
            })
        );
    }

    rewrite_str(html, RewriteStrSettings {
        element_content_handlers: element_handlers,
        ..RewriteStrSettings::default()
    })
}

/// Self-contained HTML/CSS/JS for the floating search modal.
const SEARCH_MODAL_HTML: &str = r##"
<div id="s2s-search-overlay" data-pagefind-ignore style="display:none;position:fixed;inset:0;z-index:99999;background:rgba(0,0,0,0.5);align-items:flex-start;justify-content:center;padding-top:min(20vh,120px)">
  <style>
    #s2s-search-dialog{background:#fff;border-radius:12px;padding:0;border:1px solid #ddd;width:90%;max-width:620px;box-shadow:0 16px 70px rgba(0,0,0,0.3);max-height:80vh;overflow:auto}
    #s2s-search-dialog .pagefind-ui__search-input{font-size:18px;padding:12px 16px;width:100%;box-sizing:border-box;border:none;border-bottom:1px solid #eee;outline:none}
    #s2s-search-dialog .pagefind-ui__result-link{color:#1a0dab;text-decoration:none}
    #s2s-search-dialog .pagefind-ui__result-link:hover{text-decoration:underline}
    @media(prefers-color-scheme:dark){#s2s-search-dialog{background:#1e1e1e;border-color:#444;color:#e0e0e0}#s2s-search-dialog .pagefind-ui__search-input{border-bottom-color:#444;color:#e0e0e0}}
    #s2s-search-kbd{position:absolute;right:16px;top:50%;transform:translateY(-50%);font-size:12px;color:#999;pointer-events:none}
  </style>
  <div id="s2s-search-dialog" role="dialog" aria-label="Site search">
    <div id="s2s-search-mount"></div>
  </div>
</div>
<script>
(function(){
  var overlay=document.getElementById('s2s-search-overlay');
  var mount=document.getElementById('s2s-search-mount');
  var ui;
  function open(){
    if(!ui){ui=new PagefindUI({element:mount,showSubResults:true,autofocus:true})}
    overlay.style.display='flex';
    var input=mount.querySelector('input');
    if(input)input.focus();
  }
  function close(){overlay.style.display='none'}
  document.addEventListener('keydown',function(e){
    if((e.metaKey||e.ctrlKey)&&e.key==='k'){e.preventDefault();overlay.style.display==='flex'?close():open()}
    if(e.key==='Escape'&&overlay.style.display==='flex'){e.preventDefault();close()}
  });
  overlay.addEventListener('click',function(e){if(e.target===overlay)close()});
})();
</script>
"##;
