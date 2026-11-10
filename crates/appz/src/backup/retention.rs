//! Retention policy enforcement for rustic backup repositories.
//!
//! Applies retention rules by count, date, and repository size.
//! Retention is applied at most once a day per project unless forced,
//! recording the execution date in `<root>/.appz/backup.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

/// Persistent backup metadata in `<root>/.appz/backup.json`.
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
    if retention.keep_last > 0 {
        args.push("--keep-last".to_string());
        args.push(retention.keep_last.to_string());
    }
    if retention.keep_daily > 0 {
        args.push("--keep-daily".to_string());
        args.push(retention.keep_daily.to_string());
    }
    if retention.keep_weekly > 0 {
        args.push("--keep-weekly".to_string());
        args.push(retention.keep_weekly.to_string());
    }
    args.push("--prune".to_string());
    args
}

fn resolve_state_file(path: &Path) -> PathBuf {
    if path.extension().and_then(|e| e.to_str()) == Some("json") {
        path.to_path_buf()
    } else {
        path.join(".appz").join("backup.json")
    }
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
    let data = serde_json::to_string_pretty(state)?;
    std::fs::write(state_file, data)
}

fn today_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

#[derive(Debug, Clone, Deserialize)]
struct SnapshotEntry {
    id: String,
    #[serde(default)]
    time: Option<String>,
}

fn parse_snapshots(json_str: &str) -> Vec<SnapshotEntry> {
    let val: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    if let Some(arr) = val.as_array() {
        for item in arr {
            if let Some(snaps) = item.get("snapshots").and_then(|s| s.as_array()) {
                for s in snaps {
                    if let Some(id) = s.get("id").and_then(|i| i.as_str()) {
                        out.push(SnapshotEntry {
                            id: id.to_string(),
                            time: s
                                .get("time")
                                .and_then(|t| t.as_str())
                                .map(|t| t.to_string()),
                        });
                    }
                }
            } else if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                out.push(SnapshotEntry {
                    id: id.to_string(),
                    time: item
                        .get("time")
                        .and_then(|t| t.as_str())
                        .map(|t| t.to_string()),
                });
            }
        }
    }

    out.sort_by(|a, b| match (&a.time, &b.time) {
        (Some(ta), Some(tb)) => ta.cmp(tb),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    out
}

fn parse_repo_size(json_str: &str) -> u64 {
    let val: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return 0,
    };

    if let Some(repo_arr) = val
        .get("files")
        .and_then(|f| f.get("repo"))
        .and_then(|r| r.as_array())
    {
        let total: u64 = repo_arr
            .iter()
            .filter_map(|entry| entry.get("size").and_then(|s| s.as_u64()))
            .sum();
        if total > 0 {
            return total;
        }
    }

    if let Some(sz) = val.get("total_size").and_then(|s| s.as_u64()) {
        return sz;
    }

    if let Some(sz) = val.get("repo_size").and_then(|s| s.as_u64()) {
        return sz;
    }

    if let Some(sz) = val.get("size").and_then(|s| s.as_u64()) {
        return sz;
    }

    0
}

/// Apply retention policy to a rustic backup repository.
///
/// Runs at most once a day unless the state records no run for today.
/// If `max_size` is exceeded, progressively forgets the oldest snapshots
/// until under capacity or reaching `min_keep`.
pub fn apply(
    engine: &impl Rustic,
    retention: &Retention,
    state_target: impl AsRef<Path>,
) -> Result<RetentionReport, EngineError> {
    let state_file = resolve_state_file(state_target.as_ref());
    let mut state = read_state(&state_file);
    let today = today_utc();

    if state.last_retention.as_deref() == Some(&today) {
        let size = query_repo_size(engine).unwrap_or(state.last_size.unwrap_or(0));
        return Ok(RetentionReport {
            ran: false,
            size,
            over_cap: false,
            removed: 0,
        });
    }

    let f_args = forget_args(retention);
    let str_args: Vec<&str> = f_args.iter().map(|s| s.as_str()).collect();
    engine.run(&str_args)?;

    let mut current_size = query_repo_size(engine).unwrap_or(0);
    let mut removed = 0;
    let mut over_cap = false;

    if let Some(max_bytes) = retention
        .max_size
        .as_deref()
        .and_then(|s| parse_size(s).ok())
    {
        let snaps_out = engine.run(&["snapshots", "--json"])?;
        let mut snapshots = parse_snapshots(&snaps_out.stdout);

        while current_size > max_bytes && snapshots.len() > retention.min_keep as usize {
            let oldest = snapshots.remove(0);
            engine.run(&["forget", &oldest.id, "--prune"])?;
            removed += 1;
            current_size = query_repo_size(engine).unwrap_or(current_size);
        }

        if current_size > max_bytes && snapshots.len() <= retention.min_keep as usize {
            over_cap = true;
        }
    }

    state.last_retention = Some(today);
    state.last_size = Some(current_size);
    let _ = write_state(&state_file, &state);

    Ok(RetentionReport {
        ran: true,
        size: current_size,
        over_cap,
        removed,
    })
}

fn query_repo_size(engine: &impl Rustic) -> Result<u64, EngineError> {
    let out = engine.run(&["repoinfo", "--json"])?;
    Ok(parse_repo_size(&out.stdout))
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

            if args.len() >= 3 && args[0] == "forget" && args[2] == "--prune" {
                let id = args[1];
                self.snapshots.borrow_mut().retain(|(i, _)| i != id);
                let current = *self.size.borrow();
                let new_size = current.saturating_sub(self.size_reduction_per_removal);
                *self.size.borrow_mut() = new_size;
                return Ok(RunOut {
                    stdout: String::new(),
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

        assert!(args.contains(&"--prune".to_string()));
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

        let report1 = apply(&engine, &retention, dir.path()).unwrap();
        assert!(report1.ran);
        assert_eq!(report1.size, 1000);
        assert_eq!(report1.removed, 0);

        let initial_calls = engine.calls.borrow().len();

        let report2 = apply(&engine, &retention, dir.path()).unwrap();
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

        let report = apply(&engine, &retention, dir.path()).unwrap();
        assert!(report.ran);
        assert_eq!(report.removed, 2);
        assert!(!report.over_cap);
        assert!(report.size <= 2 * one_gib);

        // Verify the oldest snapshots (snap1, then snap2) were the ones forgotten
        let forgotten_ids: Vec<String> = engine
            .calls
            .borrow()
            .iter()
            .filter(|c| c.len() >= 3 && c[0] == "forget" && c[2] == "--prune")
            .map(|c| c[1].clone())
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
            min_keep: 3,
            ..Default::default()
        };

        let report = apply(&engine, &retention, dir.path()).unwrap();
        assert!(report.ran);
        // Only 1 snapshot can be removed before hitting min_keep (4 -> 3)
        assert_eq!(report.removed, 1);
        assert!(report.over_cap);
        assert!(report.size > 2 * one_gib);
    }
}
