//! Quiet-aware UI helpers for the init crate.
//!
//! Mirrors the deployer crate's UX pattern: all status messages, section titles,
//! and progress indicators respect json_output and is_ci flags.

use crate::config::InitOptions;

/// Whether to suppress UI output.
fn should_suppress(options: &InitOptions) -> bool {
    options.json_output || options.is_ci
}

/// Emit an info message unless quiet.
pub fn info(options: &InitOptions, msg: &str) {
    if !should_suppress(options) {
        let _ = ui::status::info(msg);
    }
}

/// Emit a success message unless quiet.
pub fn success(options: &InitOptions, msg: &str) {
    if !should_suppress(options) {
        let _ = ui::status::success(msg);
    }
}

/// Emit a warning message unless quiet.
pub fn warning(options: &InitOptions, msg: &str) {
    if !should_suppress(options) {
        let _ = ui::status::warning(msg);
    }
}

/// Emit an error message unless quiet.
pub fn error(options: &InitOptions, msg: &str) {
    if !should_suppress(options) {
        let _ = ui::status::error(msg);
    }
}

/// Emit a blank line unless quiet.
pub fn blank_line(options: &InitOptions) {
    if !should_suppress(options) {
        let _ = ui::layout::blank_line();
    }
}

/// Emit a section title unless quiet.
pub fn section_title(options: &InitOptions, title: &str) {
    if !should_suppress(options) {
        let _ = ui::layout::section_title(title);
    }
}

/// Start a spinner with the given message unless quiet.
pub fn spinner(
    options: &InitOptions,
    message: &str,
) -> Option<ui::progress::SpinnerHandle> {
    if should_suppress(options) {
        None
    } else {
        Some(ui::progress::spinner(message))
    }
}
