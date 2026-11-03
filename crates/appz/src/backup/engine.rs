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
use appz_deploy::exec_captured_scrubbed;

use crate::deploy::{Consent, confirm_mise_install};

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

/// `APPZ_BACKUP_PASSWORD`, when set and non-empty. A value that is not valid
/// UTF-8 is an error rather than being silently altered into a wrong password.
pub fn env_password() -> Result<Option<String>, String> {
    let Some(value) = std::env::var_os(ENV_PASSWORD).and_then(non_empty) else {
        return Ok(None);
    };
    value
        .into_string()
        .map(Some)
        .map_err(|_| format!("{ENV_PASSWORD} is not valid UTF-8"))
}

fn non_empty(value: std::ffi::OsString) -> Option<std::ffi::OsString> {
    if value.is_empty() { None } else { Some(value) }
}

/// Timeout for read-only queries (snapshots/repoinfo).
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);
/// Default timeout for an actual backup/prune/restore. Generous because a
/// first backup or a prune of a large repository legitimately runs for hours;
/// override with [`ENV_TIMEOUT_SECS`] or [`RusticCli::with_timeout`].
const BACKUP_TIMEOUT: Duration = Duration::from_secs(12 * 60 * 60);
/// Timeout override (seconds) for backup/prune/restore runs.
pub const ENV_TIMEOUT_SECS: &str = "APPZ_BACKUP_TIMEOUT_SECS";

fn default_timeout() -> Duration {
    std::env::var(ENV_TIMEOUT_SECS)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|s| *s > 0)
        .map_or(BACKUP_TIMEOUT, Duration::from_secs)
}

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
#[derive(Clone)]
pub struct RusticCli {
    bin: PathBuf,
    repo: PathBuf,
    password: String,
    cwd: PathBuf,
    timeout: Duration,
}

// The password must never reach logs: print everything else.
impl std::fmt::Debug for RusticCli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RusticCli")
            .field("bin", &self.bin)
            .field("repo", &self.repo)
            .field("password", &"<redacted>")
            .field("cwd", &self.cwd)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl RusticCli {
    /// Build a client around an already-resolved binary (see
    /// [`ensure_installed`]). A binary that turns out not to exist is
    /// reported by [`Rustic::run`] as [`EngineError::Missing`].
    pub fn new(
        bin: impl Into<PathBuf>,
        repo: impl Into<PathBuf>,
        password: impl Into<String>,
        cwd: impl Into<PathBuf>,
    ) -> Self {
        Self {
            bin: bin.into(),
            repo: repo.into(),
            password: password.into(),
            cwd: cwd.into(),
            timeout: default_timeout(),
        }
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
    /// rustic reads `RUSTIC_*`. The appz-facing names ([`ENV_REPOSITORY`],
    /// [`ENV_PASSWORD`]) are resolved before construction, and any ambient
    /// `RUSTIC_*` variable is scrubbed from the child (see [`SCRUB_PREFIX`]),
    /// so only the repository and password set here reach rustic.
    fn invocation(&self, args: &[&str]) -> (PathBuf, Vec<String>, Vec<(&'static str, String)>) {
        let bin = self.bin.clone();
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
        if !self.cwd.is_dir() {
            return Err(EngineError::Failed(format!(
                "working directory {} does not exist",
                self.cwd.display()
            )));
        }
        let (bin, all, env) = self.invocation(args);
        let borrowed: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let arg_refs: Vec<&str> = all.iter().map(String::as_str).collect();

        let out = exec_captured_scrubbed(
            &bin.to_string_lossy(),
            &arg_refs,
            &self.cwd,
            &borrowed,
            SCRUB_PREFIX,
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

/// Ambient variables with this prefix are removed from the child's environment.
const SCRUB_PREFIX: &str = "RUSTIC_";

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
    // stderr only, and anchored to rustic's wording: a bare "locked" would
    // also match "blocked by firewall" or a path/tag that contains it.
    let stderr_lower = stderr.to_lowercase();
    if stderr_lower.contains(" is locked")
        || stderr_lower.contains(" locked by ")
        || stderr_lower.contains("repository is in use")
    {
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
        // Windows reports ERROR_PATH_NOT_FOUND (3) when the directory part of
        // the path does not exist, and ERROR_FILE_NOT_FOUND (2) only for a
        // missing leaf — a "rustic not installed" case hits either.
        || lower.contains("os error 3")
        || lower.contains("cannot find the")
        || lower.contains("no such file")
        || lower.contains("not found")
}

/// Locate `rustic` on PATH or in mise's bin dirs (shims are not always
/// activated), with the same lookup `appz deploy` uses for platform CLIs.
pub fn find_rustic(dir: &Path) -> Option<PathBuf> {
    command::shell::find_command_with_paths("rustic", None, &mise_bin_paths(dir))
}

/// Return a usable `rustic` path, installing it through mise first when it is
/// missing and the policy (or the user, when asked) allows. This is the same
/// flow `appz deploy` uses for platform CLIs, so `appz backup` works on a
/// machine that never installed rustic.
pub fn ensure_installed(project_dir: &Path, install: Consent) -> Result<PathBuf, String> {
    if let Some(bin) = find_rustic(project_dir) {
        return Ok(bin);
    }

    confirm_mise_install(
        install,
        "rustic",
        MISE_TOOL,
        &format!("rustic is not installed. Install it with `mise use -g {MISE_TOOL}`"),
    )?;
    ensure_mise(project_dir)?;
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
        RusticCli::new(
            "C:/fake/rustic.exe",
            "C:/repos/abc",
            "hunter2",
            "C:/project",
        )
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
        let cwd = std::env::current_dir().unwrap();
        let cli = RusticCli::new(
            "C:/definitely/not/here/rustic-xyz.exe",
            "C:/repos/abc",
            "pw",
            cwd,
        );
        assert!(matches!(cli.run(&["snapshots"]), Err(EngineError::Missing)));
    }

    #[test]
    fn a_missing_working_directory_is_not_reported_as_a_missing_binary() {
        let cli = RusticCli::new("rustic", "C:/repos/abc", "pw", "C:/no/such/project/dir");
        match cli.run(&["snapshots"]) {
            Err(EngineError::Failed(msg)) => assert!(msg.contains("does not exist"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn lock_detection_ignores_look_alike_words() {
        for stderr in ["access blocked by firewall", "tag unlocked-release failed"] {
            assert!(
                matches!(classify(stderr, "", Some(1)), EngineError::Failed(_)),
                "{stderr}"
            );
        }
        // stdout (snapshot tags, paths) is never scanned for locks.
        assert!(matches!(
            classify("boom", "tag: repository is locked by me", Some(1)),
            EngineError::Failed(_)
        ));
    }

    #[test]
    fn debug_output_redacts_the_password() {
        let shown = format!("{:?}", cli());
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("<redacted>"));
    }
}
