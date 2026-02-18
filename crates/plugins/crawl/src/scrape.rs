//! Single-page scrape: fetch URL, convert HTML to markdown, write output.

use crate::vfs_wasm::CrawlVfs;
use crawl_core::post_process_markdown;
use htmd;

/// Scrape a single URL to markdown.
pub fn scrape_url(
    vfs: &dyn CrawlVfs,
    url: &str,
    output_path: &str,
    want_raw_html: bool,
    strict_ssl: bool,
) -> Result<String, String> {
    let cache_path = ".crawl/cache/page.html";

    vfs.mkdir(".crawl/cache")?;
    vfs.download(url, cache_path, strict_ssl)?;

    let html = vfs.read_file(cache_path)?;

    if want_raw_html {
        let html_path = output_path
            .strip_suffix(".md")
            .map(|s| format!("{}.html", s))
            .unwrap_or_else(|| format!("{}.html", output_path));
        vfs.write_file(&html_path, &html)?;
    }

    let markdown = htmd::convert(&html).map_err(|e| e.to_string())?;
    let markdown = post_process_markdown(markdown).unwrap_or_else(|_| String::new());

    if let Some((dir, _)) = output_path.rsplit_once('/') {
        if !dir.is_empty() && dir != "." {
            let _ = vfs.mkdir(dir);
        }
    }

    vfs.write_file(output_path, &markdown)?;

    Ok(markdown)
}
