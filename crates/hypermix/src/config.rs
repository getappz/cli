//! Load hypermix config from JSON or JSONC.

use std::path::{Path, PathBuf};

use miette::IntoDiagnostic;

use crate::HypermixConfig;

const CONFIG_NAMES: &[&str] = &["pack.config.json", "pack.config.jsonc", "hypermix.config.json", "hypermix.config.jsonc"];

/// Find and load hypermix/pack config. Searches cwd then --config path.
pub fn load_config(config_path: Option<&Path>, cwd: &Path) -> miette::Result<(PathBuf, HypermixConfig)> {
    let path = match config_path {
        Some(p) => p.to_path_buf(),
        None => {
            let found = CONFIG_NAMES
                .iter()
                .map(|n| cwd.join(n))
                .find(|p| p.exists())
                .ok_or_else(|| {
                    miette::miette!(
                        "No config file found. Expected one of: {}. Run `appz pack init` to create.",
                        CONFIG_NAMES.join(", ")
                    )
                })?;
            found
        }
    };

    let content = std::fs::read_to_string(&path).into_diagnostic()?;
    let config = if path.extension().map_or(false, |e| e == "jsonc") {
        let stripped = json_comments::StripComments::new(content.as_bytes());
        serde_json::from_reader(stripped).into_diagnostic()?
    } else {
        serde_json::from_str(&content).into_diagnostic()?
    };

    Ok((path, config))
}
