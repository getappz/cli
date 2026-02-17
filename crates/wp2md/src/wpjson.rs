//! WordPress REST API (wp-json) client for fetching posts and pages.
//!
//! Fetches content via `/wp-json/wp/v2/posts` and `/wp-json/wp/v2/pages`
//! with `_embed` to include author, terms, and featured media in one request.

use crate::common;
use crate::config::Wp2mdConfig;
use crate::types::{ImageRef, Post};
use chrono::{DateTime, NaiveDateTime};
use miette::{miette, Result};
use regex::Regex;
use serde::Deserialize;

/// Image extensions for validating scraped image URLs.
const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "tiff", "tif", "ico",
];

/// Fetch posts and pages from WordPress REST API and convert to our Post type.
pub fn fetch_posts_from_wpjson(
    vfs: &dyn crate::vfs::Wp2mdVfs,
    base_url: &str,
    config: &Wp2mdConfig,
) -> Result<Vec<Post>> {
    let api_base = normalize_api_url(base_url)?;

    // Ensure output dir exists for temp file
    vfs.create_dir_all(&config.output)?;

    let temp_path = format!("{}/.wp2md_wpjson_page.json", config.output);

    let mut posts = Vec::new();

    // Fetch posts
    fetch_and_append(&api_base, "posts", config, vfs, &temp_path, &mut posts)?;

    // Fetch pages if configured
    if config.wpjson_include_pages {
        fetch_and_append(&api_base, "pages", config, vfs, &temp_path, &mut posts)?;
    }

    // Sort by date (newest first) to match typical WXR order
    posts.sort_by(|a, b| {
        match (a.date, b.date) {
            (Some(da), Some(db)) => db.cmp(&da),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    Ok(posts)
}

fn normalize_api_url(url: &str) -> Result<String> {
    let url = url.trim().trim_end_matches('/');
    let base = if url.ends_with("/wp-json") {
        url.to_string()
    } else if url.ends_with("/wp-json/wp/v2") {
        url.to_string()
    } else {
        format!("{}/wp-json/wp/v2", url)
    };
    Ok(base)
}

fn fetch_and_append(
    api_base: &str,
    endpoint: &str,
    config: &Wp2mdConfig,
    vfs: &dyn crate::vfs::Wp2mdVfs,
    temp_path: &str,
    posts: &mut Vec<Post>,
) -> Result<()> {
    let per_page = config.wpjson_per_page;
    let mut page = 1u32;

    loop {
        let url = format!(
            "{}/{}?per_page={}&page={}&status=publish&_embed",
            api_base, endpoint, per_page, page
        );

        vfs.download_to_file(&url, temp_path, config.strict_ssl)
            .map_err(|e| miette!("Failed to fetch {}: {}", url, e))?;

        let content = vfs.read_to_string(temp_path)?;
        let items: Vec<WpApiPost> = serde_json::from_str(&content)
            .map_err(|e| miette!("Invalid JSON from WordPress API: {}", e))?;

        let count = items.len();
        if count == 0 {
            break;
        }

        for item in &items {
            posts.push(map_to_post(item, endpoint, config)?);
        }

        if (count as u32) < per_page {
            break;
        }
        page += 1;
    }

    Ok(())
}

fn map_to_post(
    api: &WpApiPost,
    post_type_hint: &str,
    config: &Wp2mdConfig,
) -> Result<Post> {
    let status = api.status.as_deref().unwrap_or("publish").to_string();
    let is_draft = status != "publish";

    let title = api
        .title
        .as_ref()
        .and_then(|t| t.rendered.as_deref())
        .unwrap_or_default()
        .to_string();

    let content_html = api
        .content
        .as_ref()
        .and_then(|c| c.rendered.as_deref())
        .unwrap_or_default()
        .to_string();

    let excerpt = api
        .excerpt
        .as_ref()
        .and_then(|e| e.rendered.as_deref())
        .filter(|s| !s.is_empty())
        .map(|s| s.replace('\n', " ").trim().to_string());

    let slug = api.slug.as_deref().unwrap_or_default().to_string();

    let date = parse_date(api.date.as_deref().or(api.date_gmt.as_deref()));

    let post_type = api
        .r#type
        .as_deref()
        .unwrap_or(post_type_hint)
        .to_string();

    let author = api
        ._embedded
        .as_ref()
        .and_then(|e| e.author.as_ref())
        .and_then(|a| a.first())
        .and_then(|u| u.name.as_ref())
        .map(|s| s.to_string());

    let (categories, tags) = extract_terms(api);

    let (cover_image, images) = collect_images(api, config);

    Ok(Post {
        id: api.id.unwrap_or(0),
        title,
        slug,
        date,
        post_type,
        status,
        author,
        excerpt,
        content_html,
        content_md: String::new(),
        categories,
        tags,
        cover_image,
        images,
        is_draft,
    })
}

fn parse_date(s: Option<&str>) -> Option<NaiveDateTime> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.naive_utc())
}

fn extract_terms(api: &WpApiPost) -> (Vec<String>, Vec<String>) {
    let mut categories = Vec::new();
    let mut tags = Vec::new();

    let wp_term = api
        ._embedded
        .as_ref()
        .and_then(|e| e.wp_term.as_ref());

    if let Some(term_arrays) = wp_term {
        for arr in term_arrays {
            for term in arr {
                let name = term.name.as_deref().unwrap_or("").trim();
                if name.is_empty() || name.eq_ignore_ascii_case("uncategorized") {
                    continue;
                }
                match term.taxonomy.as_deref() {
                    Some("category") => categories.push(name.to_string()),
                    Some("post_tag") => tags.push(name.to_string()),
                    _ => {}
                }
            }
        }
    }

    categories.dedup();
    tags.dedup();
    (categories, tags)
}

fn collect_images(api: &WpApiPost, config: &Wp2mdConfig) -> (Option<String>, Vec<ImageRef>) {
    let mut images = Vec::new();
    let mut cover_image: Option<String> = None;

    let post_id = api.id;

    // Featured media
    if config.save_images.includes_attached() {
        if let Some(media_arr) = api
            ._embedded
            .as_ref()
            .and_then(|e| e.wp_featuredmedia.as_ref())
        {
            if let Some(media) = media_arr.first() {
                if let Some(url) = media.source_url.as_deref() {
                    if is_image_url(url) {
                        let filename = common::filename_from_url(url);
                        cover_image = Some(filename.clone());
                        images.push(ImageRef {
                            url: url.to_string(),
                            filename,
                            post_id,
                            is_cover: true,
                        });
                    }
                }
            }
        }
    }

    // Scraped from content
    if config.save_images.includes_scraped() {
        let content = api
            .content
            .as_ref()
            .and_then(|c| c.rendered.as_deref())
            .unwrap_or("");
        let img_re = Regex::new(r#"<img[^>]+src=["']([^"']+)["']"#).unwrap();
        for cap in img_re.captures_iter(content) {
            let url = &cap[1];
            if !url.contains("://") || !is_image_url(url) {
                continue;
            }
            let filename = common::filename_from_url(url);
            let is_cover = cover_image.as_ref().map_or(false, |c| *c == filename);
            images.push(ImageRef {
                url: url.to_string(),
                filename,
                post_id,
                is_cover,
            });
        }
    }

    (cover_image, images)
}

fn is_image_url(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    let path = path.split('#').next().unwrap_or(path);
    if let Some(ext) = path.rsplit('.').next() {
        IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}

// ============================================================================
// API response types
// ============================================================================

#[derive(Debug, Deserialize)]
struct WpApiPost {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    date_gmt: Option<String>,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    title: Option<RenderedField>,
    #[serde(default)]
    content: Option<RenderedField>,
    #[serde(default)]
    excerpt: Option<RenderedField>,
    #[serde(default)]
    #[serde(rename = "_embedded")]
    _embedded: Option<WpEmbedded>,
}

#[derive(Debug, Deserialize)]
struct RenderedField {
    #[serde(default)]
    rendered: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WpEmbedded {
    #[serde(default)]
    author: Option<Vec<WpAuthor>>,
    #[serde(rename = "wp:term")]
    #[serde(default)]
    wp_term: Option<Vec<Vec<WpTerm>>>,
    #[serde(rename = "wp:featuredmedia")]
    #[serde(default)]
    wp_featuredmedia: Option<Vec<WpFeaturedMedia>>,
}

#[derive(Debug, Deserialize)]
struct WpAuthor {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WpTerm {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    taxonomy: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WpFeaturedMedia {
    #[serde(default)]
    source_url: Option<String>,
}
