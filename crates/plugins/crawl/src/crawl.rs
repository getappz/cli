//! Multi-page crawl: discover links, filter, fetch each page, convert to markdown.

use crate::vfs_wasm::CrawlVfs;
use crawl_core::{
    extract_links, filter_links, post_process_markdown, process_sitemap, FilterLinksCall,
};
use std::collections::{HashSet, VecDeque};
use url::Url;

/// Options for a full crawl.
pub struct CrawlOptions {
    pub base_url: String,
    pub output_dir: String,
    pub limit: u32,
    pub max_depth: u32,
    pub include_paths: Vec<String>,
    pub exclude_paths: Vec<String>,
    pub allow_subdomains: bool,
    pub sitemap_mode: SitemapMode,
    pub want_raw_html: bool,
    pub strict_ssl: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SitemapMode {
    Skip,
    Include,
    Only,
}

/// URL to filesystem-safe slug.
fn url_to_slug(url: &str) -> String {
    let u = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return "page".to_string(),
    };
    let path = u.path().trim_matches('/');
    if path.is_empty() {
        "index".to_string()
    } else {
        path.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else if c == '/' {
                    '_'
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .split('_')
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("_")
    }
}

/// Run a full crawl.
pub fn crawl(vfs: &dyn CrawlVfs, opts: &CrawlOptions) -> Result<u32, String> {
    let mut written = 0u32;
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();

    let robots_txt = fetch_robots(vfs, &opts.base_url, opts.strict_ssl).unwrap_or_default();

    match opts.sitemap_mode {
        SitemapMode::Only => {
            let sitemap_urls =
                fetch_sitemap_urls(vfs, &opts.base_url, opts.strict_ssl).unwrap_or_default();
            for url in sitemap_urls.into_iter().take(opts.limit as usize) {
                if visited.contains(&url) {
                    continue;
                }
                visited.insert(url.clone());
                if scrape_one(vfs, &url, &opts.output_dir, opts.want_raw_html, opts.strict_ssl).is_ok() { written += 1 }
            }
            return Ok(written);
        }
        SitemapMode::Include => {
            let sitemap_urls =
                fetch_sitemap_urls(vfs, &opts.base_url, opts.strict_ssl).unwrap_or_default();
            for url in sitemap_urls {
                queue.push_back((url, 0));
            }
        }
        SitemapMode::Skip => {}
    }

    queue.push_back((opts.base_url.clone(), 0));

    while let Some((url, depth)) = queue.pop_front() {
        if written >= opts.limit {
            break;
        }
        if depth > opts.max_depth {
            continue;
        }
        if visited.contains(&url) {
            continue;
        }
        visited.insert(url.clone());

        match scrape_one(vfs, &url, &opts.output_dir, opts.want_raw_html, opts.strict_ssl) {
            Ok(_) => written += 1,
            Err(_) => continue,
        }

        if depth >= opts.max_depth {
            continue;
        }

        let html = match fetch_html(vfs, &url, opts.strict_ssl) {
            Ok(h) => h,
            Err(_) => continue,
        };

        let links = extract_links(Some(html)).map_err(|e| format!("extract_links: {}", e))?;

        let filter_call = FilterLinksCall {
            links,
            limit: Some((opts.limit - written) as i64),
            max_depth: opts.max_depth.saturating_sub(depth),
            base_url: opts.base_url.clone(),
            initial_url: opts.base_url.clone(),
            regex_on_full_url: true,
            excludes: opts.exclude_paths.clone(),
            includes: opts.include_paths.clone(),
            allow_backward_crawling: true,
            ignore_robots_txt: robots_txt.is_empty(),
            robots_txt: robots_txt.clone(),
            allow_external_content_links: false,
            allow_subdomains: opts.allow_subdomains,
        };

        let result = filter_links(filter_call).map_err(|e| format!("filter_links: {}", e))?;

        for link in result.links {
            if !visited.contains(&link) {
                queue.push_back((link, depth + 1));
            }
        }
    }

    Ok(written)
}

fn fetch_robots(vfs: &dyn CrawlVfs, base: &str, strict_ssl: bool) -> Result<String, String> {
    let u = Url::parse(base).map_err(|e| e.to_string())?;
    let robots_url = format!(
        "{}://{}/robots.txt",
        u.scheme(),
        u.host_str().unwrap_or("")
    );
    let dest = ".crawl/cache/robots.txt";
    vfs.mkdir(".crawl/cache")?;
    vfs.download(&robots_url, dest, strict_ssl)?;
    vfs.read_file(dest)
}

fn fetch_sitemap_urls(
    vfs: &dyn CrawlVfs,
    base: &str,
    strict_ssl: bool,
) -> Result<Vec<String>, String> {
    let u = Url::parse(base).map_err(|e| e.to_string())?;
    let sitemap_url = format!(
        "{}://{}/sitemap.xml",
        u.scheme(),
        u.host_str().unwrap_or("")
    );
    let dest = ".crawl/cache/sitemap.xml";
    vfs.mkdir(".crawl/cache")?;
    vfs.download(&sitemap_url, dest, strict_ssl)?;
    let xml = vfs.read_file(dest)?;
    let result = process_sitemap(xml)?;
    let mut urls = Vec::new();
    for inst in result.instructions {
        urls.extend(inst.urls);
    }
    Ok(urls)
}

fn fetch_html(vfs: &dyn CrawlVfs, url: &str, strict_ssl: bool) -> Result<String, String> {
    let cache = ".crawl/cache/page.html";
    vfs.mkdir(".crawl/cache")?;
    vfs.download(url, cache, strict_ssl)?;
    vfs.read_file(cache)
}

fn scrape_one(
    vfs: &dyn CrawlVfs,
    url: &str,
    output_dir: &str,
    want_raw_html: bool,
    strict_ssl: bool,
) -> Result<(), String> {
    let slug = url_to_slug(url);
    let slug = if slug.is_empty() { "index" } else { &slug };
    let output_path = format!("{}/{}.md", output_dir.trim_end_matches('/'), slug);

    let html = fetch_html(vfs, url, strict_ssl)?;

    if want_raw_html {
        let html_path = format!("{}/{}.html", output_dir.trim_end_matches('/'), slug);
        vfs.write_file(&html_path, &html)?;
    }

    let markdown = htmd::convert(&html).map_err(|e| e.to_string())?;
    let markdown = post_process_markdown(markdown).unwrap_or_else(|_| String::new());

    vfs.mkdir(output_dir)?;
    vfs.write_file(&output_path, &markdown)?;

    Ok(())
}

