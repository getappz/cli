pub mod error;
pub mod executor;
pub mod generator;
pub mod runtime;
pub mod runtimes;
pub mod static_export;
pub mod types;

pub use error::BlueprintError;
pub use executor::BlueprintExecutor;
pub use generator::BlueprintGenerator;
pub use runtime::{RuntimeError, WordPressRuntime};
pub use runtimes::{DdevRuntime, PlaygroundRuntime};
pub use static_export::{ExportResult, StaticExporter};
pub use site2static::ProgressEvent;
pub use types::Blueprint;

use std::path::Path;

/// Load and parse a blueprint.json file.
pub fn load(path: &Path) -> Result<Blueprint, BlueprintError> {
    let raw = std::fs::read_to_string(path).map_err(|e| BlueprintError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    serde_json::from_str(&raw).map_err(|e| BlueprintError::Parse(e.to_string()))
}
