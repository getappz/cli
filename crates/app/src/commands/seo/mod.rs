//! SEO command module - audit and fix SEO issues

use crate::session::AppzSession;
use clap::Subcommand;
use starbase::AppResult;
use std::path::PathBuf;

pub mod audit;
pub mod fix;

pub use audit::seo_audit;
pub use fix::seo_fix;

#[derive(Subcommand, Debug, Clone)]
pub enum SeoCommands {
    /// Audit SEO of HTML files in the build output directory
    Audit {
        /// Directory to audit (default: detect from framework)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Show detailed issues for each individual page
        #[arg(long)]
        verbose: bool,
        /// Automatically fix detected SEO issues
        #[arg(long)]
        fix: bool,
    },
    /// Fix SEO issues with preview and control
    Fix {
        /// Directory to fix (default: detect from framework)
        #[arg(short, long)]
        dir: Option<PathBuf>,
        /// Preview fixes without applying them
        #[arg(long)]
        preview: bool,
        /// Apply fixes (requires explicit flag)
        #[arg(long)]
        apply: bool,
        /// Output format: json for CI/automation
        #[arg(long)]
        json: bool,
        /// Scope override: template, section:PREFIX, or page
        #[arg(long)]
        scope: Option<String>,
        /// Skip specific issue codes (comma-separated)
        #[arg(long)]
        skip: Option<String>,
    },
}

pub async fn run(session: AppzSession, command: SeoCommands) -> AppResult {
    match command {
        SeoCommands::Audit { dir, verbose, fix } => {
            seo_audit(session, dir, verbose, fix).await
        }
        SeoCommands::Fix { dir, preview, apply, json, scope, skip } => {
            seo_fix(session, dir, preview, apply, json, scope, skip).await
        }
    }
}

