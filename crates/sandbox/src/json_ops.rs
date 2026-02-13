//! JSON read / write / deep-merge helpers built on top of [`ScopedFs`].
//!
//! All file paths are resolved through [`ScopedFs`] so they inherit the same
//! path-safety guarantees as the rest of the sandbox. Uses
//! [`starbase_utils::json`] for file-level I/O following the codebase
//! convention.
//!
//! # Functions
//!
//! | Function | Description |
//! |----------|-------------|
//! | [`read_json`] | Deserialise a JSON file into a typed `T` |
//! | [`read_json_value`] | Read a JSON file into a generic `serde_json::Value` |
//! | [`write_json`] | Serialise `T` to pretty-printed JSON and write it |
//! | [`write_json_value`] | Write a `serde_json::Value` as pretty-printed JSON |
//! | [`merge_json`] | Deep-merge a patch `Value` into an existing JSON file |
//!
//! # Examples
//!
//! ```rust,no_run
//! use sandbox::ScopedFs;
//! use sandbox::json_ops::{read_json_value, write_json_value, merge_json};
//! use serde_json::json;
//!
//! # fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let fs = ScopedFs::new("/tmp/project")?;
//!
//! // Write a package.json
//! write_json_value(&fs, "package.json", &json!({
//!     "name": "my-site",
//!     "version": "1.0.0",
//!     "scripts": { "build": "hugo" }
//! }))?;
//!
//! // Deep-merge additional fields (adds keys, preserves existing)
//! merge_json(&fs, "package.json", &json!({
//!     "scripts": { "dev": "hugo server" },
//!     "license": "MIT"
//! }))?;
//!
//! // Read it back
//! let pkg = read_json_value(&fs, "package.json")?;
//! assert_eq!(pkg["scripts"]["build"], "hugo");
//! assert_eq!(pkg["scripts"]["dev"], "hugo server");
//! # Ok(())
//! # }
//! ```

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use starbase_utils::json as starbase_json;

use crate::error::{SandboxError, SandboxResult};
use crate::scoped_fs::ScopedFs;

/// Read a JSON file and deserialise it into `T`.
pub fn read_json<T: DeserializeOwned>(
    fs: &ScopedFs,
    rel_path: impl AsRef<Path>,
) -> SandboxResult<T> {
    let abs = fs.resolve(&rel_path)?;
    starbase_json::read_file(&abs).map_err(|e| SandboxError::JsonError {
        reason: format!(
            "Failed to read {}: {}",
            rel_path.as_ref().display(),
            e
        ),
    })
}

/// Read a JSON file into a generic [`serde_json::Value`].
pub fn read_json_value(
    fs: &ScopedFs,
    rel_path: impl AsRef<Path>,
) -> SandboxResult<Value> {
    read_json::<Value>(fs, rel_path)
}

/// Serialise `value` as pretty-printed JSON and write it to a file.
pub fn write_json<T: Serialize>(
    fs: &ScopedFs,
    rel_path: impl AsRef<Path>,
    value: &T,
) -> SandboxResult<()> {
    let abs = fs.resolve(&rel_path)?;
    // Ensure parent directory exists.
    if let Some(parent) = abs.parent() {
        starbase_utils::fs::create_dir_all(parent)?;
    }
    starbase_json::write_file(&abs, value, true).map_err(|e| SandboxError::JsonError {
        reason: e.to_string(),
    })
}

/// Write a [`serde_json::Value`] as pretty-printed JSON.
pub fn write_json_value(
    fs: &ScopedFs,
    rel_path: impl AsRef<Path>,
    value: &Value,
) -> SandboxResult<()> {
    write_json(fs, rel_path, value)
}

/// Deep-merge `patch` into the JSON file at `rel_path`.
///
/// - Object keys in `patch` are recursively merged into the existing object.
/// - Non-object values in `patch` replace the existing value.
/// - If the file doesn't exist, `patch` is written as the entire contents.
pub fn merge_json(
    fs: &ScopedFs,
    rel_path: impl AsRef<Path>,
    patch: &Value,
) -> SandboxResult<()> {
    let rel = rel_path.as_ref();
    let mut base = if fs.exists(rel) {
        read_json_value(fs, rel)?
    } else {
        Value::Object(serde_json::Map::new())
    };
    deep_merge(&mut base, patch);
    write_json_value(fs, rel, &base)
}

/// Recursive deep merge: values from `patch` are merged into `base`.
fn deep_merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (key, patch_val) in patch_map {
                let base_val = base_map
                    .entry(key.clone())
                    .or_insert(Value::Null);
                deep_merge(base_val, patch_val);
            }
        }
        (base, patch) => {
            *base = patch.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrip_json() {
        let dir = tempfile::tempdir().unwrap();
        let fs = ScopedFs::new(dir.path()).unwrap();
        let data = json!({"name": "test", "version": 1});
        write_json_value(&fs, "data.json", &data).unwrap();
        let loaded = read_json_value(&fs, "data.json").unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn merge_json_objects() {
        let dir = tempfile::tempdir().unwrap();
        let fs = ScopedFs::new(dir.path()).unwrap();
        let initial = json!({"a": 1, "b": {"x": 10}});
        write_json_value(&fs, "cfg.json", &initial).unwrap();

        let patch = json!({"b": {"y": 20}, "c": 3});
        merge_json(&fs, "cfg.json", &patch).unwrap();

        let result = read_json_value(&fs, "cfg.json").unwrap();
        assert_eq!(result["a"], 1);
        assert_eq!(result["b"]["x"], 10);
        assert_eq!(result["b"]["y"], 20);
        assert_eq!(result["c"], 3);
    }

    #[test]
    fn merge_creates_file_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let fs = ScopedFs::new(dir.path()).unwrap();
        let patch = json!({"key": "value"});
        merge_json(&fs, "new.json", &patch).unwrap();
        let loaded = read_json_value(&fs, "new.json").unwrap();
        assert_eq!(loaded["key"], "value");
    }

    #[test]
    fn typed_json_roundtrip() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Pkg {
            name: String,
            version: String,
        }
        let dir = tempfile::tempdir().unwrap();
        let fs = ScopedFs::new(dir.path()).unwrap();
        let pkg = Pkg {
            name: "my-app".into(),
            version: "1.0.0".into(),
        };
        write_json(&fs, "package.json", &pkg).unwrap();
        let loaded: Pkg = read_json(&fs, "package.json").unwrap();
        assert_eq!(loaded, pkg);
    }
}
