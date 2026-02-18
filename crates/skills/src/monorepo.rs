//! Monorepo package discovery for npm/pnpm/yarn/Lerna workspaces.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use starbase_utils::fs;

/// Discover all workspace package directories from root.
/// Returns canonical paths. If no workspace config found, returns [root] if root has package.json.
pub fn discover_packages(root: &Path) -> Vec<PathBuf> {
    let root = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => root.to_path_buf(),
    };

    let patterns = try_pnpm_workspace(&root)
        .or_else(|| try_lerna(&root))
        .or_else(|| try_npm_workspaces(&root));

    let mut packages: HashSet<PathBuf> = HashSet::new();
    let mut exclusions: Vec<String> = Vec::new();

    if let Some(patterns) = patterns {
        for pattern in patterns {
            if let Some(exclusion) = pattern.strip_prefix('!') {
                exclusions.push(exclusion.to_string());
                continue;
            }
            let expanded = expand_workspace_pattern(&root, &pattern);
            packages.extend(expanded);
        }
    }

    for exclusion in &exclusions {
        packages.retain(|p| !path_matches_exclusion(p, exclusion, &root));
    }

    if root.join("package.json").exists() {
        packages.insert(root.clone());
    }

    if packages.is_empty() && root.join("package.json").exists() {
        return vec![root];
    }

    let mut out: Vec<PathBuf> = packages.into_iter().collect();
    out.sort();
    out
}

fn try_pnpm_workspace(root: &Path) -> Option<Vec<String>> {
    let path = root.join("pnpm-workspace.yaml");
    if !path.exists() {
        return None;
    }
    let content = fs::read_file(&path).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    let packages = yaml.get("packages")?.as_sequence()?;
    let patterns: Vec<String> = packages
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    if patterns.is_empty() {
        return None;
    }
    Some(patterns)
}

fn try_lerna(root: &Path) -> Option<Vec<String>> {
    let path = root.join("lerna.json");
    if !path.exists() {
        return None;
    }
    let content = fs::read_file(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let packages = json.get("packages")?.as_array()?;
    let patterns: Vec<String> = packages
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    if patterns.is_empty() {
        return None;
    }
    Some(patterns)
}

fn try_npm_workspaces(root: &Path) -> Option<Vec<String>> {
    let path = root.join("package.json");
    if !path.exists() {
        return None;
    }
    let content = fs::read_file(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let workspaces = json.get("workspaces")?;
    let patterns: Vec<String> = if let Some(arr) = workspaces.as_array() {
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    } else if let Some(obj) = workspaces.as_object() {
        obj.get("packages")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })?
    } else {
        return None;
    };
    if patterns.is_empty() {
        return None;
    }
    Some(patterns)
}

/// Expand a workspace pattern (e.g. "packages/*", "packages/**") to package dirs.
/// Each matched path must contain package.json.
fn path_matches_exclusion(path: &Path, exclusion: &str, root: &Path) -> bool {
    let rel = match path.strip_prefix(root) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    let escaped = regex::escape(exclusion);
    let pattern = escaped
        .replace("\\*\\*", ".*")
        .replace("\\*", "[^/]*");
    regex::Regex::new(&format!("^{}$", pattern))
        .or_else(|_| regex::Regex::new(&format!(".*{}.*", pattern)))
        .map_or(false, |re| re.is_match(&rel_str))
}

fn expand_workspace_pattern(root: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();

    if pattern.starts_with('!') {
        return out;
    }

    let normalized = pattern.trim().trim_matches('/');
    if normalized.is_empty() {
        return out;
    }

    let has_double_star = normalized.contains("**");
    let glob_pattern = if has_double_star {
        format!("{}/**/package.json", normalized)
    } else {
        format!("{}/package.json", normalized)
    };

    let full_pattern = root.join(&glob_pattern);
    let pattern_str = full_pattern.to_string_lossy().replace('\\', "/");

    if let Ok(entries) = glob::glob(&pattern_str) {
        for entry in entries.flatten() {
            if let Some(parent) = entry.parent() {
                if parent.join("package.json").exists() {
                    if let Ok(canon) = parent.canonicalize() {
                        out.push(canon);
                    } else {
                        out.push(parent.to_path_buf());
                    }
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_packages_pnpm() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("test-monorepo");
        if !root.exists() {
            return;
        }
        let packages = discover_packages(&root);
        assert!(
            packages.len() >= 2,
            "expected at least 2 packages (packages/a, apps/web), got {:?}",
            packages
        );
    }
}
