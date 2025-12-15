use crate::detectors::filesystem::DetectorFilesystem;
use frameworks::types::{DetectorStatic, Framework};
use regex::Regex;
use std::sync::Arc;

/// Package manager detection result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageManagerInfo {
    pub manager: String,
    pub version: Option<String>,
    /// Which lockfile was detected (if detected via lockfile)
    pub detected_lockfile: Option<String>,
    /// Dev script from package.json scripts.dev field
    pub dev_script: Option<String>,
    /// Install script from package.json scripts.install field
    pub install_script: Option<String>,
    /// Build script from package.json scripts.build field
    pub build_script: Option<String>,
}

/// Result of framework detection with optional version and package manager
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub framework: Framework,
    pub detected_version: Option<String>,
    pub package_manager: Option<PackageManagerInfo>,
}

/// Options for detecting a framework
pub struct DetectFrameworkOptions {
    pub fs: Arc<dyn DetectorFilesystem>,
    pub framework_list: Vec<Framework>,
}

/// Options for detecting frameworks with full record
pub struct DetectFrameworkRecordOptions {
    pub fs: Arc<dyn DetectorFilesystem>,
    pub framework_list: Vec<Framework>,
}

/// Check if a single detector item matches
/// Returns version string and optionally package manager info (when reading package.json)
async fn check_detector_item(
    fs: &Arc<dyn DetectorFilesystem>,
    detector: &DetectorStatic,
    _framework_slug: &str,
) -> Result<Option<(String, Option<PackageManagerInfo>)>, String> {
    match detector {
        DetectorStatic::MatchPackage { match_package } => {
            let path = "package.json";

            if !fs.is_file(path).await {
                return Ok(None);
            }

            let content = fs
                .read_file(path)
                .await
                .map_err(|e| format!("Failed to read {}: {}", path, e))?;

            // Parse JSON to extract packageManager field and scripts
            let json: Option<serde_json::Value> =
                serde_json::from_str::<serde_json::Value>(&content).ok();

            // Extract scripts.dev, scripts.install, and scripts.build
            let scripts = json.as_ref().and_then(|j| j.get("scripts"));
            let dev_script = scripts
                .and_then(|s| s.get("dev"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.trim().is_empty()); // Treat empty strings as missing
            let install_script = scripts
                .and_then(|s| s.get("install"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.trim().is_empty());
            let build_script = scripts
                .and_then(|s| s.get("build"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.trim().is_empty());

            // Extract packageManager field
            let package_manager = json
                .as_ref()
                .and_then(|j| j.get("packageManager"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .and_then(|pm_str| {
                    // Parse format: "npm@9.0.0" or "pnpm@9.0.0+hash" or just "npm"
                    let parts: Vec<&str> = pm_str.split('@').collect();
                    if parts.is_empty() {
                        return None;
                    }

                    let manager = parts[0].to_string().to_lowercase();

                    // Extract version if present
                    let version = if parts.len() > 1 {
                        let version_str = parts[1];
                        // Remove corepack hash if present (format: "9.0.0+hash")
                        let version_clean = version_str.split('+').next().unwrap_or(version_str);
                        Some(version_clean.to_string())
                    } else {
                        None
                    };

                    Some(PackageManagerInfo {
                        manager,
                        version,
                        detected_lockfile: None,
                        dev_script: dev_script.clone(),
                        install_script: install_script.clone(),
                        build_script: build_script.clone(),
                    })
                })
                .or_else(|| {
                    // If no packageManager field but we have scripts, create PackageManagerInfo anyway
                    // The manager will be detected later from lockfiles
                    if dev_script.is_some() || install_script.is_some() || build_script.is_some() {
                        Some(PackageManagerInfo {
                            manager: String::new(), // Will be detected from lockfiles later
                            version: None,
                            detected_lockfile: None,
                            dev_script,
                            install_script,
                            build_script,
                        })
                    } else {
                        None
                    }
                });

            // Continue with regex matching for framework detection
            let pattern = format!(
                r#"(dev)?(d|D)ependencies"\s*:\s*\{{[^}}]*"{}"\s*:\s*"(.+?)"[^}}]*\}}"#,
                regex::escape(match_package)
            );

            let regex =
                Regex::new(&pattern).map_err(|e| format!("Invalid regex pattern: {}", e))?;

            Ok(regex
                .captures(&content)
                .and_then(|c| c.get(3))
                .map(|m| (m.as_str().to_string(), package_manager)))
        }

        DetectorStatic::Path { path } => {
            if fs.has_path(path).await {
                Ok(Some((String::new(), None)))
            } else {
                Ok(None)
            }
        }

        DetectorStatic::MatchContent {
            path,
            match_content,
        } => {
            if !fs.is_file(path).await {
                return Ok(None);
            }

            let content = fs
                .read_file(path)
                .await
                .map_err(|e| format!("Failed to read {}: {}", path, e))?;

            let regex = Regex::new(match_content)
                .map_err(|e| format!("Invalid regex pattern in matchContent: {}", e))?;

            Ok(regex.is_match(&content).then_some((String::new(), None)))
        }
        DetectorStatic::MatchComposerPackage {
            match_composer_package,
        } => {
            let path = "composer.json";

            if !fs.is_file(path).await {
                return Ok(None);
            }

            let content = fs
                .read_file(path)
                .await
                .map_err(|e| format!("Failed to read {}: {}", path, e))?;

            let json: Option<serde_json::Value> =
                serde_json::from_str::<serde_json::Value>(&content).ok();

            let has_package = json
                .as_ref()
                .and_then(|j| j.get("require"))
                .and_then(|r| r.as_object())
                .map(|req| {
                    let key: &str = match_composer_package;
                    req.get(key).is_some()
                })
                .unwrap_or(false);

            Ok(has_package.then_some((String::new(), None)))
        }
    }
}

/// Check if a framework matches based on its detectors
async fn matches(
    fs: &Arc<dyn DetectorFilesystem>,
    framework: &Framework,
) -> Result<Option<MatchResult>, String> {
    let detectors = match &framework.detectors {
        Some(d) => d,
        None => return Ok(None),
    };

    let mut detected_version: Option<String> = None;
    let mut detected_package_manager: Option<PackageManagerInfo> = None;

    // ✅ "every" detectors
    if let Some(eds) = detectors.every {
        for det in eds {
            match check_detector_item(fs, det, framework.slug.unwrap_or("")).await? {
                Some((ver, pm_info)) => {
                    if !ver.is_empty() {
                        detected_version = Some(ver);
                    }
                    // Store package manager info if detected (from MatchPackage detector)
                    if let Some(pm) = pm_info {
                        detected_package_manager = Some(pm);
                    }
                }
                None => return Ok(None), // fails "every"
            }
        }

        return Ok(Some(MatchResult {
            framework: framework.clone(),
            detected_version,
            package_manager: detected_package_manager,
        }));
    }

    // ✅ "some" detectors - break immediately on first match (matches Vercel behavior)
    if let Some(sds) = detectors.some {
        for det in sds {
            if let Some((ver, pm_info)) =
                check_detector_item(fs, det, framework.slug.unwrap_or("")).await?
            {
                // Store version if non-empty (from matchPackage detector)
                if !ver.is_empty() {
                    detected_version = Some(ver);
                }
                // Store package manager info if detected
                if let Some(pm) = pm_info {
                    detected_package_manager = Some(pm);
                }
                // Return immediately on first match (Vercel breaks here)
                return Ok(Some(MatchResult {
                    framework: framework.clone(),
                    detected_version,
                    package_manager: detected_package_manager,
                }));
            }
        }

        return Ok(None);
    }

    Ok(None)
}

/// Remove superseded frameworks
fn remove_superseded_frameworks(frameworks: &mut Vec<Option<Framework>>) {
    let mut to_remove = std::collections::HashSet::<&'static str>::new();

    for fw in frameworks.iter().flatten() {
        if let Some(list) = fw.supersedes {
            for &slug in list {
                to_remove.insert(slug);
            }
        }
    }

    frameworks.retain(|fw| {
        fw.as_ref()
            .and_then(|f| f.slug.map(|s| !to_remove.contains(s)))
            .unwrap_or(true)
    });
}

/// Legacy, returns only slug
pub async fn detect_framework(options: DetectFrameworkOptions) -> Result<Option<String>, String> {
    let mut result = futures::future::join_all(options.framework_list.iter().map(|fw| {
        let fs = Arc::clone(&options.fs);
        let fwc = fw.clone();

        async move { matches(&fs, &fwc).await.ok().flatten().map(|r| r.framework) }
    }))
    .await;

    remove_superseded_frameworks(&mut result);

    Ok(result
        .into_iter()
        .flatten()
        .find_map(|fw| fw.slug.map(str::to_string)))
}

/// Return all matched frameworks
pub async fn detect_frameworks(
    options: DetectFrameworkRecordOptions,
) -> Result<Vec<Framework>, String> {
    let mut result = futures::future::join_all(options.framework_list.iter().map(|fw| {
        let fs = Arc::clone(&options.fs);
        let fwc = fw.clone();

        async move { matches(&fs, &fwc).await.ok().flatten().map(|r| r.framework) }
    }))
    .await;

    remove_superseded_frameworks(&mut result);

    Ok(result.into_iter().flatten().collect())
}

/// Detect a single framework with version and package manager
pub async fn detect_framework_record(
    options: DetectFrameworkRecordOptions,
) -> Result<Option<(Framework, Option<String>, Option<PackageManagerInfo>)>, String> {
    let detections: Vec<Option<MatchResult>> =
        futures::future::join_all(options.framework_list.iter().map(|fw| {
            let fs = Arc::clone(&options.fs);
            let fwc = fw.clone();

            async move { matches(&fs, &fwc).await.ok().flatten() }
        }))
        .await;

    // Framework list without superseded items
    let mut frameworks: Vec<Option<Framework>> = detections
        .iter()
        .map(|r| r.as_ref().map(|mr| mr.framework.clone()))
        .collect();

    remove_superseded_frameworks(&mut frameworks);

    for (idx, fw_opt) in frameworks.iter().enumerate() {
        if let Some(fw) = fw_opt {
            let version = detections[idx]
                .as_ref()
                .and_then(|mr| mr.detected_version.clone());
            let mut package_manager = detections[idx]
                .as_ref()
                .and_then(|mr| mr.package_manager.clone());

            // If package_manager has empty manager but has dev_script, merge with lockfile detection
            if let Some(ref mut pm) = package_manager {
                if pm.manager.is_empty() && pm.dev_script.is_some() {
                    // Try to detect package manager from lockfiles and merge
                    let lockfile_detection =
                        detect_from_lockfiles(&options.fs).await.ok().flatten();
                    if let Some(lockfile_pm) = lockfile_detection {
                        pm.manager = lockfile_pm.manager;
                        pm.version = lockfile_pm.version;
                        pm.detected_lockfile = lockfile_pm.detected_lockfile;
                        // Keep existing dev_script
                    }
                }
            } else {
                // No package_manager detected from framework detection, try full detection
                package_manager = detect_package_manager(&options.fs).await.ok().flatten();
            }

            return Ok(Some((fw.clone(), version, package_manager)));
        }
    }

    Ok(None)
}

/// Detect package manager from lockfiles
/// Based on Vercel's scanParentDirs lockfile detection logic
async fn detect_from_lockfiles(
    fs: &Arc<dyn DetectorFilesystem>,
) -> Result<Option<PackageManagerInfo>, String> {
    // Check for lockfiles in order of specificity
    // bun.lockb (binary) or bun.lock (text)
    let has_bun_lockb = fs.has_path("bun.lockb").await;
    let has_bun_lock = fs.has_path("bun.lock").await;
    if has_bun_lockb || has_bun_lock {
        return Ok(Some(PackageManagerInfo {
            manager: "bun".to_string(),
            version: None,
            detected_lockfile: Some(if has_bun_lockb {
                "bun.lockb".to_string()
            } else {
                "bun.lock".to_string()
            }),
            dev_script: None,
            install_script: None,
            build_script: None,
        }));
    }

    // yarn.lock
    if fs.has_path("yarn.lock").await {
        return Ok(Some(PackageManagerInfo {
            manager: "yarn".to_string(),
            version: None,
            detected_lockfile: Some("yarn.lock".to_string()),
            dev_script: None,
            install_script: None,
            build_script: None,
        }));
    }

    // pnpm-lock.yaml
    if fs.has_path("pnpm-lock.yaml").await {
        return Ok(Some(PackageManagerInfo {
            manager: "pnpm".to_string(),
            version: None,
            detected_lockfile: Some("pnpm-lock.yaml".to_string()),
            dev_script: None,
            install_script: None,
            build_script: None,
        }));
    }

    // package-lock.json (npm)
    if fs.has_path("package-lock.json").await {
        return Ok(Some(PackageManagerInfo {
            manager: "npm".to_string(),
            version: None,
            detected_lockfile: Some("package-lock.json".to_string()),
            dev_script: None,
            install_script: None,
            build_script: None,
        }));
    }

    Ok(None)
}

/// Detect package manager from package.json's `packageManager` field
async fn detect_from_package_json(
    fs: &Arc<dyn DetectorFilesystem>,
) -> Result<Option<PackageManagerInfo>, String> {
    let path = "package.json";

    if !fs.is_file(path).await {
        return Ok(None);
    }

    let content = fs
        .read_file(path)
        .await
        .map_err(|e| format!("Failed to read {}: {}", path, e))?;

    // Parse JSON
    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse {}: {}", path, e))?;

    // Extract scripts.dev, scripts.install, and scripts.build
    let scripts = json.get("scripts");
    let dev_script = scripts
        .and_then(|s| s.get("dev"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty()); // Treat empty strings as missing
    let install_script = scripts
        .and_then(|s| s.get("install"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());
    let build_script = scripts
        .and_then(|s| s.get("build"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());

    // Extract packageManager field
    let package_manager = json
        .get("packageManager")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if let Some(pm_str) = package_manager {
        // Parse format: "npm@9.0.0" or "pnpm@9.0.0+hash" or just "npm"
        let parts: Vec<&str> = pm_str.split('@').collect();

        if parts.is_empty() {
            return Ok(None);
        }

        let manager = parts[0].to_string().to_lowercase();

        // Extract version if present
        let version = if parts.len() > 1 {
            let version_str = parts[1];
            // Remove corepack hash if present (format: "9.0.0+hash")
            let version_clean = version_str.split('+').next().unwrap_or(version_str);
            Some(version_clean.to_string())
        } else {
            None
        };

        // Return the package manager info (packageManager field doesn't specify lockfile)
        Ok(Some(PackageManagerInfo {
            manager,
            version,
            detected_lockfile: None,
            dev_script,
            install_script,
            build_script,
        }))
    } else {
        Ok(None)
    }
}

/// Detect package manager from lockfiles and package.json's `packageManager` field.
///
/// This follows Vercel's scanParentDirs logic:
/// 1. First checks for lockfiles (yarn.lock, package-lock.json, pnpm-lock.yaml, bun.lockb/bun.lock)
/// 2. Then checks package.json's `packageManager` field (takes precedence if present)
///
/// The `packageManager` field follows the format: `"packageManager": "npm@9.0.0"` or `"packageManager": "pnpm@9.0.0+hash"`
///
/// Returns the manager name (npm, pnpm, yarn, bun) and optional version.
/// Also includes scripts.dev from package.json if present.
pub async fn detect_package_manager(
    fs: &Arc<dyn DetectorFilesystem>,
) -> Result<Option<PackageManagerInfo>, String> {
    // First, check for lockfiles to determine package manager
    let lockfile_detection = detect_from_lockfiles(fs).await?;

    // Then check package.json's packageManager field (takes precedence)
    let package_manager_from_json = detect_from_package_json(fs).await?;

    // packageManager field takes precedence over lockfile detection
    if let Some(pm_info) = package_manager_from_json {
        Ok(Some(pm_info))
    } else if let Some(mut pm_info) = lockfile_detection {
        // If we detected from lockfile, try to get scripts from package.json
        // We need to read package.json to get scripts.dev, scripts.install, and scripts.build
        let path = "package.json";
        if fs.is_file(path).await {
            if let Ok(content) = fs.read_file(path).await {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let scripts = json.get("scripts");
                    if let Some(dev_script) = scripts
                        .and_then(|s| s.get("dev"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .filter(|s| !s.trim().is_empty())
                    {
                        pm_info.dev_script = Some(dev_script);
                    }
                    if let Some(install_script) = scripts
                        .and_then(|s| s.get("install"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .filter(|s| !s.trim().is_empty())
                    {
                        pm_info.install_script = Some(install_script);
                    }
                    if let Some(build_script) = scripts
                        .and_then(|s| s.get("build"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .filter(|s| !s.trim().is_empty())
                    {
                        pm_info.build_script = Some(build_script);
                    }
                }
            }
        }
        Ok(Some(pm_info))
    } else {
        Ok(None)
    }
}

/// Hugo version requirements detected from config files
#[derive(Debug, Clone, Default)]
pub struct HugoInfo {
    /// Whether Hugo extended is required (for SCSS/SASS support)
    pub extended: bool,
    /// Minimum version required (e.g., "0.83.0")
    pub min_version: Option<String>,
}

/// Detect Hugo version requirements from Hugo config files
/// Checks config.toml, hugo.toml, config.yaml, hugo.yaml, etc.
/// Looks for [module.hugoVersion] section with extended and min fields
pub async fn detect_hugo_info(fs: &Arc<dyn DetectorFilesystem>) -> Result<Option<HugoInfo>, String> {
    // Hugo config files to check (in priority order)
    let toml_configs = ["config.toml", "hugo.toml"];
    let yaml_configs = ["config.yaml", "config.yml", "hugo.yaml", "hugo.yml"];

    // Try TOML configs first
    for config_file in toml_configs {
        if !fs.is_file(config_file).await {
            continue;
        }
        if let Ok(content) = fs.read_file(config_file).await {
            if let Some(info) = parse_hugo_toml_config(&content) {
                return Ok(Some(info));
            }
        }
    }

    // Try YAML configs
    for config_file in yaml_configs {
        if !fs.is_file(config_file).await {
            continue;
        }
        if let Ok(content) = fs.read_file(config_file).await {
            if let Some(info) = parse_hugo_yaml_config(&content) {
                return Ok(Some(info));
            }
        }
    }

    // No Hugo version requirements found, return default (non-extended, latest)
    Ok(Some(HugoInfo::default()))
}

/// Parse Hugo TOML config to extract [module.hugoVersion] section
fn parse_hugo_toml_config(content: &str) -> Option<HugoInfo> {
    // Parse TOML
    let parsed: toml::Value = toml::from_str(content).ok()?;

    // Look for module.hugoVersion section
    let hugo_version = parsed
        .get("module")
        .and_then(|m| m.get("hugoVersion"))?;

    let extended = hugo_version
        .get("extended")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let min_version = hugo_version
        .get("min")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(HugoInfo {
        extended,
        min_version,
    })
}

/// Parse Hugo YAML config to extract module.hugoVersion section
fn parse_hugo_yaml_config(content: &str) -> Option<HugoInfo> {
    // Parse YAML as JSON value (serde_yaml uses serde_json::Value compatible structure)
    let parsed: serde_json::Value = serde_yaml::from_str(content).ok()?;

    // Look for module.hugoVersion section
    let hugo_version = parsed
        .get("module")
        .and_then(|m| m.get("hugoVersion"))?;

    let extended = hugo_version
        .get("extended")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let min_version = hugo_version
        .get("min")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(HugoInfo {
        extended,
        min_version,
    })
}
