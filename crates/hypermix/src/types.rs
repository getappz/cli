//! Config types for hypermix.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixConfig {
    pub remote: Option<String>,
    pub include: Option<Vec<String>>,
    pub ignore: Option<Vec<String>>,
    pub output: Option<String>,
    #[serde(alias = "repomixConfig")]
    pub repomix_config: Option<String>,
    #[serde(alias = "extraFlags")]
    pub extra_flags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypermixConfig {
    pub mixes: Vec<MixConfig>,
    pub silent: Option<bool>,
    #[serde(alias = "outputPath")]
    pub output_path: Option<String>,
}
