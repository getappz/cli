use serde::{Deserialize, Serialize};

/// Represents a detector condition for framework detection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Detector {
    /// Detects by matching a package name in package.json
    MatchPackage {
        #[serde(rename = "matchPackage")]
        match_package: String,
    },
    /// Detects by matching a package name in composer.json
    MatchComposerPackage {
        #[serde(rename = "matchComposerPackage")]
        match_composer_package: String,
    },
    /// Detects by checking if a path exists
    Path { path: String },
    /// Detects by matching content in a file
    MatchContent {
        path: String,
        #[serde(rename = "matchContent")]
        match_content: String,
    },
}

// Runtime-optimized version using &'static str
#[derive(Debug, Clone)]
pub enum DetectorStatic {
    MatchPackage {
        match_package: &'static str,
    },
    MatchComposerPackage {
        match_composer_package: &'static str,
    },
    Path {
        path: &'static str,
    },
    MatchContent {
        path: &'static str,
        match_content: &'static str,
    },
}

/// Framework detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detectors {
    pub every: Option<Vec<Detector>>,
    pub some: Option<Vec<Detector>>,
}

// Runtime-optimized version
#[derive(Debug, Clone)]
pub struct DetectorsStatic {
    pub every: Option<&'static [DetectorStatic]>,
    pub some: Option<&'static [DetectorStatic]>,
}

/// Runtime configuration for a framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseRuntime {
    pub src: String,
    #[serde(rename = "use")]
    pub r#use: String,
}

// Runtime-optimized version
#[derive(Debug, Clone)]
pub struct UseRuntimeStatic {
    pub src: &'static str,
    pub r#use: &'static str,
}

/// Command setting with optional placeholder and value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSetting {
    pub placeholder: Option<String>,
    pub value: Option<String>,
}

// Runtime-optimized version
#[derive(Debug, Clone)]
pub struct CommandSettingStatic {
    pub placeholder: Option<&'static str>,
    pub value: Option<&'static str>,
}

/// Framework settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(rename = "installCommand")]
    pub install_command: Option<CommandSetting>,
    #[serde(rename = "buildCommand")]
    pub build_command: Option<CommandSetting>,
    #[serde(rename = "devCommand")]
    pub dev_command: Option<CommandSetting>,
    #[serde(rename = "outputDirectory")]
    pub output_directory: Option<CommandSetting>,
}

// Runtime-optimized version
#[derive(Debug, Clone)]
pub struct SettingsStatic {
    pub install_command: Option<CommandSettingStatic>,
    pub build_command: Option<CommandSettingStatic>,
    pub dev_command: Option<CommandSettingStatic>,
    pub output_directory: Option<CommandSettingStatic>,
}

/// Recommended integration for a framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedIntegration {
    pub id: String,
    pub dependencies: Vec<String>,
}

// Runtime-optimized version
#[derive(Debug, Clone)]
pub struct RecommendedIntegrationStatic {
    pub id: &'static str,
    pub dependencies: &'static [&'static str],
}

/// Route configuration for default routes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub src: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "continue")]
    pub continue_: Option<bool>,
    pub handle: Option<String>,
    pub status: Option<u16>,
    pub dest: Option<String>,
}

/// Main Framework structure - runtime optimized with &'static str
#[derive(Debug, Clone)]
pub struct Framework {
    pub name: &'static str,
    pub slug: Option<&'static str>,
    pub demo: Option<&'static str>,
    pub logo: Option<&'static str>,
    pub dark_mode_logo: Option<&'static str>,
    pub screenshot: Option<&'static str>,
    pub tagline: Option<&'static str>,
    pub description: Option<&'static str>,
    pub website: Option<&'static str>,
    pub sort: Option<u32>,
    pub env_prefix: Option<&'static str>,
    pub use_runtime: Option<UseRuntimeStatic>,
    pub ignore_runtimes: Option<&'static [&'static str]>,
    pub detectors: Option<DetectorsStatic>,
    pub settings: Option<SettingsStatic>,
    pub recommended_integrations: Option<&'static [RecommendedIntegrationStatic]>,
    pub dependency: Option<&'static str>,
    pub supersedes: Option<&'static [&'static str]>,
    pub cache_pattern: Option<&'static str>,
}
