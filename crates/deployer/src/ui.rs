//! Quiet-aware UI helpers for the deployer.
//!
//! Mirrors the sandbox crate's UX pattern: all status messages, section titles,
//! and progress indicators respect a `quiet` flag. When `quiet` is true (e.g.
//! `--json` output or CI mode), no terminal output is emitted so machine-readable
//! output stays clean.
//!
//! # Usage
//!
//! ```ignore
//! use crate::ui as deploy_ui;
//!
//! deploy_ui::info(&ctx, "Deploying to Vercel...");
//! let sp = deploy_ui::spinner(&ctx, "Uploading...");
//! // ... do work ...
//! if let Some(s) = sp {
//!     s.finish_with_message("Done!");
//! }
//! ```

use crate::config::DeployContext;

/// Whether to suppress UI output (status messages, spinners, section titles).
///
/// True when output is machine-readable (JSON) or in non-interactive CI mode.
fn should_suppress(ctx: &DeployContext) -> bool {
    ctx.json_output
}

/// Emit an info message unless quiet.
pub fn info(ctx: &DeployContext, msg: &str) {
    if !should_suppress(ctx) {
        let _ = ui::status::info(msg);
    }
}

/// Emit a success message unless quiet.
#[allow(dead_code)]
pub fn success(ctx: &DeployContext, msg: &str) {
    if !should_suppress(ctx) {
        let _ = ui::status::success(msg);
    }
}

/// Emit a warning message unless quiet.
#[allow(dead_code)]
pub fn warning(ctx: &DeployContext, msg: &str) {
    if !should_suppress(ctx) {
        let _ = ui::status::warning(msg);
    }
}

/// Emit an error message unless quiet.
#[allow(dead_code)]
pub fn error(ctx: &DeployContext, msg: &str) {
    if !should_suppress(ctx) {
        let _ = ui::status::error(msg);
    }
}

/// Emit a blank line unless quiet.
#[allow(dead_code)]
pub fn blank_line(ctx: &DeployContext) {
    if !should_suppress(ctx) {
        let _ = ui::layout::blank_line();
    }
}

/// Emit a section title unless quiet.
#[allow(dead_code)]
pub fn section_title(ctx: &DeployContext, title: &str) {
    if !should_suppress(ctx) {
        let _ = ui::layout::section_title(title);
    }
}

/// Start a spinner with the given message unless quiet.
///
/// Returns `Some(SpinnerHandle)` when UI is enabled; call `finish()` or
/// `finish_with_message()` when done. Returns `None` when quiet.
#[allow(dead_code)]
pub fn spinner(ctx: &DeployContext, message: &str) -> Option<ui::progress::SpinnerHandle> {
    if should_suppress(ctx) {
        None
    } else {
        Some(ui::progress::spinner(message))
    }
}
