//! Build appz.dev's public asset URLs from the bytes in its upload manifest.
use crate::static_cas::{
    LocalBlob, MAX_FILE_SIZE, ScannedFileEntry, ScannedTree, blob_id, has_content_hash,
    scan_output_dir,
};
use lol_html::{RewriteStrSettings, element, rewrite_str};
use percent_encoding::percent_decode_str;
use regex::{Captures, Regex};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;
use tempfile::TempDir;
use url::Url;

type Aliases = HashMap<String, String>;

// Keep the original paths for runtime imports, SRI, service workers and incoming links.
// The aliases share CAS blobs with their originals unless a stylesheet was rewritten.
pub(crate) fn prepare_upload(root: &Path) -> Result<(ScannedTree, TempDir), String> {
    let mut tree = scan_output_dir(root)?;
    let stage = tempfile::tempdir().map_err(|e| format!("cannot stage upload: {e}"))?;
    let mut occupied: HashSet<_> = tree.files.iter().map(|f| f.path.clone()).collect();
    let mut aliases = Aliases::new();
    let mut additions = Vec::new();
    for file in &tree.files {
        if eligible(file)
            && !file.r#type.starts_with("text/css")
            && !imports_siblings(file, &tree.blobs_by_hash)
        {
            add_alias(file, &mut occupied, &mut aliases, &mut additions);
        }
    }
    // All stylesheets see the same map. CSS imports can still use their original URL.
    let mut css_aliases = Aliases::new();
    for file in &tree.files {
        if eligible(file) && file.r#type.starts_with("text/css") {
            let mut css = file.clone();
            if !aliases.is_empty() {
                rewrite_blob(&mut css, &mut tree.blobs_by_hash, &stage, |source| {
                    Ok(rewrite_css(source, &file_url(&file.path), &aliases))
                })?;
            }
            add_alias(&css, &mut occupied, &mut css_aliases, &mut additions);
        }
    }
    aliases.extend(css_aliases);
    if !aliases.is_empty() {
        for file in &mut tree.files {
            if file.r#type.starts_with("text/html") && file.immutable != Some(true) {
                let base = file_url(&file.path);
                rewrite_blob(file, &mut tree.blobs_by_hash, &stage, |source| {
                    rewrite_html(source, base, &aliases)
                })?;
            }
        }
    }
    tree.files.extend(additions);
    tree.total_bytes = tree.files.iter().map(|f| f.size).sum();
    let live: HashSet<_> = tree.files.iter().map(|f| &f.hash).collect();
    tree.blobs_by_hash.retain(|hash, _| live.contains(hash));
    Ok((tree, stage))
}

fn eligible(file: &ScannedFileEntry) -> bool {
    let mime = file.r#type.split(';').next().unwrap_or_default();
    let legacy_hash = file
        .path
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .and_then(|(stem, _)| stem.rsplit_once(['-', '_']))
        .is_some_and(|(_, suffix)| {
            suffix.len() >= 8 && suffix.bytes().all(|b| b.is_ascii_hexdigit())
        });
    file.immutable != Some(true)
        && !legacy_hash
        && !has_content_hash(&file.path)
        && !file.path.ends_with("bundle.min.js")
        && (matches!(
            mime,
            "text/css" | "application/javascript" | "application/wasm" | "application/pdf"
        ) || ["image/", "font/", "audio/", "video/"]
            .iter()
            .any(|p| mime.starts_with(p)))
}

// A cached-forever alias must not import siblings by their unhashed paths: after a
// redeploy it would load newer chunks against old code, so such scripts keep one URL.
fn imports_siblings(file: &ScannedFileEntry, blobs: &HashMap<String, LocalBlob>) -> bool {
    static IMPORT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"\b(?:from|import)\s*\(?\s*["'][./]"#).unwrap());
    file.r#type.starts_with("application/javascript")
        && std::fs::read_to_string(&blobs[&file.hash].path).is_ok_and(|js| IMPORT.is_match(&js))
}

fn add_alias(
    file: &ScannedFileEntry,
    occupied: &mut HashSet<String>,
    aliases: &mut Aliases,
    additions: &mut Vec<ScannedFileEntry>,
) {
    let Some((stem, ext)) = file.path.rsplit_once('.') else {
        return;
    };
    let suffix = &file.hash.trim_start_matches("b3:")[..10];
    let path = format!("{stem}-{suffix}.{ext}");
    if occupied.insert(path.clone()) {
        aliases.insert(file.path.clone(), suffix.to_owned());
        additions.push(ScannedFileEntry {
            path,
            immutable: Some(true),
            ..file.clone()
        });
    }
}

fn rewrite_blob(
    file: &mut ScannedFileEntry,
    blobs: &mut HashMap<String, LocalBlob>,
    stage: &TempDir,
    rewrite: impl FnOnce(&str) -> Result<String, String>,
) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(file.size);
    std::fs::File::open(&blobs[&file.hash].path)
        .and_then(|f| f.take((MAX_FILE_SIZE + 1) as u64).read_to_end(&mut bytes))
        .map_err(|e| format!("cannot read {}: {e}", file.path))?;
    if bytes.len() != file.size || blob_id(&bytes) != file.hash {
        return Err(format!(
            "{} changed during deploy; retry the deploy",
            file.path
        ));
    }
    let Ok(source) = std::str::from_utf8(&bytes) else {
        eprintln!(
            "warning: {} is not UTF-8; its asset references are not fingerprinted",
            file.path
        );
        return Ok(());
    };
    let updated = rewrite(source)?;
    if updated == source {
        return Ok(());
    }
    if updated.len() > MAX_FILE_SIZE {
        return Err(format!("{} exceeds 25 MB after fingerprinting", file.path));
    }
    file.size = updated.len();
    file.hash = blob_id(updated.as_bytes());
    let path = stage.path().join(file.hash.trim_start_matches("b3:"));
    std::fs::write(&path, updated).map_err(|e| format!("cannot stage {}: {e}", file.path))?;
    blobs.insert(
        file.hash.clone(),
        LocalBlob {
            hash: file.hash.clone(),
            path,
        },
    );
    Ok(())
}

fn file_url(path: &str) -> Url {
    let mut url = Url::parse("https://appz.invalid/").unwrap();
    url.path_segments_mut()
        .unwrap()
        .clear()
        .extend(path[1..].split('/'));
    url
}

// The site path a same-site URL points at, without its query or fragment.
fn site_path(raw: &str, base: &Url) -> Option<String> {
    let path = &raw[..raw.find(['?', '#']).unwrap_or(raw.len())];
    if path.is_empty() || path.starts_with("//") || path.contains([':', '\\']) {
        return None;
    }
    let resolved = base.join(path).ok()?;
    if resolved.host_str() != Some("appz.invalid") {
        return None;
    }
    Some(
        percent_decode_str(resolved.path())
            .decode_utf8()
            .ok()?
            .into_owned(),
    )
}

fn rewrite_url(raw: &str, base: &Url, aliases: &Aliases) -> Option<String> {
    let end = raw.find(['?', '#']).unwrap_or(raw.len());
    let path = &raw[..end];
    let key = site_path(raw, base)?;
    let suffix = aliases.get(&key)?;
    let (stem, ext) = path.rsplit_once('.')?;
    if ext.contains('/') || key.rsplit_once('.')?.1 != percent_decode_str(ext).decode_utf8().ok()? {
        return None;
    }
    Some(format!("{stem}-{suffix}.{ext}{}", &raw[end..]))
}

fn rewrite_css(source: &str, base: &Url, aliases: &Aliases) -> String {
    // Consume comments and ordinary strings so their URL-like text stays literal.
    static TOKENS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
        r#"(?is)/\*.*?(?:\*/|$)|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|\burl\(\s*(?:"(?P<dq>[^"\\]*)"|'(?P<sq>[^'\\]*)'|(?P<bare>[^\s()'"\\]+))\s*\)"#
    ).unwrap()
    });
    TOKENS
        .replace_all(source, |cap: &Captures| {
            let token = cap.get(0).unwrap();
            if let Some(value) = cap.name("dq").or(cap.name("sq")).or(cap.name("bare"))
                && let Some(url) = rewrite_url(value.as_str(), base, aliases)
            {
                return format!(
                    "{}{url}{}",
                    &source[token.start()..value.start()],
                    &source[value.end()..token.end()]
                );
            }
            token.as_str().to_owned()
        })
        .into_owned()
}

fn rewrite_srcset(raw: &str, base: &Url, aliases: &Aliases) -> String {
    let mut rest = raw;
    let mut result = String::with_capacity(raw.len());
    while !rest.is_empty() {
        let start = rest.trim_start_matches(|c: char| c == ',' || c.is_ascii_whitespace());
        result.push_str(&rest[..rest.len() - start.len()]);
        let end = start
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(start.len());
        // Commas inside a URL are data. Trailing commas terminate the candidate.
        let token = &start[..end];
        let url = token.trim_end_matches(',');
        if let Some(updated) = rewrite_url(url, base, aliases) {
            result.push_str(&updated);
        } else {
            result.push_str(url);
        }
        result.push_str(&token[url.len()..]);
        rest = &start[end..];
        if url.len() == token.len() {
            let next = rest.find(',').unwrap_or(rest.len());
            result.push_str(&rest[..next]);
            rest = &rest[next..];
        }
    }
    result
}

fn rewrite_html(source: &str, mut base: Url, aliases: &Aliases) -> Result<String, String> {
    static BASE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<base\s").unwrap());
    if BASE.is_match(source) {
        let mut href = None;
        rewrite_str(
            source,
            RewriteStrSettings {
                element_content_handlers: vec![element!("base[href]", |el| {
                    if href.is_none() {
                        href = el.get_attribute("href");
                    }
                    Ok(())
                })],
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
        if let Some(href) = href {
            let Ok(url) = base.join(&href) else {
                return Ok(source.to_owned());
            };
            if url.origin() != base.origin() {
                return Ok(source.to_owned());
            }
            base = url;
        }
    }
    // A URL pinned by integrity anywhere on the page keeps one name everywhere,
    // so preloads of it hit the cache entry the pinned element uses.
    let mut pinned = HashSet::new();
    rewrite_str(
        source,
        RewriteStrSettings {
            element_content_handlers: vec![element!("[integrity]", |el| {
                for attr in ["src", "href", "poster"] {
                    if let Some(key) = el.get_attribute(attr).and_then(|r| site_path(&r, &base)) {
                        pinned.insert(key);
                    }
                }
                Ok(())
            })],
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let mut aliases = aliases.clone();
    aliases.retain(|path, _| !pinned.contains(path));
    let aliases = &aliases;
    rewrite_str(source, RewriteStrSettings {
        element_content_handlers: vec![element!(
            "script[src], link[href], link[imagesrcset], img[src], img[srcset], source[src], source[srcset], video[src], video[poster], audio[src], iframe[src], a[href]",
            |el| {
                if el.has_attribute("integrity") {
                    return Ok(());
                }
                for attr in ["src", "href", "poster"] {
                    if let Some(raw) = el.get_attribute(attr)
                        && let Some(url) = rewrite_url(&raw, &base, aliases)
                    {
                        el.set_attribute(attr, &url)?;
                    }
                }
                for attr in ["srcset", "imagesrcset"] {
                    if let Some(raw) = el.get_attribute(attr) {
                        let updated = rewrite_srcset(&raw, &base, aliases);
                        if updated != raw {
                            el.set_attribute(attr, &updated)?;
                        }
                    }
                }
                Ok(())
            }
        )], ..Default::default()
    }).map_err(|e| format!("cannot rewrite {base}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn upload_names_and_references_follow_the_final_bytes() {
        let site = tempfile::tempdir().unwrap();
        let root = site.path();
        fs::create_dir(root.join("css")).unwrap();
        fs::create_dir(root.join("pages")).unwrap();
        let css = "body { background: url('../photo.png?v=1#hero') }";
        let html = r#"<link href="../css/app.min.css"><link integrity="test" href="../css/app.min.css"><script src="/main.js"></script><img srcset="../photo.png 1x, //cdn.example/photo.png 2x"><img src="https://cdn.example/photo.png">"#;
        fs::write(root.join("css/app.min.css"), css).unwrap();
        fs::write(root.join("pages/index.html"), html).unwrap();
        fs::write(root.join("main.js"), "export {};").unwrap();
        for photo in ["first", "first", "second"] {
            fs::write(root.join("photo.png"), photo).unwrap();
            let (tree, _stage) = prepare_upload(root).unwrap();
            let img = format!(
                "photo-{}.png",
                &blake3::hash(photo.as_bytes()).to_hex()[..10]
            );
            let updated_css = css.replace("photo.png", &img);
            let sheet = format!(
                "app.min-{}.css",
                &blake3::hash(updated_css.as_bytes()).to_hex()[..10]
            );
            let js = format!("main-{}.js", &blake3::hash(b"export {};").to_hex()[..10]);
            // The integrity-pinned stylesheet keeps its URL on the whole page.
            let expected = html
                .replace("/main.js", &format!("/{js}"))
                .replace("../photo.png", &format!("../{img}"));
            let mut seen = HashSet::new();
            for entry in &tree.files {
                assert!(seen.insert(entry.path.clone()));
                let bytes = fs::read(&tree.blobs_by_hash[&entry.hash].path).unwrap();
                assert_eq!(blob_id(&bytes), entry.hash);
                assert_eq!(bytes.len(), entry.size);
                if entry.path == "/pages/index.html" {
                    assert_eq!(bytes, expected.as_bytes());
                    assert_eq!(entry.immutable, None);
                } else if entry.path == format!("/css/{sheet}") {
                    assert_eq!(bytes, updated_css.as_bytes());
                    assert_eq!(entry.immutable, Some(true));
                }
            }
            assert_eq!(seen.len(), 7);
            for path in [format!("/{img}"), format!("/css/{sheet}"), format!("/{js}")] {
                assert!(seen.contains(&path));
            }
            assert_eq!(
                fs::read_to_string(root.join("pages/index.html")).unwrap(),
                html
            );
            assert_eq!(
                fs::read_to_string(root.join("css/app.min.css")).unwrap(),
                css
            );
        }
    }

    #[test]
    fn pinned_urls_and_scripts_importing_siblings_keep_one_name() {
        let aliases = Aliases::from([("/app.js".into(), "1234567890".into())]);
        let html = r#"<link rel=modulepreload href="/app.js"><script src="/app.js" integrity="x"></script>"#;
        assert_eq!(
            rewrite_html(html, file_url("/index.html"), &aliases).unwrap(),
            html
        );
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("main.js"), "import './chunk.js';").unwrap();
        fs::write(site.path().join("chunk.js"), "export {};").unwrap();
        let (tree, _stage) = prepare_upload(site.path()).unwrap();
        let paths: Vec<_> = tree.files.iter().map(|f| f.path.as_str()).collect();
        assert!(!paths.iter().any(|p| p.starts_with("/main-")));
        assert!(paths.iter().any(|p| p.starts_with("/chunk-")));
    }

    #[test]
    fn existing_paths_bundles_and_external_bases_are_preserved() {
        let site = tempfile::tempdir().unwrap();
        let root = site.path();
        fs::write(root.join("a.png"), "abc").unwrap();
        let occupied = format!("a-{}.png", &blake3::hash(b"abc").to_hex()[..10]);
        fs::write(root.join(&occupied), "different").unwrap();
        fs::write(root.join("bundle.min.js"), "stable").unwrap();
        let html = "<base href='https://cdn.example/'><img src='/b.png'>";
        fs::write(root.join("index.html"), html).unwrap();
        fs::write(root.join("b.png"), "another").unwrap();
        fs::write(root.join(".appzignore"), "ignored.png\n").unwrap();
        fs::write(root.join("ignored.png"), "ignored").unwrap();
        let (tree, _stage) = prepare_upload(root).unwrap();
        assert_eq!(tree.files.len(), 6);
        let entry = tree
            .files
            .iter()
            .find(|f| f.path == format!("/{occupied}"))
            .unwrap();
        assert_eq!(entry.hash, blob_id(b"different"));
        assert!(
            tree.files
                .iter()
                .any(|f| f.path == "/index.html" && f.hash == blob_id(html.as_bytes()))
        );
    }

    #[test]
    fn relative_urls_respect_the_base_and_srcset_keeps_data_and_descriptors() {
        let aliases = Aliases::from([("/media/hero image.png".into(), "1234567890".into())]);
        let source = r#"<base href="/media/"><img srcset="data:image/png;base64,abc 1x, hero%20image.png?v=2#x 2x">"#;
        let updated = rewrite_html(source, file_url("/pages/index.html"), &aliases).unwrap();
        assert_eq!(
            updated,
            source.replace("image.png?v", "image-1234567890.png?v")
        );
        let css = r#"/* url('hero image.png') */ a{content:"url('hero image.png')";background:url("hero image.png")}"#;
        assert_eq!(
            rewrite_css(css, &file_url("/media/main.css"), &aliases),
            css.replace(
                "background:url(\"hero image.png",
                "background:url(\"hero image-1234567890.png"
            )
        );
    }
}
