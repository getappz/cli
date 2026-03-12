//! Monorepo workspace detection and resolution.
//!
//! Supports pnpm-workspace.yaml and package.json workspaces.

use std::path::{Path, PathBuf};

use crate::repomix::RepomixError;

/// A workspace in a monorepo.
#[derive(Debug)]
pub struct Workspace {
    pub name: String,
    pub path: PathBuf,
    pub relative_path: String,
}

/// Detect workspaces from pnpm-workspace.yaml or package.json.
pub async fn detect_monorepo(workdir: &Path) -> Result<Option<Vec<Workspace>>, RepomixError> {
    // pnpm-workspace.yaml
    let pnpm = workdir.join("pnpm-workspace.yaml");
    if pnpm.exists() {
        let content = tokio::fs::read_to_string(&pnpm)
            .await
            .map_err(|e| RepomixError(format!("Failed to read pnpm-workspace.yaml: {}", e)))?;
        let config: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| RepomixError(format!("Invalid pnpm-workspace.yaml: {}", e)))?;
        let packages = config
            .get("packages")
            .and_then(|p| p.as_sequence())
            .ok_or_else(|| RepomixError("pnpm-workspace.yaml: missing packages".into()))?;
        let globs: Vec<String> = packages
            .iter()
            .filter_map(|p| p.as_str().map(String::from))
            .collect();
        if !globs.is_empty() {
            return resolve_workspace_globs(workdir, &globs).await;
        }
    }

    // package.json workspaces
    let pkg = workdir.join("package.json");
    if pkg.exists() {
        let content = tokio::fs::read_to_string(&pkg)
            .await
            .map_err(|e| RepomixError(format!("Failed to read package.json: {}", e)))?;
        let pkg_json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| RepomixError(format!("Invalid package.json: {}", e)))?;
        let globs: Vec<String> = match pkg_json.get("workspaces") {
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            Some(serde_json::Value::Object(obj)) => obj
                .get("packages")
                .and_then(|p| p.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            _ => vec![],
        };
        if !globs.is_empty() {
            return resolve_workspace_globs(workdir, &globs).await;
        }
    }

    Ok(None)
}

async fn resolve_workspace_globs(
    root: &Path,
    globs: &[String],
) -> Result<Option<Vec<Workspace>>, RepomixError> {
    let mut workspaces = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for glob_pattern in globs {
        // Expand "packages/*" -> read packages/ dir, each subdir with package.json is a workspace
        let base = glob_pattern.split('*').next().unwrap_or("").trim_end_matches('/');
        if base.is_empty() {
            continue;
        }
        let base_path = root.join(base);
        if !base_path.is_dir() {
            continue;
        }
        let mut entries = tokio::fs::read_dir(&base_path)
            .await
            .map_err(|e| RepomixError(format!("Failed to read {}: {}", base_path.display(), e)))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| RepomixError(format!("Failed to read dir: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() && seen.insert(path.clone()) {
                let package_json = path.join("package.json");
                if package_json.exists() {
                    let name = read_package_name(&package_json).await.unwrap_or_else(|| {
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    });
                    let rel = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    workspaces.push(Workspace {
                        name,
                        path,
                        relative_path: rel,
                    });
                }
            }
        }
    }

    workspaces.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Some(workspaces))
}

async fn read_package_name(path: &Path) -> Option<String> {
    let content = tokio::fs::read_to_string(path).await.ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;
    pkg.get("name")?.as_str().map(String::from)
}

/// Resolve workspace by name (e.g. "@org/pkg" or "packages/foo").
pub async fn resolve_workspace(
    workdir: &Path,
    name: &str,
) -> Result<Option<Workspace>, RepomixError> {
    let Some(workspaces) = detect_monorepo(workdir).await? else {
        return Ok(None);
    };
    let name_norm = name.trim_start_matches('@');
    for ws in workspaces {
        let ws_name = ws.name.trim_start_matches('@');
        if ws_name.eq_ignore_ascii_case(name_norm)
            || ws.relative_path == name
            || ws.name == name
        {
            return Ok(Some(ws));
        }
    }
    Ok(None)
}
