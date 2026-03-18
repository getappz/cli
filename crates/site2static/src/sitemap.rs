use url::Url;
use crate::downloader::Downloader;

/// Discover all page URLs from sitemaps. Returns empty vec on any failure.
pub fn discover_urls(origin: &Url, downloader: &Downloader) -> Vec<Url> {
    let sitemap_url = match origin.join("/sitemap.xml") {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("Failed to construct sitemap URL: {}", e);
            return Vec::new();
        }
    };

    tracing::info!("Checking sitemap at {}", sitemap_url);
    let xml = match fetch_xml(downloader, &sitemap_url) {
        Some(xml) => xml,
        None => return Vec::new(),
    };

    let mut all_urls = Vec::new();
    process_recursive(downloader, &xml, &mut all_urls);
    tracing::info!("Sitemap discovery found {} page URLs", all_urls.len());
    all_urls
}

fn process_recursive(downloader: &Downloader, xml: &str, urls: &mut Vec<Url>) {
    let result = match crawl_core::process_sitemap(xml.to_string()) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to parse sitemap: {}", e);
            return;
        }
    };

    for instr in &result.instructions {
        match instr.action.as_str() {
            "process" => {
                for url_str in &instr.urls {
                    if let Ok(url) = Url::parse(url_str) {
                        urls.push(url);
                    }
                }
            }
            "recurse" => {
                for url_str in &instr.urls {
                    if let Ok(url) = Url::parse(url_str) {
                        if let Some(child_xml) = fetch_xml(downloader, &url) {
                            process_recursive(downloader, &child_xml, urls);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn fetch_xml(downloader: &Downloader, url: &Url) -> Option<String> {
    match downloader.get(url) {
        Ok(resp) => {
            let bytes = match resp.data {
                crate::response::ResponseData::Html(b)
                | crate::response::ResponseData::Css(b)
                | crate::response::ResponseData::Other(b) => b,
            };
            // Handle gzip if Content-Encoding wasn't handled by reqwest
            if url.path().ends_with(".xml.gz") && bytes.starts_with(&[0x1f, 0x8b]) {
                use flate2::read::GzDecoder;
                use std::io::Read;
                let mut decoder = GzDecoder::new(&bytes[..]);
                let mut xml = String::new();
                decoder.read_to_string(&mut xml).ok()?;
                Some(xml)
            } else {
                String::from_utf8(bytes).ok()
            }
        }
        Err(e) => {
            tracing::debug!("Could not fetch sitemap {}: {}", url, e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_urlset_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url><loc>https://example.com/</loc></url>
            <url><loc>https://example.com/about/</loc></url>
        </urlset>"#;
        let result = crawl_core::process_sitemap(xml.to_string()).unwrap();
        assert!(!result.instructions.is_empty());
        let process_instr = result.instructions.iter()
            .find(|i| i.action == "process").unwrap();
        assert_eq!(process_instr.urls.len(), 2);
    }
}
