//! Static site export for CMS projects.
//!
//! Uses site2static to crawl a running local dev server and produce
//! a static HTML export suitable for deployment.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::runtime::{RuntimeError, WordPressRuntime};

/// Default output directory name for static exports.
const DEFAULT_OUTPUT_DIR: &str = "dist";

/// Result of a static site export.
pub struct ExportResult {
    pub output_dir: PathBuf,
    pub pages_crawled: u64,
    pub assets_copied: u64,
    pub duration: std::time::Duration,
}

/// Exports a CMS site as static HTML using site2static.
pub struct StaticExporter {
    project_path: PathBuf,
    runtime: Arc<dyn WordPressRuntime>,
}

impl StaticExporter {
    pub fn new(project_path: PathBuf, runtime: Arc<dyn WordPressRuntime>) -> Self {
        Self { project_path, runtime }
    }

    /// Run the full static export pipeline.
    ///
    /// 1. Resolve the site URL and webroot from the runtime
    /// 2. Run site2static to crawl and mirror the site
    ///
    /// Returns the output path and export stats.
    pub fn export(
        &self,
        output_dir: Option<&Path>,
        on_progress: Option<Arc<dyn Fn(site2static::ProgressEvent) + Send + Sync>>,
    ) -> Result<ExportResult, RuntimeError> {
        let host_output = output_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| self.project_path.join(DEFAULT_OUTPUT_DIR));

        let origin = self.runtime.site_url(&self.project_path);
        let webroot = self.resolve_webroot()?;

        let origin_url = url::Url::parse(&origin).map_err(|e| RuntimeError::CommandFailed {
            command: "site2static".into(),
            message: format!("invalid origin URL: {e}"),
        })?;

        let copy_dirs = Self::default_copy_dirs(&webroot);
        let config = site2static::MirrorConfig {
            origin: origin_url,
            webroot: site2static::WebRoot::Direct(webroot),
            output: host_output.clone(),
            workers: 8,
            depth: None,
            force: false,
            exclude_patterns: vec![],
            include_patterns: vec![],
            copy_dirs,
            on_progress,
        };

        let mirror = site2static::SiteMirror::new(config);
        let result = mirror.run().map_err(|e| RuntimeError::CommandFailed {
            command: "site2static".into(),
            message: e.to_string(),
        })?;

        Ok(ExportResult {
            output_dir: host_output,
            pages_crawled: result.pages_crawled,
            assets_copied: result.assets_copied,
            duration: result.duration,
        })
    }

    fn resolve_webroot(&self) -> Result<PathBuf, RuntimeError> {
        Ok(self.project_path.clone())
    }

    /// Detect directories that need full copying (JS-dynamically-loaded assets).
    fn default_copy_dirs(webroot: &Path) -> Vec<String> {
        let mut dirs = Vec::new();

        // Elementor: webpack chunks, conditional CSS, dialog/lightbox libs
        if webroot.join("wp-content/plugins/elementor/assets").is_dir() {
            dirs.push("wp-content/plugins/elementor/assets".into());
        }

        // WordPress core: emoji, mediaelement, etc.
        if webroot.join("wp-includes/js").is_dir() {
            dirs.push("wp-includes/js".into());
        }

        dirs
    }
}
