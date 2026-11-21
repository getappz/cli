//! Retention policy enforcement for rustic backup repositories.
//!
//! Applies retention rules by count, date, and repository size.
//! Retention is applied at most once a day per project and repository.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::engine::{EngineError, Rustic};

/// Default number of recent snapshots to keep.
pub const DEFAULT_KEEP_LAST: u32 = 10;
/// Default number of daily snapshots to keep.
pub const DEFAULT_KEEP_DAILY: u32 = 14;
/// Default number of weekly snapshots to keep.
pub const DEFAULT_KEEP_WEEKLY: u32 = 8;
/// Default repository size limit.
pub const DEFAULT_MAX_SIZE: &str = "2GiB";
/// Default minimum number of snapshots to retain regardless of size cap.
pub const DEFAULT_MIN_KEEP: u32 = 3;

fn default_keep_last() -> u32 {
    DEFAULT_KEEP_LAST
}

fn default_keep_daily() -> u32 {
    DEFAULT_KEEP_DAILY
}

fn default_keep_weekly() -> u32 {
    DEFAULT_KEEP_WEEKLY
}

fn default_max_size() -> Option<String> {
    Some(DEFAULT_MAX_SIZE.to_string())
}

fn default_min_keep() -> u32 {
    DEFAULT_MIN_KEEP
}

/// Retention configuration from `appz.jsonc` under the "backup.retention" key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retention {
    #[serde(default = "default_keep_last")]
    pub keep_last: u32,
    #[serde(default = "default_keep_daily")]
    pub keep_daily: u32,
    #[serde(default = "default_keep_weekly")]
    pub keep_weekly: u32,
    #[serde(default = "default_max_size")]
    pub max_size: Option<String>,
    #[serde(default = "default_min_keep")]
    pub min_keep: u32,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            keep_last: DEFAULT_KEEP_LAST,
            keep_daily: DEFAULT_KEEP_DAILY,
            keep_weekly: DEFAULT_KEEP_WEEKLY,
            max_size: default_max_size(),
            min_keep: DEFAULT_MIN_KEEP,
        }
    }
}

/// Outcome report of applying retention rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionReport {
    /// Whether retention logic ran (false if skipped because it already ran today).
    pub ran: bool,
    /// Repository size in bytes after retention.
    pub size: u64,
    /// True if repository size exceeds `max_size` even after stopping at `min_keep`.
    pub over_cap: bool,
    /// Number of snapshots removed during the retention run.
    pub removed: usize,
}

/// Persistent backup metadata for one repository.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackupState {
    pub last_retention: Option<String>,
    pub last_size: Option<u64>,
}

/// Parse human-readable byte sizes (e.g., "2GiB", "500MB", "1024", "10KiB").
pub fn parse_size(input: &str) -> Result<u64, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("size string is empty".to_string());
    }

    let end_of_digits = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());

    let (num_part, unit_part) = s.split_at(end_of_digits);
    let num_part = num_part.trim();
    let unit_part = unit_part.trim().to_ascii_lowercase();

    let num: f64 = num_part
        .parse::<f64>()
        .map_err(|e| format!("invalid number '{num_part}': {e}"))?;

    let multiplier: f64 = match unit_part.as_str() {
        "" | "b" | "bytes" => 1.0,
        "k" | "kb" => 1_000.0,
        "kib" => 1024.0,
        "m" | "mb" => 1_000_000.0,
        "mib" => 1024.0 * 1024.0,
        "g" | "gb" => 1_000_000_000.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "t" | "tb" => 1_000_000_000_000.0,
        "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        other => return Err(format!("unknown size unit '{other}'")),
    };

    let total = num * multiplier;
    if total < 0.0 || total > u64::MAX as f64 {
        return Err("size out of range".to_string());
    }

    Ok(total as u64)
}

/// Build rustic forget CLI arguments based on retention policy.
pub fn forget_args(retention: &Retention) -> Vec<String> {
    let mut args = vec!["forget".to_string()];
    let keep_last = retention.keep_last.max(retention.min_keep);
    if keep_last > 0 {
        args.push("--keep-last".to_string());
        args.push(keep_last.to_string());
    }
    if retention.keep_daily > 0 {
        args.push("--keep-daily".to_string());
        args.push(retention.keep_daily.to_string());
    }
    if retention.keep_weekly > 0 {
        args.push("--keep-weekly".to_string());
        args.push(retention.keep_weekly.to_string());
    }
    args.push("--dry-run".to_string());
    args.push("--json".to_string());
    args
}

fn state_file(project: &Path, repository: &str) -> PathBuf {
    let key = Sha256::digest(repository.as_bytes());
    project.join(".appz").join(format!("backup-{key:x}.json"))
}

fn read_state(state_file: &Path) -> BackupState {
    if let Ok(raw) = std::fs::read_to_string(state_file) {
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        BackupState::default()
    }
}

fn write_state(state_file: &Path, state: &BackupState) -> std::io::Result<()> {
    if let Some(parent) = state_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = tempfile::NamedTempFile::new_in(state_file.parent().unwrap())?;
    serde_json::to_writer_pretty(file.as_file(), state)?;
    file.persist(state_file)?;
    Ok(())
}

fn today_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

#[derive(Deserialize)]
struct SnapshotEntry {
    id: String,
    time: chrono::DateTime<chrono::FixedOffset>,
}

fn parse_snapshots(output: &str) -> Result<Vec<SnapshotEntry>, EngineError> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Entry {
        Group { snapshots: Vec<SnapshotEntry> },
        Single(SnapshotEntry),
    }
    let entries: Vec<Entry> = parse_output("snapshots", output)?;
    let mut snapshots = Vec::new();
    for entry in entries {
        match entry {
            Entry::Group { snapshots: group } => snapshots.extend(group),
            Entry::Single(snapshot) => snapshots.push(snapshot),
        }
    }
    snapshots.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.time));
    Ok(snapshots)
}

fn parse_output<T: serde::de::DeserializeOwned>(
    command: &str,
    output: &str,
) -> Result<T, EngineError> {
    serde_json::from_str(output)
        .map_err(|e| EngineError::Failed(format!("invalid Rustic {command} output: {e}")))
}

#[derive(Deserialize)]
struct ForgetItem {
    snapshot: SnapshotEntry,
    keep: bool,
}

fn forget_items(output: &str) -> Result<Vec<ForgetItem>, EngineError> {
    #[derive(Deserialize)]
    struct Group {
        items: Vec<ForgetItem>,
    }
    let groups: Vec<Group> = parse_output("forget", output)?;
    Ok(groups.into_iter().flat_map(|g| g.items).collect())
}

fn forget_ids<E: Rustic + ?Sized>(engine: &E, ids: &[&str]) -> Result<usize, EngineError> {
    let mut args = vec!["forget", "--json", "--"];
    args.extend_from_slice(ids);
    Ok(forget_items(&engine.run(&args)?.stdout)?
        .iter()
        .filter(|i| !i.keep)
        .count())
}

struct MaintenanceLock(std::fs::File);

impl Drop for MaintenanceLock {
    fn drop(&mut self) {
        // A concurrently forked child can briefly retain a duplicate descriptor.
        let _ = self.0.unlock();
    }
}

// All appz pruners of a local repository share the same OS lock, across projects.
// Remote repositories require coordination with other machines and direct Rustic users.
fn maintenance_lock(project: &Path, repository: &str) -> std::io::Result<MaintenanceLock> {
    let local = Path::new(repository.strip_prefix("local:").unwrap_or(repository));
    let lock_path = if local.is_absolute() {
        local.join(".appz-prune.lock")
    } else {
        let dir = dirs::cache_dir()
            .unwrap_or_else(|| project.join(".appz"))
            .join("appz/backup");
        std::fs::create_dir_all(&dir)?;
        dir.join(format!("{:x}.lock", Sha256::digest(repository.as_bytes())))
    };
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    file.try_lock().map_err(std::io::Error::from)?;
    Ok(MaintenanceLock(file))
}

/// Apply retention policy to a rustic backup repository.
///
/// Runs at most once a day unless the state records no run for today.
/// If `max_size` is exceeded, progressively forgets the oldest snapshots
/// until under capacity or reaching `min_keep`.
pub fn apply<E: Rustic + ?Sized>(
    engine: &E,
    retention: &Retention,
    project: &Path,
    repository: &str,
) -> Result<RetentionReport, EngineError> {
    let state_file = state_file(project, repository);
    let _lock = maintenance_lock(project, repository)
        .map_err(|e| EngineError::Failed(format!("cannot lock repository for retention: {e}")))?;
    let mut state = read_state(&state_file);
    let today = today_utc();
    let max_bytes = retention
        .max_size
        .as_deref()
        .map(parse_size)
        .transpose()
        .map_err(EngineError::Failed)?;

    if state.last_retention.as_deref() == Some(&today) {
        let size = query_repo_size(engine)?;
        return Ok(RetentionReport {
            ran: false,
            size,
            over_cap: max_bytes.is_some_and(|cap| size > cap),
            removed: 0,
        });
    }

    let plan_args = forget_args(retention);
    let args: Vec<_> = plan_args.iter().map(String::as_str).collect();
    let mut plan = forget_items(&engine.run(&args)?.stdout)?;
    let floor = retention.min_keep as usize;
    let budget = plan.len().saturating_sub(floor);
    plan.sort_by_key(|item| (item.snapshot.time, item.snapshot.id.clone()));
    // Expiration takes precedence over Rustic's keep rules. Cap the explicit plan
    // before executing even when expired snapshots exceed the deletion budget.
    let candidates: Vec<_> = plan
        .iter()
        .filter(|item| !item.keep)
        .take(budget)
        .map(|item| item.snapshot.id.as_str())
        .collect();
    let mut removed = 0;
    for ids in candidates.chunks(32) {
        removed += forget_ids(engine, ids)?;
    }
    engine.run(&["prune"])?;

    let mut current_size = query_repo_size(engine)?;
    if let Some(cap) = max_bytes {
        let mut protected = std::collections::HashSet::new();
        while current_size > cap {
            let out = engine.run(&["snapshots", "--json"])?;
            let snapshots = parse_snapshots(&out.stdout)?;
            current_size = query_repo_size(engine)?;
            if current_size <= cap || snapshots.len() <= floor {
                break;
            }
            let Some(oldest) = snapshots.iter().rev().find(|s| !protected.contains(&s.id)) else {
                break;
            };
            if forget_ids(engine, &[&oldest.id])? == 0 {
                protected.insert(oldest.id.clone());
            } else {
                removed += 1;
                engine.run(&["prune"])?;
                current_size = query_repo_size(engine)?;
            }
        }
    }
    let over_cap = max_bytes.is_some_and(|cap| current_size > cap);

    state.last_retention = Some(today);
    state.last_size = Some(current_size);
    write_state(&state_file, &state).map_err(|e| {
        EngineError::Failed(format!(
            "retention applied but could not save {}: {e}",
            state_file.display()
        ))
    })?;

    Ok(RetentionReport {
        ran: true,
        size: current_size,
        over_cap,
        removed,
    })
}

pub(super) fn query_repo_size<E: Rustic + ?Sized>(engine: &E) -> Result<u64, EngineError> {
    let out = engine.run(&["repoinfo", "--json"])?;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Info {
        Files {
            files: Files,
        },
        Total {
            #[serde(alias = "repo_size", alias = "size")]
            total_size: u64,
        },
    }
    #[derive(Deserialize)]
    struct Files {
        repo: Vec<FileStats>,
    }
    #[derive(Deserialize)]
    struct FileStats {
        size: u64,
    }
    match parse_output("repoinfo", &out.stdout)? {
        Info::Total { total_size } => Ok(total_size),
        Info::Files { files } => files.repo.into_iter().try_fold(0_u64, |sum, entry| {
            sum.checked_add(entry.size)
                .ok_or_else(|| EngineError::Failed("Rustic repository size exceeds u64".into()))
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use tempfile::tempdir;

    use super::*;
    use crate::backup::engine::RunOut;

    struct FakeRustic {
        calls: RefCell<Vec<Vec<String>>>,
        size: RefCell<u64>,
        snapshots: RefCell<Vec<(String, String)>>,
        size_reduction_per_removal: u64,
    }

    impl FakeRustic {
        fn new(initial_size: u64, snapshots: Vec<(&str, &str)>) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                size: RefCell::new(initial_size),
                snapshots: RefCell::new(
                    snapshots
                        .into_iter()
                        .map(|(id, t)| (id.to_string(), t.to_string()))
                        .collect(),
                ),
                size_reduction_per_removal: 0,
            }
        }

        fn with_reduction(mut self, bytes_per_snap: u64) -> Self {
            self.size_reduction_per_removal = bytes_per_snap;
            self
        }
    }

    impl Rustic for FakeRustic {
        fn run(&self, args: &[&str]) -> Result<RunOut, EngineError> {
            let str_vec: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            self.calls.borrow_mut().push(str_vec);

            if args == ["repoinfo", "--json"] {
                let sz = *self.size.borrow();
                let json = serde_json::json!({
                    "files": {
                        "repo": [
                            { "type": "data", "size": sz }
                        ]
                    }
                });
                return Ok(RunOut {
                    stdout: json.to_string(),
                    stderr: String::new(),
                });
            }

            if args == ["snapshots", "--json"] {
                let list: Vec<serde_json::Value> = self
                    .snapshots
                    .borrow()
                    .iter()
                    .map(|(id, t)| {
                        serde_json::json!({
                            "id": id,
                            "time": t,
                        })
                    })
                    .collect();
                return Ok(RunOut {
                    stdout: serde_json::Value::Array(list).to_string(),
                    stderr: String::new(),
                });
            }

            if args.first() == Some(&"forget") {
                let planning = args.contains(&"--dry-run");
                let mut snapshots = self.snapshots.borrow_mut();
                let mut items = Vec::new();
                snapshots.retain(|(id, time)| {
                    if !planning && !args[3..].contains(&id.as_str()) {
                        return true;
                    }
                    let keep =
                        id.starts_with("protected") || (planning && !id.starts_with("expired"));
                    items.push(serde_json::json!({"snapshot":{"id":id, "time":time},"keep":keep}));
                    if !planning && !keep {
                        let size = *self.size.borrow();
                        *self.size.borrow_mut() =
                            size.saturating_sub(self.size_reduction_per_removal);
                    }
                    planning || keep
                });
                return Ok(RunOut {
                    stdout: serde_json::json!([{"items": items}]).to_string(),
                    stderr: String::new(),
                });
            }

            Ok(RunOut {
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }

    #[test]
    fn snapshot_order_uses_the_instant_across_time_zones() {
        let output = r#"[{"snapshots":[{"id":"older","time":"2026-01-02T00:00:00+05:30"},{"id":"newer","time":"2026-01-01T23:00:00Z"}]}]"#;
        assert_eq!(parse_snapshots(output).unwrap().pop().unwrap().id, "older");
        assert!(parse_snapshots(r#"[{"id":"missing-time"}]"#).is_err());
    }

    #[test]
    fn forget_args_follow_the_defaults() {
        let ret = Retention::default();
        let args = forget_args(&ret);

        assert_eq!(args[0], "forget");

        let pos_last = args.iter().position(|a| a == "--keep-last").unwrap();
        assert_eq!(args[pos_last + 1], "10");

        let pos_daily = args.iter().position(|a| a == "--keep-daily").unwrap();
        assert_eq!(args[pos_daily + 1], "14");

        let pos_weekly = args.iter().position(|a| a == "--keep-weekly").unwrap();
        assert_eq!(args[pos_weekly + 1], "8");

        assert!(args.contains(&"--dry-run".to_string()));
    }

    #[test]
    fn parse_size_units() {
        assert_eq!(parse_size("1024").unwrap(), 1024);
        assert_eq!(parse_size("1024 B").unwrap(), 1024);
        assert_eq!(parse_size("500KB").unwrap(), 500_000);
        assert_eq!(parse_size("500KiB").unwrap(), 500 * 1024);
        assert_eq!(parse_size("10MB").unwrap(), 10_000_000);
        assert_eq!(parse_size("10MiB").unwrap(), 10 * 1024 * 1024);
        assert_eq!(parse_size("2GB").unwrap(), 2_000_000_000);
        assert_eq!(parse_size("2GiB").unwrap(), 2 * 1024 * 1024 * 1024);
        assert_eq!(parse_size(" 2 gib ").unwrap(), 2 * 1024 * 1024 * 1024);
        assert!(parse_size("").is_err());
        assert!(parse_size("abc").is_err());
        assert!(parse_size("10xyz").is_err());
    }

    #[test]
    fn skips_when_ran_today() {
        let dir = tempdir().unwrap();
        let engine = FakeRustic::new(1000, vec![("snap1", "2026-01-01T00:00:00Z")]);
        let retention = Retention::default();

        let a = tempdir().unwrap();
        let b = tempdir().unwrap();
        let repo_a = a.path().to_str().unwrap();
        let repo_b = b.path().to_str().unwrap();
        let report1 = apply(&engine, &retention, dir.path(), repo_a).unwrap();
        assert!(report1.ran);
        assert_eq!(report1.size, 1000);
        assert_eq!(report1.removed, 0);

        let initial_calls = engine.calls.borrow().len();

        let report2 = apply(&engine, &retention, dir.path(), repo_a).unwrap();
        assert!(!report2.ran);
        assert_eq!(report2.size, 1000);
        assert_eq!(report2.removed, 0);

        // No forget command should have been executed in the second run
        let calls_after = engine.calls.borrow();
        assert!(
            !calls_after[initial_calls..]
                .iter()
                .any(|call| call.first().map(|s| s.as_str()) == Some("forget"))
        );
        drop(calls_after);
        assert!(apply(&engine, &retention, dir.path(), repo_b).unwrap().ran);
        assert!(!apply(&engine, &retention, dir.path(), repo_a).unwrap().ran);
    }

    #[test]
    fn expired_and_protected_snapshots_obey_the_floor() {
        let dir = tempdir().unwrap();
        let project = tempdir().unwrap();
        let repo = dir.path().to_str().unwrap();
        let engine = FakeRustic::new(
            1000,
            vec![
                ("protected", "2025-01-01T00:00:00Z"),
                ("expired1", "2025-01-02T00:00:00Z"),
                ("expired2", "2025-01-03T00:00:00Z"),
                ("expired3", "2025-01-04T00:00:00Z"),
            ],
        );
        let policy = Retention {
            max_size: Some("1B".into()),
            min_keep: 3,
            ..Default::default()
        };
        let lock = maintenance_lock(project.path(), repo).unwrap();
        assert!(apply(&engine, &policy, project.path(), repo).is_err());
        drop(lock);
        let report = apply(&engine, &policy, project.path(), repo).unwrap();
        assert_eq!(report.removed, 1);
        assert_eq!(engine.snapshots.borrow().len(), 3);
        assert!(report.over_cap);
    }

    #[test]
    fn size_pressure_skips_protected_snapshots_without_counting_them() {
        let dir = tempdir().unwrap();
        let engine = FakeRustic::new(
            1000,
            vec![
                ("protected", "2025-01-01T00:00:00Z"),
                ("eligible", "2025-01-02T00:00:00Z"),
                ("newest", "2025-01-03T00:00:00Z"),
            ],
        )
        .with_reduction(500);
        let policy = Retention {
            max_size: Some("500B".into()),
            min_keep: 2,
            ..Default::default()
        };
        let report = apply(&engine, &policy, dir.path(), dir.path().to_str().unwrap()).unwrap();
        assert_eq!(report.removed, 1);
        assert!(!report.over_cap);
        assert!(
            engine
                .snapshots
                .borrow()
                .iter()
                .all(|(id, _)| id != "eligible")
        );
    }

    #[test]
    fn size_loop_removes_oldest_until_under_cap() {
        let dir = tempdir().unwrap();
        // 3 GiB initial size, 4 snapshots. Each snapshot drops size by 600 MiB.
        // Cap is 2 GiB (~2.147 GB). 3.0 GiB - 600 MiB = 2.4 GiB (still over),
        // next removal drops to 1.8 GiB (under cap).
        let one_gib = 1024 * 1024 * 1024;
        let initial_size = 3 * one_gib;
        let reduction = 600 * 1024 * 1024;

        let engine = FakeRustic::new(
            initial_size,
            vec![
                ("snap1", "2026-01-01T00:00:00Z"),
                ("snap2", "2026-01-02T00:00:00Z"),
                ("snap3", "2026-01-03T00:00:00Z"),
                ("snap4", "2026-01-04T00:00:00Z"),
            ],
        )
        .with_reduction(reduction);

        let retention = Retention {
            max_size: Some("2GiB".to_string()),
            min_keep: 1,
            ..Default::default()
        };

        let report = apply(
            &engine,
            &retention,
            dir.path(),
            dir.path().to_str().unwrap(),
        )
        .unwrap();
        assert!(report.ran);
        assert_eq!(report.removed, 2);
        assert!(!report.over_cap);
        assert!(report.size <= 2 * one_gib);

        // Verify the oldest snapshots (snap1, then snap2) were the ones forgotten
        let forgotten_ids: Vec<String> = engine
            .calls
            .borrow()
            .iter()
            .filter(|c| c.starts_with(&["forget".into(), "--json".into(), "--".into()]))
            .map(|c| c[3].clone())
            .collect();

        assert_eq!(forgotten_ids, vec!["snap1", "snap2"]);
    }

    #[test]
    fn size_loop_stops_at_min_keep_and_reports_over_cap() {
        let dir = tempdir().unwrap();
        let one_gib = 1024 * 1024 * 1024;
        let initial_size = 3 * one_gib;
        // Each removal only frees 100 bytes - cannot get under 2 GiB
        let engine = FakeRustic::new(
            initial_size,
            vec![
                ("snap1", "2026-01-01T00:00:00Z"),
                ("snap2", "2026-01-02T00:00:00Z"),
                ("snap3", "2026-01-03T00:00:00Z"),
                ("snap4", "2026-01-04T00:00:00Z"),
            ],
        )
        .with_reduction(100);

        let retention = Retention {
            max_size: Some("2GiB".to_string()),
            keep_last: 1,
            min_keep: 3,
            ..Default::default()
        };

        let report = apply(
            &engine,
            &retention,
            dir.path(),
            dir.path().to_str().unwrap(),
        )
        .unwrap();
        assert!(
            engine.calls.borrow()[0]
                .windows(2)
                .any(|args| args == ["--keep-last", "3"])
        );
        assert!(report.ran);
        // Only 1 snapshot can be removed before hitting min_keep (4 -> 3)
        assert_eq!(report.removed, 1);
        assert!(report.over_cap);
        assert!(report.size > 2 * one_gib);
        assert!(
            apply(
                &engine,
                &retention,
                dir.path(),
                dir.path().to_str().unwrap()
            )
            .unwrap()
            .over_cap
        );
    }
}
