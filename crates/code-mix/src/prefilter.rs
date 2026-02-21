//! Pre-filters: content search (ripgrep), git modes, bundle load, path discovery.
//!
//! When options require a custom file list, we run the appropriate filter
//! and return paths to pipe to Repomix --stdin. All external commands
//! (ripgrep, git) run via the sandbox. When no filter applies, we discover
//! paths from include/ignore patterns so the pack can be cached.

use std::path::Path;

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use sandbox::SandboxProvider;

use crate::repomix::RepomixError;
use crate::types::PackOptions;

/// If options require a pre-filter (strings, git modes, bundle), return
/// the file list to pipe to Repomix --stdin. Otherwise discover paths from
/// include/ignore so the pack can be cached.
///
/// All external commands run through `sandbox`.
pub async fn get_stdin_paths(
    sandbox: &dyn SandboxProvider,
    options: &PackOptions,
) -> Result<Option<Vec<String>>, RepomixError> {
    if let Some(paths) = content_search_paths(sandbox, options).await? {
        return Ok(Some(paths));
    }
    if let Some(paths) = git_mode_paths(sandbox, options).await? {
        return Ok(Some(paths));
    }
    if let Some(paths) = bundle_paths(sandbox, options).await? {
        return Ok(Some(paths));
    }

    // Default: discover paths from include/ignore so we can cache the pack
    let workdir = sandbox.project_path();
    let paths = discover_paths_from_patterns(workdir, options)?;
    if paths.is_empty() {
        return Ok(None); // Fall back to non-cached run (e.g. fully ignored)
    }
    Ok(Some(paths))
}

/// Discover file paths by walking workdir with include/ignore glob patterns.
pub fn discover_paths_from_patterns(
    workdir: &Path,
    options: &PackOptions,
) -> Result<Vec<String>, RepomixError> {
    let mut overrides = OverrideBuilder::new(workdir);

    // Ignore patterns (gitignore semantics: !prefix = ignore)
    for p in &options.ignore {
        let rule = format!("!{}", p.trim());
        overrides
            .add(&rule)
            .map_err(|e| RepomixError(format!("Invalid ignore pattern '{}': {}", p, e)))?;
    }

    // When include is set, whitelist those patterns first, then ignore all else
    if !options.include.is_empty() {
        for p in &options.include {
            let rule = p.trim();
            if !rule.is_empty() {
                overrides
                    .add(rule)
                    .map_err(|e| RepomixError(format!("Invalid include pattern '{}': {}", p, e)))?;
            }
        }
        overrides
            .add("!**")
            .map_err(|e| RepomixError(format!("Override error: {}", e)))?;
    }

    let overrides = overrides
        .build()
        .map_err(|e| RepomixError(format!("Override build failed: {}", e)))?;

    let mut paths: Vec<String> = Vec::new();
    for result in WalkBuilder::new(workdir)
        .overrides(overrides)
        .standard_filters(true)
        .hidden(false)
        .build()
    {
        let entry = result.map_err(|e| RepomixError(format!("Walk error: {}", e)))?;
        if entry.path().is_file() {
            if let Ok(rel) = entry.path().strip_prefix(workdir) {
                let s = rel.as_os_str().to_string_lossy().replace('\\', "/");
                if !s.is_empty() {
                    paths.push(s);
                }
            }
        }
    }

    paths.sort();
    Ok(paths)
}

/// Run ripgrep to find files containing search strings; pipe to Repomix.
async fn content_search_paths(
    sandbox: &dyn SandboxProvider,
    options: &PackOptions,
) -> Result<Option<Vec<String>>, RepomixError> {
    if options.strings.is_empty() && options.exclude_strings.is_empty() {
        return Ok(None);
    }

    let workdir_str = sandbox.project_path().display().to_string();
    let mut all_paths: Option<Vec<String>> = None;

    for pattern in &options.strings {
        // Shell-escape pattern for safe use in rg
        let escaped = pattern.replace('"', "\\\"");
        let cmd = format!("rg -l \"{}\"", escaped);
        let out = sandbox
            .exec(&cmd)
            .await
            .map_err(|e| RepomixError(format!("ripgrep failed: {}", e)))?;

        if !out.success() {
            if out.exit_code() == Some(1) {
                return Err(RepomixError(format!(
                    "No files contain \"{}\" in {}",
                    pattern, workdir_str
                )));
            }
            return Err(RepomixError(format!("ripgrep failed: {}", out.stderr())));
        }

        let paths: Vec<String> = out
            .stdout()
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        all_paths = Some(match all_paths.take() {
            Some(existing) => {
                let set: std::collections::HashSet<_> =
                    existing.into_iter().chain(paths).collect();
                set.into_iter().collect()
            }
            None => paths,
        });
    }

    if let Some(mut paths) = all_paths {
        for pattern in &options.exclude_strings {
            let escaped = pattern.replace('"', "\\\"");
            let cmd = format!("rg -l \"{}\"", escaped);
            let out = sandbox
                .exec(&cmd)
                .await
                .map_err(|e| RepomixError(format!("ripgrep failed: {}", e)))?;

            if out.success() {
                let exclude: std::collections::HashSet<String> = out
                    .stdout()
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                paths.retain(|p| !exclude.contains(p));
            }
        }

        paths.sort();
        return Ok(Some(paths));
    }

    Ok(None)
}

/// Get file list from git (staged, dirty, diff).
async fn git_mode_paths(
    sandbox: &dyn SandboxProvider,
    options: &PackOptions,
) -> Result<Option<Vec<String>>, RepomixError> {
    let git_mode = if options.staged {
        Some("staged")
    } else if options.dirty {
        Some("dirty")
    } else if options.diff {
        Some("diff")
    } else {
        None
    };

    let Some(mode) = git_mode else {
        return Ok(None);
    };

    let out = match mode {
        "staged" => {
            sandbox
                .exec("git diff --cached --name-only")
                .await
                .map_err(|e| RepomixError(format!("git failed: {}", e)))?
        }
        "dirty" => {
            let out = sandbox
                .exec("git status -u --porcelain")
                .await
                .map_err(|e| RepomixError(format!("git failed: {}", e)))?;
            if !out.success() {
                return Err(RepomixError(format!("git failed: {}", out.stderr())));
            }
            let paths: Vec<String> = out
                .stdout()
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    if line.len() >= 4 {
                        let rest = line[3..].trim();
                        let path = if let Some(idx) = rest.find(" -> ") {
                            rest[idx + 4..].trim()
                        } else {
                            rest
                        };
                        if !path.is_empty() {
                            return Some(path.to_string());
                        }
                    }
                    None
                })
                .collect();
            return Ok(Some(paths));
        }
        "diff" => {
            let base = options.diff_base.as_deref().unwrap_or("main");
            sandbox
                .exec(&format!("git diff {} --name-only", base))
                .await
                .map_err(|e| RepomixError(format!("git failed: {}", e)))?
        }
        _ => return Ok(None),
    };

    if !out.success() {
        return Err(RepomixError(format!("git failed: {}", out.stderr())));
    }

    let paths: Vec<String> = out
        .stdout()
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(Some(paths))
}

/// Load file list from a saved bundle via sandbox fs.
async fn bundle_paths(
    sandbox: &dyn SandboxProvider,
    options: &PackOptions,
) -> Result<Option<Vec<String>>, RepomixError> {
    let name = match &options.bundle {
        Some(n) => n,
        None => return Ok(None),
    };

    let rel_path = format!(".pack/bundles/{}.bundle", name);
    let content = sandbox.fs().read_to_string(&rel_path).map_err(|e| {
        RepomixError(format!("Failed to load bundle '{}': {}", name, e))
    })?;

    let paths: Vec<String> = content
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if paths.is_empty() {
        return Err(RepomixError(format!("Bundle '{}' is empty", name)));
    }

    Ok(Some(paths))
}
