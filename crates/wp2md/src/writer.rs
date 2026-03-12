//! File writer for Markdown posts and image downloads.
//!
//! Mirrors the reference `writer.js`:
//! - Write markdown files with frontmatter + content
//! - Download and save images
//! - Resume support: skip files that already exist

use crate::common;
use crate::config::{SaveImages, Wp2mdConfig};
use crate::frontmatter;
use crate::types::{ConvertResult, Post};
use crate::vfs::Wp2mdVfs;
use miette::{miette, Result};
use std::collections::HashSet;

/// Write all markdown files and download images for the given posts.
pub fn write_all(
    vfs: &dyn Wp2mdVfs,
    posts: &[Post],
    config: &Wp2mdConfig,
) -> Result<ConvertResult> {
    let mut result = ConvertResult::default();

    // Write markdown files
    write_markdown_files(vfs, posts, config, &mut result)?;

    // Download images
    if config.save_images != SaveImages::None {
        write_image_files(vfs, posts, config, &mut result)?;
    }

    Ok(result)
}

/// Write all markdown files for the posts.
fn write_markdown_files(
    vfs: &dyn Wp2mdVfs,
    posts: &[Post],
    config: &Wp2mdConfig,
    result: &mut ConvertResult,
) -> Result<()> {
    for post in posts {
        let dest = common::build_post_path(post, config);

        // Resume support: skip if file already exists
        if vfs.exists(&dest) {
            result.posts_skipped += 1;
            continue;
        }

        let content = build_markdown_content(post, config);

        vfs.write_string(&dest, &content)
            .map_err(|e| miette!("Failed to write {}: {}", dest, e))?;

        result.posts_written += 1;
    }

    Ok(())
}

/// Build the full markdown file content (frontmatter + body).
fn build_markdown_content(post: &Post, config: &Wp2mdConfig) -> String {
    let fm = frontmatter::build_frontmatter(post, config);
    if fm.is_empty() {
        post.content_md.clone()
    } else {
        format!("{}\n{}", fm, post.content_md)
    }
}

/// Download and save all images for the posts.
fn write_image_files(
    vfs: &dyn Wp2mdVfs,
    posts: &[Post],
    config: &Wp2mdConfig,
    result: &mut ConvertResult,
) -> Result<()> {
    // Collect all unique (url, dest_path) pairs
    let mut seen_urls: HashSet<String> = HashSet::new();
    let mut payloads: Vec<(String, String)> = Vec::new();

    for post in posts {
        let images_dir = common::build_images_path(post, config);

        for img in &post.images {
            if seen_urls.insert(img.url.clone()) {
                let dest = format!("{}/{}", images_dir, img.filename);
                payloads.push((img.url.clone(), dest));
            }
        }
    }

    for (url, dest) in &payloads {
        // Resume support: skip if file already exists
        if vfs.exists(dest) {
            result.images_skipped += 1;
            continue;
        }

        // Ensure parent directory exists
        if let Some(parent) = dest.rsplit_once('/').map(|(p, _)| p) {
            vfs.create_dir_all(parent)?;
        }

        match vfs.download_to_file(url, dest, config.strict_ssl) {
            Ok(()) => {
                result.images_downloaded += 1;
            }
            Err(e) => {
                // Log error but continue with remaining images
                eprintln!("Warning: failed to download {}: {}", url, e);
            }
        }

        // Delay between requests
        if config.request_delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(config.request_delay_ms));
        }
    }

    Ok(())
}
