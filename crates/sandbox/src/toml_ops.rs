//! TOML read / write helpers built on top of [`ScopedFs`].
//!
//! All file paths are resolved through [`ScopedFs`] so they inherit the same
//! path-safety guarantees as the rest of the sandbox.
//!
//! # Functions
//!
//! | Function | Description |
//! |----------|-------------|
//! | [`read_toml`] | Deserialise a TOML file into a typed `T` |
//! | [`read_toml_value`] | Read a TOML file into a generic `toml::Value` |
//! | [`write_toml`] | Serialise `T` to pretty-printed TOML and write it |
//! | [`write_toml_value`] | Write a `toml::Value` as pretty-printed TOML |
//!
//! # Examples
//!
//! ```rust,no_run
//! use sandbox::ScopedFs;
//! use sandbox::toml_ops::{read_toml, write_toml};
//!
//! #[derive(serde::Serialize, serde::Deserialize)]
//! struct SiteConfig { title: String, base_url: String }
//!
//! # fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let fs = ScopedFs::new("/tmp/project")?;
//!
//! let cfg = SiteConfig {
//!     title: "My Blog".into(),
//!     base_url: "https://example.com".into(),
//! };
//! write_toml(&fs, "config.toml", &cfg)?;
//!
//! let loaded: SiteConfig = read_toml(&fs, "config.toml")?;
//! assert_eq!(loaded.title, "My Blog");
//! # Ok(())
//! # }
//! ```

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{SandboxError, SandboxResult};
use crate::scoped_fs::ScopedFs;

/// Read a TOML file and deserialise it into `T`.
pub fn read_toml<T: DeserializeOwned>(
    fs: &ScopedFs,
    rel_path: impl AsRef<Path>,
) -> SandboxResult<T> {
    let content = fs.read_to_string(&rel_path)?;
    toml::from_str(&content).map_err(|e| SandboxError::TomlError {
        reason: format!(
            "Failed to parse {}: {}",
            rel_path.as_ref().display(),
            e
        ),
    })
}

/// Read a TOML file into a generic [`toml::Value`].
pub fn read_toml_value(
    fs: &ScopedFs,
    rel_path: impl AsRef<Path>,
) -> SandboxResult<toml::Value> {
    read_toml::<toml::Value>(fs, rel_path)
}

/// Serialise `value` as TOML and write it to a file.
pub fn write_toml<T: Serialize>(
    fs: &ScopedFs,
    rel_path: impl AsRef<Path>,
    value: &T,
) -> SandboxResult<()> {
    let content = toml::to_string_pretty(value).map_err(|e| SandboxError::TomlError {
        reason: e.to_string(),
    })?;
    fs.write_string(rel_path, &content)
}

/// Write a [`toml::Value`] as TOML.
pub fn write_toml_value(
    fs: &ScopedFs,
    rel_path: impl AsRef<Path>,
    value: &toml::Value,
) -> SandboxResult<()> {
    write_toml(fs, rel_path, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_toml() {
        let dir = tempfile::tempdir().unwrap();
        let fs = ScopedFs::new(dir.path()).unwrap();

        let mut table = toml::value::Table::new();
        table.insert("name".into(), toml::Value::String("test".into()));
        table.insert(
            "version".into(),
            toml::Value::String("0.1.0".into()),
        );
        let value = toml::Value::Table(table);

        write_toml_value(&fs, "config.toml", &value).unwrap();
        let loaded = read_toml_value(&fs, "config.toml").unwrap();
        assert_eq!(loaded["name"].as_str(), Some("test"));
        assert_eq!(loaded["version"].as_str(), Some("0.1.0"));
    }

    #[test]
    fn typed_toml_roundtrip() {
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Config {
            title: String,
            port: u16,
        }
        let dir = tempfile::tempdir().unwrap();
        let fs = ScopedFs::new(dir.path()).unwrap();
        let cfg = Config {
            title: "My Site".into(),
            port: 3000,
        };
        write_toml(&fs, "site.toml", &cfg).unwrap();
        let loaded: Config = read_toml(&fs, "site.toml").unwrap();
        assert_eq!(loaded, cfg);
    }
}
