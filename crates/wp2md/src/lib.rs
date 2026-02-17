//! # wp2md — WordPress Export to Markdown
//!
//! Converts a WordPress WXR (WordPress eXtended RSS) export XML file
//! into Markdown files with YAML frontmatter and downloaded images.
//!
//! This is a library crate with no I/O dependencies in its default
//! configuration. All filesystem and network operations go through
//! the [`Wp2mdVfs`] trait, enabling both native and WASM execution.
//!
//! ## Quick start (native)
//!
//! ```rust,no_run
//! use wp2md::{convert_export, config::Wp2mdConfig, vfs_native::NativeFs};
//!
//! let config = Wp2mdConfig {
//!     input: "export.xml".to_string(),
//!     output: "output".to_string(),
//!     ..Default::default()
//! };
//!
//! let result = convert_export(&NativeFs, &config).unwrap();
//! println!("Wrote {} posts, downloaded {} images",
//!     result.posts_written, result.images_downloaded);
//! ```

pub mod common;
pub mod config;
pub mod frontmatter;
pub mod parser;
pub mod translator;
pub mod types;
pub mod vfs;
#[cfg(feature = "native")]
pub mod vfs_native;
pub mod wpjson;
pub mod xml;
mod writer;

use config::Wp2mdConfig;
use miette::Result;
use types::ConvertResult;
use vfs::Wp2mdVfs;

/// Convert a WordPress WXR export or wp-json API site to Markdown files.
///
/// Input can be either:
/// - A file path to a WXR (XML) export
/// - A WordPress site URL (e.g. `https://mysite.com`) to fetch via REST API
pub fn convert_export(vfs: &dyn Wp2mdVfs, config: &Wp2mdConfig) -> Result<ConvertResult> {
    let mut posts = if is_wpjson_url(&config.input) {
        vfs.create_dir_all(&config.output)?;
        wpjson::fetch_posts_from_wpjson(vfs, &config.input, config)?
    } else {
        let xml_content = vfs.read_to_string(&config.input)?;
        let rss = xml::parse_wxr(&xml_content)?;
        parser::collect_posts(&rss, config)?
    };

    translator::translate_posts(&mut posts);
    writer::write_all(vfs, &posts, config)
}

fn is_wpjson_url(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}
