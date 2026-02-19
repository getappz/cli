//! User-level configuration from `~/.appz/config.toml`.
//!
//! Provides a layered configuration system:
//!
//! 1. **Defaults** (lowest precedence)
//! 2. **User config** (`~/.appz/config.toml`)
//! 3. **Project config** (`appz.json`)
//! 4. **CLI flags** (highest precedence)
//!
//! The user config is TOML-based and supports the same fields as the
//! project-level check config, enabling users to set personal defaults
//! (preferred AI model, API keys, safety thresholds) across all projects.
//!
//! # Example `~/.appz/config.toml`
//!
//! ```toml
//! [check]
//! strict = true
//! aiProvider = "openai"
//! aiModel = "gpt-4o"
//!
//! [check.aiSafety]
//! max_change_pct = 30.0
//! max_attempts = 5
//! ```

use starbase_utils::{dirs, fs};
use std::path::{Path, PathBuf};

/// The directory name under the user's home.
const APPZ_DIR_NAME: &str = ".appz";
/// The config file name within the appz dir.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Resolve the path to `~/.appz/config.toml`.
///
/// Returns `None` if the home directory cannot be determined.
pub fn user_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(APPZ_DIR_NAME).join(CONFIG_FILE_NAME))
}

/// Resolve the path to `~/.appz/` directory.
pub fn user_appz_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(APPZ_DIR_NAME))
}

/// Read the user config file and return its raw TOML content as a
/// `serde_json::Value` (converted from TOML → JSON for compatibility
/// with the existing JSON-based config merging).
///
/// Returns `None` if the file does not exist.
pub fn read_user_config_raw() -> Option<serde_json::Value> {
    let path = user_config_path()?;
    if !path.exists() {
        return None;
    }

    let content = fs::read_file(&path).ok()?;
    let toml_value: toml::Value = toml::from_str(&content).ok()?;

    // Convert TOML → JSON value for uniform merging.
    Some(toml_to_json(&toml_value))
}

/// Read a specific section from the user config as a JSON value.
///
/// E.g., `read_user_config_section("check")` returns the `[check]` table.
pub fn read_user_config_section(section: &str) -> Option<serde_json::Value> {
    let config = read_user_config_raw()?;
    config.get(section).cloned()
}

/// Deep-merge two JSON values.
///
/// - Objects are recursively merged (keys in `overlay` override `base`).
/// - All other types: `overlay` wins.
/// - `None` / null values in `overlay` are skipped (do not delete base keys).
pub fn deep_merge_json(base: &serde_json::Value, overlay: &serde_json::Value) -> serde_json::Value {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            let mut merged = base_map.clone();
            for (key, overlay_val) in overlay_map {
                if overlay_val.is_null() {
                    continue; // Don't delete base keys with null.
                }
                let merged_val = match merged.get(key) {
                    Some(base_val) => deep_merge_json(base_val, overlay_val),
                    None => overlay_val.clone(),
                };
                merged.insert(key.clone(), merged_val);
            }
            serde_json::Value::Object(merged)
        }
        // For non-objects, overlay wins.
        (_, overlay) => overlay.clone(),
    }
}

/// Convert a TOML value to a JSON value.
pub fn toml_to_json(toml_val: &toml::Value) -> serde_json::Value {
    match toml_val {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::json!(*i),
        toml::Value::Float(f) => serde_json::json!(*f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(toml_to_json).collect())
        }
        toml::Value::Table(table) => {
            let map: serde_json::Map<String, serde_json::Value> = table
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

/// Check if the user config file exists.
pub fn user_config_exists() -> bool {
    user_config_path().map(|p| p.exists()).unwrap_or(false)
}

/// Ensure the `~/.appz/` directory exists.
pub fn ensure_user_appz_dir() -> std::io::Result<PathBuf> {
    let dir = user_appz_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Could not determine home directory")
    })?;
    fs::create_dir_all(&dir).map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(dir)
}

/// Resolve a file path relative to `~/.appz/`.
pub fn user_appz_file(relative: &str) -> Option<PathBuf> {
    user_appz_dir().map(|d| d.join(relative))
}

/// Return a path string suitable for user-facing display: paths under the user's home
/// are shown with a `~` prefix (e.g. `~/.appz/skills` instead of `/home/user/.appz/skills`).
pub fn path_for_display(path: &Path) -> String {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return path.display().to_string(),
    };
    match path.strip_prefix(&home) {
        Ok(suffix) => {
            let s = suffix.display().to_string();
            let s = s.replace('\\', "/");
            if s.is_empty() || s == "." {
                "~".to_string()
            } else {
                format!("~/{}", s.trim_start_matches('/'))
            }
        }
        Err(_) => path.display().to_string(),
    }
}

/// List files in a subdirectory of `~/.appz/`.
///
/// Returns an empty vec if the directory doesn't exist.
pub fn list_user_appz_dir(subdir: &str) -> Vec<PathBuf> {
    let dir = match user_appz_dir() {
        Some(d) => d.join(subdir),
        None => return Vec::new(),
    };
    if !dir.is_dir() {
        return Vec::new();
    }
    fs::read_dir(dir)
        .ok()
        .map(|entries| entries.into_iter().map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// Try to read a file from `~/.appz/{relative}`, falling back to
/// `{project_dir}/.appz/{relative}`, then to `None`.
pub fn read_layered_file(project_dir: &Path, relative: &str) -> Option<String> {
    // 1. Project-level.
    let project_file = project_dir.join(".appz").join(relative);
    if project_file.exists() {
        if let Ok(content) = fs::read_file(&project_file) {
            return Some(content);
        }
    }

    // 2. User-level.
    if let Some(user_file) = user_appz_file(relative) {
        if user_file.exists() {
            if let Ok(content) = fs::read_file(&user_file) {
                return Some(content);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_merge_objects() {
        let base = serde_json::json!({
            "a": 1,
            "b": {"c": 2, "d": 3},
            "e": "hello"
        });
        let overlay = serde_json::json!({
            "b": {"c": 99, "f": 100},
            "g": "new"
        });
        let merged = deep_merge_json(&base, &overlay);

        assert_eq!(merged["a"], 1);
        assert_eq!(merged["b"]["c"], 99);
        assert_eq!(merged["b"]["d"], 3);
        assert_eq!(merged["b"]["f"], 100);
        assert_eq!(merged["e"], "hello");
        assert_eq!(merged["g"], "new");
    }

    #[test]
    fn deep_merge_null_preserves_base() {
        let base = serde_json::json!({"a": 1, "b": 2});
        let overlay = serde_json::json!({"a": null, "b": 99});
        let merged = deep_merge_json(&base, &overlay);
        assert_eq!(merged["a"], 1); // null in overlay doesn't delete base
        assert_eq!(merged["b"], 99);
    }
}
