//! Thin, argv-only wrapper around the `rustic` CLI.
//!
//! Spellings below were verified against rustic v0.11.4 - the version mise
//! installs from [`MISE_TOOL`]:
//!
//! - repository: `--repository` / `--repo` / `-r`, env `RUSTIC_REPOSITORY`
//! - password: `--password` or env `RUSTIC_PASSWORD`; appz only ever uses the
//!   env var so the secret never shows up in a process list. Users set the
//!   appz-named [`ENV_REPOSITORY`] / [`ENV_PASSWORD`], which are translated
//!   to the `RUSTIC_*` names for the child process.
//! - init: `rustic init`
//! - backup: `rustic backup [--json] [--host NAME] [--tag TAG[,TAG]...]
//!   [--skip-if-unchanged] SOURCE...`. `--json` prints the snapshot object;
//!   with `--skip-if-unchanged` and no changes rustic prints no JSON and
//!   exits 0. There is **no** `--files-from`: every selected path is passed
//!   as its own SOURCE argument.
//! - list: `rustic snapshots --json` -> array of
//!   `{ group_key: { hostname, label, paths }, snapshots: [{ id, time, tags,
//!   summary: { data_added, total_bytes_processed, ... }, ... }] }`
//! - size: `rustic repoinfo --json` -> `files.repo[].size` (bytes)
//! - retention: `rustic forget [--json] --keep-last N --keep-daily N
//!   --keep-weekly N [--prune]`
//! - prune: `rustic prune`
//! - restore: `rustic restore <SNAPSHOT[:PATH]> <DEST> [--delete]`
//! - compression: whatever the repository was initialised with (zstd, auto
//!   level) - nothing to pass per backup
//! - rustic is lock-free, so a concurrent prune/forget is the realistic
//!   failure; [`EngineError::Locked`] is a defensive classification.
//! - wrong password stderr: "The password that has been entered, seems to be
//!   incorrect. No suitable key found for the given password."
//!   (rustic error C002, error class `credentials handling`)

use std::path::{Path, PathBuf};
use std::time::Duration;

use appz_core::{ensure_mise, install_global_tool, mise_bin_paths};
use appz_deploy::exec_captured;

use crate::deploy::Consent;

/// mise tool spec that provides the `rustic` binary.
pub const MISE_TOOL: &str = "aqua:rustic-rs/rustic";

/// Repository override users set in appz's own namespace.
pub const ENV_REPOSITORY: &str = "APPZ_BACKUP_REPOSITORY";
/// Password override users set in appz's own namespace (the vault is the
/// normal source; this exists for one-off runs and CI).
pub const ENV_PASSWORD: &str = "APPZ_BACKUP_PASSWORD";

/// `APPZ_BACKUP_REPOSITORY`, when set and non-empty.
pub fn env_repository() -> Option<PathBuf> {
    non_empty(std::env::var_os(ENV_REPOSITORY)?).map(PathBuf::from)
}

/// `APPZ_BACKUP_PASSWORD`, when set and non-empty.
pub fn env_password() -> Option<String> {
    non_empty(std::env::var_os(ENV_PASSWORD)?).map(|v| v.to_string_lossy().into_owned())
}

fn non_empty(value: std::ffi::OsString) -> Option<std::ffi::OsString> {
    if value.is_empty() { None } else { Some(value) }
}

/// Timeout for read-only queries (snapshots/repoinfo).
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
/// Timeout for an actual backup/prune/restore (uploads can legitimately take
/// minutes).
const BACKUP_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// Output of one rustic invocation.
#[derive(Debug, Clone)]
pub struct RunOut {
    pub stdout: String,
    pub stderr: String,
}

/// Why a rustic invocation did not succeed.
#[derive(Debug)]
pub enum EngineError {
    /// The `rustic` binary is not installed (or not on PATH).
    Missing,
    /// Another rustic process holds the repository.
    Locked,
    /// The repository password does not match.
    WrongPassword,
    /// Anything else rustic reported.
    Failed(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(
                f,
                "rustic is not installed. Install it with `mise use -g {MISE_TOOL}` (or re-run with --yes)"
            ),
            Self::Locked => write!(
                f,
                "the backup repository is locked by another rustic process"
            ),
            Self::WrongPassword => write!(
                f,
                "the backup repository password is incorrect (check APPZ_BACKUP_PASSWORD or the vault entry)"
            ),
            Self::Failed(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for EngineError {}

/// The rustic surface the backup tasks need. Kept as a trait so every later
/// task can be tested against a fake engine instead of spawning processes.
pub trait Rustic {
    fn run(&self, args: &[&str]) -> Result<RunOut, EngineError>;
}

/// One repository, one password, one working directory.
#[derive(Debug, Clone)]
pub struct RusticCli {
    bin: Option<PathBuf>,
    repo: PathBuf,
    password: String,
    cwd: PathBuf,
    timeout: Duration,
}

impl RusticCli {
    /// Build a client, resolving the binary now. A missing binary is not an
    /// error here: [`Rustic::run`] reports [`EngineError::Missing`], so
    /// callers that only read `--help`-less metadata can still be constructed.
    pub fn new(
        repo: impl Into<PathBuf>,
        password: impl Into<String>,
        cwd: impl Into<PathBuf>,
    ) -> Self {
        let cwd = cwd.into();
        let bin = find_rustic(&cwd);
        Self {
            bin,
            repo: repo.into(),
            password: password.into(),
            cwd,
            timeout: BACKUP_TIMEOUT,
        }
    }

    /// Override the resolved binary (tests, and callers that already know
    /// where mise put rustic).
    pub fn with_bin(mut self, bin: impl Into<PathBuf>) -> Self {
        self.bin = Some(bin.into());
        self
    }

    /// Use the short timeout reserved for read-only queries.
    pub fn with_query_timeout(mut self) -> Self {
        self.timeout = QUERY_TIMEOUT;
        self
    }

    /// Run with an explicit timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The executable, repository and environment for one invocation.
    /// Exposed so tests can prove the password never becomes an argument.
    ///
    /// The child only sees `RUSTIC_*`: those are the names rustic reads. The
    /// appz-facing names ([`ENV_REPOSITORY`], [`ENV_PASSWORD`]) are resolved
    /// before construction.
    fn invocation(&self, args: &[&str]) -> (PathBuf, Vec<String>, Vec<(&'static str, String)>) {
        let bin = self.bin.clone().unwrap_or_else(|| PathBuf::from("rustic"));
        let mut all = Vec::with_capacity(args.len() + 1);
        all.push("--no-progress".to_string());
        all.extend(args.iter().map(|a| (*a).to_string()));
        let env = vec![
            (
                "RUSTIC_REPOSITORY",
                self.repo.to_string_lossy().into_owned(),
            ),
            ("RUSTIC_PASSWORD", self.password.clone()),
        ];
        (bin, all, env)
    }

    fn run_inner(&self, args: &[&str]) -> Result<RunOut, EngineError> {
        let (bin, all, env) = self.invocation(args);
        let borrowed: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let arg_refs: Vec<&str> = all.iter().map(String::as_str).collect();

        let out = exec_captured(
            &bin.to_string_lossy(),
            &arg_refs,
            &self.cwd,
            &borrowed,
            self.timeout,
        )
        .map_err(|e| {
            if looks_like_missing_binary(&e) {
                EngineError::Missing
            } else {
                EngineError::Failed(e)
            }
        })?;

        if out.success {
            return Ok(RunOut {
                stdout: out.stdout,
                stderr: out.stderr,
            });
        }

        Err(classify(&out.stderr, &out.stdout, out.code))
    }
}

impl Rustic for RusticCli {
    fn run(&self, args: &[&str]) -> Result<RunOut, EngineError> {
        self.run_inner(args)
    }
}

/// Classify a non-zero exit from rustic's own output.
fn classify(stderr: &str, stdout: &str, code: Option<i32>) -> EngineError {
    let combined = format!("{stderr}\n{stdout}");
    let lower = combined.to_lowercase();

    if lower.contains("no suitable key found")
        || lower.contains("password that has been entered, seems to be incorrect")
        || lower.contains("wrong password")
    {
        return EngineError::WrongPassword;
    }
    if lower.contains("locked") || lower.contains("repository is in use") {
        return EngineError::Locked;
    }

    let message = if !stderr.trim().is_empty() {
        stderr.trim()
    } else if !stdout.trim().is_empty() {
        stdout.trim()
    } else {
        return EngineError::Failed(format!("rustic exited with {code:?}"));
    };
    EngineError::Failed(message.to_string())
}

/// Spawn failures mean "no such binary" or "permission denied"; only the
/// former maps to [`EngineError::Missing`].
fn looks_like_missing_binary(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("os error 2")
        || lower.contains("cannot find the file")
        || lower.contains("no such file")
        || lower.contains("not found")
}

/// Whether `rustic` can be spawned from `dir`: either it is on PATH, or mise
/// installed it into one of its bin dirs (shims are not always activated).
pub fn find_rustic(dir: &Path) -> Option<PathBuf> {
    let on_path = command::Command::new("rustic")
        .arg("--version")
        .without_shell()
        .set_error_on_nonzero(false)
        .cwd(dir)
        .exec()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if on_path {
        return Some(PathBuf::from("rustic"));
    }

    let exe = format!("rustic{}", std::env::consts::EXE_SUFFIX);
    mise_bin_paths(dir).into_iter().find_map(|bin_dir| {
        let candidate = bin_dir.join(&exe);
        candidate.is_file().then_some(candidate)
    })
}

/// Return a usable `rustic` path, installing it through mise first when it is
/// missing and the policy (or the user, when asked) allows. This is the same
/// flow `appz deploy` uses for platform CLIs, so `appz backup` works on a
/// machine that never installed rustic.
pub fn ensure_installed(project_dir: &Path, install: Consent) -> Result<PathBuf, String> {
    if let Some(bin) = find_rustic(project_dir) {
        return Ok(bin);
    }

    match install {
        Consent::Never => {
            return Err(format!(
                "rustic is not installed. Install it with `mise use -g {MISE_TOOL}`, or re-run with --yes to install it"
            ));
        }
        Consent::Ask => {
            let prompt = format!("rustic is not installed. Install {MISE_TOOL} with mise?");
            if !cliclack::confirm(prompt)
                .initial_value(true)
                .interact()
                .unwrap_or(false)
            {
                return Err(format!(
                    "rustic is not installed. Install it with `mise use -g {MISE_TOOL}`"
                ));
            }
        }
        Consent::Yes => {}
    }

    ensure_mise(project_dir)?;
    eprintln!("installing {MISE_TOOL} with mise...");
    install_global_tool(MISE_TOOL)?;

    find_rustic(project_dir).ok_or_else(|| {
        "rustic was installed but is not on PATH - restart your shell or run `mise reshim`"
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli() -> RusticCli {
        RusticCli::new("C:/repos/abc", "hunter2", "C:/project").with_bin("C:/fake/rustic.exe")
    }

    #[test]
    fn classifies_wrong_password_errors() {
        let stderr = "error: `rustic_core` experienced an error related to `credentials handling`.\n\
                      Message:\n\
                      The password that has been entered, seems to be incorrect. No suitable key found for the given password.";
        assert!(matches!(
            classify(stderr, "", Some(1)),
            EngineError::WrongPassword
        ));
    }

    #[test]
    fn classifies_lock_errors() {
        assert!(matches!(
            classify("repository is locked by another process", "", Some(1)),
            EngineError::Locked
        ));
    }

    #[test]
    fn passes_other_failures_through_verbatim() {
        match classify("something exploded", "", Some(2)) {
            EngineError::Failed(msg) => assert_eq!(msg, "something exploded"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn password_is_never_on_the_command_line() {
        let (bin, args, env) = cli().invocation(&["backup", "--json", "src"]);
        assert_eq!(bin, PathBuf::from("C:/fake/rustic.exe"));
        assert_eq!(
            args[0], "--no-progress",
            "rustic must not draw progress bars"
        );
        assert!(
            !args.iter().any(|a| a.contains("hunter2")),
            "password leaked into argv: {args:?}"
        );
        assert!(
            !args.iter().any(|a| a == "--password"),
            "--password must never be used: {args:?}"
        );
        assert!(env.contains(&("RUSTIC_PASSWORD", "hunter2".to_string())));
        assert!(env.contains(&("RUSTIC_REPOSITORY", "C:/repos/abc".to_string())));
    }

    #[test]
    fn appz_owns_the_env_names_and_translates_them_for_rustic() {
        assert_eq!(ENV_REPOSITORY, "APPZ_BACKUP_REPOSITORY");
        assert_eq!(ENV_PASSWORD, "APPZ_BACKUP_PASSWORD");

        let (_, _, env) = cli().invocation(&["snapshots"]);
        assert!(
            !env.iter().any(|(k, _)| k.starts_with("APPZ_")),
            "appz names must not leak into the child: {env:?}"
        );
        assert!(env.iter().any(|(k, _)| *k == "RUSTIC_PASSWORD"));
        assert!(env.iter().any(|(k, _)| *k == "RUSTIC_REPOSITORY"));
    }

    #[test]
    fn missing_binary_is_missing_not_a_generic_failure() {
        let cli = RusticCli::new("C:/repos/abc", "pw", "C:/project")
            .with_bin("C:/definitely/not/here/rustic-xyz.exe");
        assert!(matches!(cli.run(&["snapshots"]), Err(EngineError::Missing)));
    }
}
