//! `appz update` — self-upgrade from GitHub releases.
//!
//! Split into submodules:
//! - [`github`] — release discovery, download, checksum verification.
//! - [`swap`] — MCP-safe binary replacement (mirrors `dev_install`'s swap, but
//!   adds the Windows locked-exe deferred-swap fallback: `dev-install` runs
//!   from an *installed* binary replacing an assumed-idle target, while
//!   `update` replaces the very binary currently running it).

mod github;
mod swap;

/// Download, verify, and install the requested (or latest) release.
pub fn run(version: Option<String>, check_only: bool, quiet: bool) {
    if !quiet && !check_only {
        crate::intro("appz update");
    }

    let target_version = match resolve_target_version(version, check_only, quiet) {
        Some(v) => v,
        None => return,
    };

    let asset = match github::asset_name(&target_version) {
        Ok(a) => a,
        Err(e) => crate::error(&e),
    };

    let data = if quiet {
        match github::download_asset(&target_version, &asset) {
            Ok(d) => d,
            Err(e) => crate::error(&format!("error downloading: {e}")),
        }
    } else {
        match crate::with_spinner(
            &format!("downloading {asset}..."),
            "download complete",
            || github::download_asset(&target_version, &asset),
        ) {
            Ok(d) => d,
            Err(e) => crate::error(&format!("error downloading: {e}")),
        }
    };

    if quiet {
        match github::expected_checksum(&target_version, &asset) {
            Ok(expected) => {
                if let Err(e) = github::verify_checksum(&data, &expected) {
                    crate::error(&e);
                }
            }
            Err(e) => crate::error(&format!("error fetching checksum: {e}")),
        }
    } else {
        match crate::with_spinner(
            "verifying checksum...",
            "checksum verified",
            || -> Result<(), String> {
                let expected = github::expected_checksum(&target_version, &asset)?;
                github::verify_checksum(&data, &expected)
            },
        ) {
            Ok(()) => {}
            Err(e) => crate::error(&e),
        }
    }

    let new_binary = match extract_binary(&data) {
        Ok(p) => p,
        Err(e) => crate::error(&e),
    };

    let current = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => crate::error(&format!("cannot determine current binary path: {e}")),
    };

    if !quiet {
        crate::step(&format!("replacing {}...", current.display()));
    }
    if let Err(e) = swap::replace_binary(&new_binary, &current) {
        crate::error(&format!("error replacing binary: {e}"));
    }

    if !quiet {
        crate::success(&format!("updated to {target_version}"));
        report_other_instances();
        crate::outro("update complete");
    }
}

/// Resolve which version to install, or `None` when there is nothing to do
/// (already up to date, or `--check` only).
fn resolve_target_version(
    version: Option<String>,
    check_only: bool,
    quiet: bool,
) -> Option<String> {
    match version {
        Some(v) => {
            let v = if v.starts_with('v') {
                v
            } else {
                format!("v{v}")
            };
            if !quiet && !check_only {
                crate::step(&format!("updating to {v}..."));
            }
            Some(v)
        }
        None => {
            let latest = match github::check_for_update() {
                Ok(Some(v)) => v,
                Ok(None) => {
                    if !quiet {
                        if check_only {
                            crate::intro("appz update --check");
                        }
                        crate::success(&format!(
                            "appz is up to date (v{})",
                            env!("CARGO_PKG_VERSION")
                        ));
                        if check_only {
                            crate::outro("nothing to update");
                        }
                    }
                    return None;
                }
                Err(e) => crate::error(&format!("error checking for updates: {e}")),
            };
            if check_only {
                if !quiet {
                    crate::intro("appz update --check");
                    crate::info(&format!(
                        "new version available: {latest} (current: v{})",
                        env!("CARGO_PKG_VERSION")
                    ));
                    crate::outro("run `appz update` to upgrade");
                }
                return None;
            }
            if !quiet {
                crate::step(&format!("latest version: {latest}"));
            }
            Some(latest)
        }
    }
}

/// Extract the release archive into a per-pid tmpdir and return the path to the
/// unpacked binary.
fn extract_binary(data: &[u8]) -> Result<std::path::PathBuf, String> {
    let tmpdir = std::env::temp_dir().join(format!("appz-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmpdir);
    std::fs::create_dir_all(&tmpdir).map_err(|e| format!("create tmpdir: {e}"))?;

    match github::asset_ext() {
        "tar.gz" => {
            let tar_gz = flate2::read::GzDecoder::new(data);
            let mut archive = tar::Archive::new(tar_gz);
            archive
                .unpack(&tmpdir)
                .map_err(|e| format!("extract error: {e}"))?;
        }
        "zip" => {
            let cursor = std::io::Cursor::new(data);
            let mut archive =
                zip::ZipArchive::new(cursor).map_err(|e| format!("zip error: {e}"))?;
            archive
                .extract(&tmpdir)
                .map_err(|e| format!("extract error: {e}"))?;
        }
        other => return Err(format!("unsupported asset extension: {other}")),
    }

    let new_binary = tmpdir.join(github::binary_name());
    if !new_binary.exists() {
        return Err("binary not found in archive".to_string());
    }
    Ok(new_binary)
}

/// Print an advisory listing other running instances that should be restarted.
fn report_other_instances() {
    let others = swap::find_killable_pids();
    if !others.is_empty() {
        let list = others
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        crate::warning(&format!(
            "{} other appz process(es) still running the old binary (pid {list}); \
             restart them to pick up this update",
            others.len()
        ));
    }
}
