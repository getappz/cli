//! `appz dev-install` — build the current source tree and install it over the
//! running `appz` binary, so local changes can be tested as the real command.
//!
//! Intended to be run from your *installed* `appz` (e.g. after
//! `cargo install --path crates/appz`) inside a checkout: it builds the
//! checkout, verifies the fresh binary runs, then swaps it into place — and
//! refuses to overwrite a working install with a broken build.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long to wait for the freshly built binary to answer `--version` before
/// declaring the build broken. `--version` returns immediately; this only
/// guards a pathological hang.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(15);

/// Build (release unless `!release`), verify, and replace the running binary.
pub fn run(release: bool, dry_run: bool) {
    crate::step(&format!(
        "building appz ({})...",
        if release { "release" } else { "debug" }
    ));
    let built = match build_and_locate(release) {
        Ok(p) if p.exists() => p,
        Ok(p) => crate::error(&format!(
            "cargo reported {} but it does not exist",
            p.display()
        )),
        Err(e) => crate::error(&e),
    };

    // Verify the fresh build runs *before* replacing anything, so a broken
    // build never overwrites a working install.
    if let Err(e) = verify_runs(&built) {
        crate::error(&format!("built binary failed verification: {e}"));
    }

    let target = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => crate::error(&format!("cannot determine current binary path: {e}")),
    };

    if same_file(&built, &target) {
        crate::error(&format!(
            "refusing to install over the build output itself ({}).\n\
             Run `dev-install` from your *installed* appz, not the freshly built binary.",
            target.display()
        ));
    }

    if dry_run {
        crate::info(&format!(
            "dry-run: would install {} -> {}",
            built.display(),
            target.display()
        ));
        return;
    }

    crate::step(&format!(
        "installing {} -> {}",
        built.display(),
        target.display()
    ));
    if let Err(e) = replace_binary(&built, &target) {
        crate::error(&format!("error installing binary: {e}"));
    }
    crate::success(&format!("installed to {}", target.display()));
    crate::info("run `appz --version` to confirm");
}

/// Build the `appz` binary from the current source tree and return the path
/// cargo actually wrote it to.
///
/// Reads the executable path from cargo's `compiler-artifact` JSON rather than
/// reconstructing `target/<profile>/appz`: that reconstruction is wrong whenever
/// `build.target` / `CARGO_BUILD_TARGET` adds a `<triple>/` segment or a custom
/// profile changes the directory name. Human progress streams to stderr.
fn build_and_locate(release: bool) -> Result<PathBuf, String> {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "build",
        "-p",
        "appz",
        "--bin",
        "appz",
        "--message-format",
        "json-render-diagnostics",
    ]);
    if release {
        cmd.arg("--release");
    }

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("failed to run cargo build: {e}"))?;

    // stderr is inherited (live progress); stdout is the JSON stream we parse.
    // Only stdout is a pipe, so draining it fully cannot deadlock.
    let mut json = String::new();
    let read_result = child
        .stdout
        .take()
        .expect("stdout was piped")
        .read_to_string(&mut json);

    // Always reap the child, even if reading stdout failed, so cargo is never
    // left as an orphan.
    let status = child.wait().map_err(|e| format!("waiting on cargo: {e}"))?;
    read_result.map_err(|e| format!("reading cargo output: {e}"))?;
    if !status.success() {
        return Err("cargo build failed".to_string());
    }

    parse_executable_path(&json)
        .ok_or_else(|| "cargo build did not report an appz executable".to_string())
}

/// Find the `appz` binary path in cargo's JSON build output. Pure, so it is
/// unit-testable without invoking cargo. Returns the last matching
/// `compiler-artifact` executable (there is normally exactly one).
fn parse_executable_path(build_json: &str) -> Option<PathBuf> {
    let mut found = None;
    for line in build_json.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact") {
            continue;
        }
        let name = v
            .get("target")
            .and_then(|t| t.get("name"))
            .and_then(serde_json::Value::as_str);
        if name != Some("appz") {
            continue;
        }
        if let Some(exe) = v.get("executable").and_then(serde_json::Value::as_str) {
            found = Some(PathBuf::from(exe));
        }
    }
    found
}

/// Run `<binary> --version` and confirm it exits successfully within
/// [`VERIFY_TIMEOUT`].
fn verify_runs(binary: &Path) -> Result<(), String> {
    let mut child = Command::new(binary)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn --version: {e}"))?;

    let deadline = Instant::now() + VERIFY_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("--version exited with {status}")),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    return Err("--version timed out".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("waiting on --version: {e}")),
        }
    }
}

/// Replace `target` with `new`, handling the case where `target` is the
/// currently-running binary.
///
/// A direct rename works on Unix always, and on Windows when the target isn't a
/// running/locked exe. When Windows refuses to overwrite a running exe, we move
/// the old binary aside (Windows *does* allow renaming a running exe, just not
/// deleting/overwriting it) and put the new one in its place. The stale `.old`
/// copy can't be deleted while the old process image is still mapped, so its
/// removal is best-effort — the next `dev-install` clears it before writing.
fn replace_binary(new: &Path, target: &Path) -> std::io::Result<()> {
    if std::fs::rename(new, target).is_ok() {
        return Ok(());
    }

    let backup = target.with_extension("old");
    let _ = std::fs::remove_file(&backup); // clear any leftover from a prior run
    std::fs::rename(target, &backup)?;

    // `new` (build dir) and `target` (install dir) may be on different volumes,
    // where rename fails with a cross-device error — fall back to copy.
    std::fs::rename(new, target).or_else(|_| std::fs::copy(new, target).map(|_| ()))?;
    let _ = std::fs::remove_file(&backup); // best-effort; may be locked if it's us
    Ok(())
}

/// Whether two paths resolve to the same file. Canonicalizes both (following
/// symlinks); falls back to a raw comparison when a path can't be canonicalized
/// (e.g. the target doesn't exist yet).
fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_executable_path_reads_the_appz_artifact_under_a_target_triple() {
        // The path carries a `<triple>/` segment (build.target set) — exactly the
        // case a reconstructed `target/<profile>/` lookup would get wrong.
        let json = concat!(
            r#"{"reason":"compiler-artifact","target":{"name":"serde"},"executable":null}"#,
            "\n",
            r#"{"reason":"compiler-artifact","target":{"name":"appz"},"executable":"/repo/target/x86_64-pc-windows-msvc/release/appz.exe"}"#,
            "\n",
            r#"{"reason":"build-finished","success":true}"#,
            "\n",
        );
        assert_eq!(
            parse_executable_path(json),
            Some(PathBuf::from(
                "/repo/target/x86_64-pc-windows-msvc/release/appz.exe"
            ))
        );
    }

    #[test]
    fn parse_executable_path_none_when_no_appz_executable() {
        let json = concat!(
            r#"{"reason":"compiler-artifact","target":{"name":"appz"},"executable":null}"#,
            "\n",
            "not json\n",
        );
        assert_eq!(parse_executable_path(json), None);
    }

    #[test]
    fn same_file_true_for_identical_path_false_for_distinct() {
        let dir = std::env::temp_dir().join(format!("appz-devinstall-same-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("bin");
        std::fs::write(&f, b"x").unwrap();
        let other = dir.join("other");
        std::fs::write(&other, b"y").unwrap();

        assert!(same_file(&f, &f));
        assert!(!same_file(&f, &other));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_runs_errors_for_a_missing_binary() {
        let missing = std::env::temp_dir().join("appz-nonexistent-binary-xyz");
        assert!(verify_runs(&missing).is_err());
    }

    #[test]
    fn replace_binary_swaps_a_non_running_target() {
        let dir = std::env::temp_dir().join(format!("appz-devinstall-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let new = dir.join("new");
        let target = dir.join("target");
        std::fs::write(&new, b"fresh").unwrap();
        std::fs::write(&target, b"stale").unwrap();

        replace_binary(&new, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"fresh");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
