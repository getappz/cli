//! Stub implementation for self-upgrade when feature is disabled.
//!
//! This module provides a minimal implementation that always returns false
//! for `is_available()` and provides stub functions for upgrade instructions.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct SelfUpdate {}

impl SelfUpdate {
    pub fn is_available() -> bool {
        false
    }
}

#[derive(Debug, Default, serde::Deserialize)]
struct InstructionsToml {
    message: Option<String>,
    #[serde(flatten)]
    commands: BTreeMap<String, String>,
}

fn read_instructions_file(path: &PathBuf) -> Option<String> {
    let parsed: InstructionsToml = starbase_utils::json::read_file(path).ok()?;
    if let Some(msg) = parsed.message {
        return Some(msg);
    }
    if let Some((_k, v)) = parsed.commands.into_iter().next() {
        return Some(v);
    }
    None
}

pub fn upgrade_instructions_text() -> Option<String> {
    if let Ok(path) = std::env::var("APPZ_SELF_UPDATE_INSTRUCTIONS") {
        let path = PathBuf::from(path);
        if let Some(msg) = read_instructions_file(&path) {
            return Some(msg);
        }
    }
    None
}

pub fn append_self_update_instructions(mut message: String) -> String {
    if let Some(instructions) = upgrade_instructions_text() {
        message.push('\n');
        message.push_str(&instructions);
    }
    message
}
