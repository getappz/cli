//! Post collector and image extraction from parsed WXR data.
//!
//! Mirrors the logic from the reference `parser.js`:
//! - Detect and prioritize post types
//! - Build `Post` structs from XML items
//! - Collect attached and scraped images
//! - Merge images into their parent posts

use crate::common;
use crate::config::Wp2mdConfig;
use crate::types::{ImageRef, Post};
use crate::xml::{Item, Rss, EXCLUDED_POST_TYPES, IMAGE_EXTENSIONS};
use chrono::NaiveDateTime;
use miette::Result;
use regex::Regex;
use std::collections::{HashMap, HashSet};

/// Collect posts from parsed WXR data according to the configuration.
pub fn collect_posts(rss: &Rss, config: &Wp2mdConfig) -> Result<Vec<Post>> {
    let items = &rss.channel.items;

    let post_types = get_post_types(items);
    let mut posts = build_posts(items, &post_types);

    let mut images = Vec::new();
    if config.save_images.includes_attached() {
        images.extend(collect_attached_images(items));
    }
    if config.save_images.includes_scraped() {
        images.extend(collect_scraped_images(items, &post_types));
    }

    if !images.is_empty() {
        merge_images_into_posts(&images, &mut posts, items);
    }

    Ok(posts)
}

// ============================================================================
// Post type detection
// ============================================================================

/// Discover all post types, excluding internal WordPress types.
/// Returns them in priority order: "post", "page", then custom types alphabetically.
fn get_post_types(items: &[Item]) -> Vec<String> {
    let mut types: HashSet<String> = HashSet::new();
    for item in items {
        let pt = item.get_post_type();
        if !EXCLUDED_POST_TYPES.contains(&pt) && pt != "attachment" {
            types.insert(pt.to_string());
        }
    }

    let mut result: Vec<String> = Vec::new();

    // Prioritize "post" and "page" first
    if types.remove("post") {
        result.push("post".to_string());
    }
    if types.remove("page") {
        result.push("page".to_string());
    }

    // Remaining custom types sorted alphabetically
    let mut custom: Vec<String> = types.into_iter().collect();
    custom.sort();
    result.extend(custom);

    result
}

// ============================================================================
// Post building
// ============================================================================

/// Build `Post` structs from all items matching the discovered post types.
fn build_posts(items: &[Item], post_types: &[String]) -> Vec<Post> {
    let type_set: HashSet<&str> = post_types.iter().map(|s| s.as_str()).collect();

    items
        .iter()
        .filter(|item| type_set.contains(item.get_post_type()))
        .map(build_post)
        .collect()
}

/// Build a single `Post` from a WXR `Item`.
fn build_post(item: &Item) -> Post {
    let status = item.status.as_deref().unwrap_or("publish").to_string();
    let is_draft = status != "publish";

    let slug = item
        .post_name
        .as_deref()
        .map(|s| urlencoding::decode(s).unwrap_or_else(|_| s.into()).into_owned())
        .unwrap_or_default();

    let date = parse_post_date(item);

    let content_html = item.content_encoded.clone().unwrap_or_default();

    let excerpt = item
        .excerpt
        .as_ref()
        .and_then(|e| e.value())
        .map(|s| s.replace('\n', " ").trim().to_string())
        .filter(|s| !s.is_empty());

    let author = item.creator.clone();

    let (categories, tags) = extract_categories_and_tags(item);

    Post {
        id: item.get_post_id().unwrap_or(0),
        title: item.title.clone().unwrap_or_default(),
        slug,
        date,
        post_type: item.get_post_type().to_string(),
        status,
        author,
        excerpt,
        content_html,
        content_md: String::new(),
        categories,
        tags,
        cover_image: None,
        images: Vec::new(),
        is_draft,
    }
}

/// Parse the post date from `wp:post_date` or `wp:post_date_gmt`.
fn parse_post_date(item: &Item) -> Option<NaiveDateTime> {
    let date_str = item
        .post_date
        .as_deref()
        .or(item.post_date_gmt.as_deref())
        .filter(|s| !s.is_empty() && *s != "0000-00-00 00:00:00")?;

    NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S").ok()
}

/// Extract decoded category and tag names from the `<category>` elements.
fn extract_categories_and_tags(item: &Item) -> (Vec<String>, Vec<String>) {
    let mut categories = Vec::new();
    let mut tags = Vec::new();

    for cat in &item.categories {
        let name = cat
            .display_name
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }

        let decoded = html_entity_decode(&name);

        match cat.domain.as_deref() {
            Some("category") => {
                if decoded.to_lowercase() != "uncategorized" {
                    categories.push(decoded);
                }
            }
            Some("post_tag") => {
                tags.push(decoded);
            }
            _ => {}
        }
    }

    // Deduplicate while preserving order
    categories.dedup();
    tags.dedup();

    (categories, tags)
}

// ============================================================================
// Image collection
// ============================================================================

/// Collect images that are WordPress attachments (uploaded via Add Media).
fn collect_attached_images(items: &[Item]) -> Vec<ImageRef> {
    items
        .iter()
        .filter(|item| item.get_post_type() == "attachment")
        .filter_map(|item| {
            let url = item.attachment_url.as_deref()?;
            if !is_image_url(url) {
                return None;
            }

            let filename = common::filename_from_url(url);
            let post_id = item
                .post_parent
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .filter(|&id| id > 0);

            let is_cover = false; // will be determined during merge

            Some(ImageRef {
                url: url.to_string(),
                filename,
                post_id,
                is_cover,
            })
        })
        .collect()
}

/// Collect images scraped from `<img>` tags in post body content.
fn collect_scraped_images(items: &[Item], post_types: &[String]) -> Vec<ImageRef> {
    let type_set: HashSet<&str> = post_types.iter().map(|s| s.as_str()).collect();
    let img_regex = Regex::new(r#"<img[^>]+src=["']([^"']+)["']"#).unwrap();

    let mut images = Vec::new();

    for item in items {
        if !type_set.contains(item.get_post_type()) {
            continue;
        }

        let content = match &item.content_encoded {
            Some(c) => c,
            None => continue,
        };

        let post_id = item.get_post_id();

        for cap in img_regex.captures_iter(content) {
            let url = &cap[1];
            if !is_absolute_url(url) || !is_image_url(url) {
                continue;
            }

            let filename = common::filename_from_url(url);
            images.push(ImageRef {
                url: url.to_string(),
                filename,
                post_id,
                is_cover: false,
            });
        }
    }

    images
}

// ============================================================================
// Image merging
// ============================================================================

/// Merge collected images into their parent posts.
///
/// Also determines cover images from the `_thumbnail_id` postmeta.
fn merge_images_into_posts(images: &[ImageRef], posts: &mut [Post], items: &[Item]) {
    // Build a map of attachment ID -> image URL for cover image lookup
    let attachment_id_to_url: HashMap<i64, &str> = items
        .iter()
        .filter(|item| item.get_post_type() == "attachment")
        .filter_map(|item| {
            let id = item.get_post_id()?;
            let url = item.attachment_url.as_deref()?;
            Some((id, url))
        })
        .collect();

    // Build a map of post ID -> thumbnail attachment ID from postmeta
    let post_thumbnail_ids: HashMap<i64, i64> = items
        .iter()
        .filter_map(|item| {
            let post_id = item.get_post_id()?;
            let thumb_id = item
                .get_meta_value("_thumbnail_id")
                .and_then(|s| s.parse::<i64>().ok())?;
            Some((post_id, thumb_id))
        })
        .collect();

    for post in posts.iter_mut() {
        // Determine cover image
        if let Some(&thumb_id) = post_thumbnail_ids.get(&post.id) {
            if let Some(&cover_url) = attachment_id_to_url.get(&thumb_id) {
                post.cover_image = Some(common::filename_from_url(cover_url));
            }
        }

        // Collect images belonging to this post
        let mut post_images: Vec<ImageRef> = Vec::new();
        let mut seen_urls: HashSet<String> = HashSet::new();

        for img in images {
            if img.post_id == Some(post.id) && seen_urls.insert(img.url.clone()) {
                let mut img = img.clone();
                // Mark as cover if this is the featured image
                if let Some(ref cover) = post.cover_image {
                    if img.filename == *cover {
                        img.is_cover = true;
                    }
                }
                post_images.push(img);
            }
        }

        post.images = post_images;
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Check if a URL points to an image based on file extension.
fn is_image_url(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    let path = path.split('#').next().unwrap_or(path);
    if let Some(ext) = path.rsplit('.').next() {
        IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}

/// Check if a URL is absolute (has a protocol).
fn is_absolute_url(url: &str) -> bool {
    url.contains("://")
}

/// Basic HTML entity decoding for common entities.
fn html_entity_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
}
