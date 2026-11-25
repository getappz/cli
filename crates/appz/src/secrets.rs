//! `appz env` storage and deploy-time lookup, on top of `appz-vault`.
//! Global vault `~/.appz/vault.json`, project vault `<root>/.appz/vault.json`.

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

use appz_vault::{
    ExpiryStatus, SecretEntry, SecretExpiry, VaultError, VaultPaths, create_vault,
    get_secret_value, open_vault, open_vault_with_dek, read_vault_body, remove_secret_value,
    secret_expiries, set_secret_expiry, set_secret_value, update_vault_body,
};

const APP: &str = "appz";
const PASSPHRASE_VAR: &str = appz_vault::PASSPHRASE_ENV;

/// Vault locations for the project containing `dir`. `dir` is canonicalized
/// first so `appz env` (run from `.` or a subfolder) and `appz deploy` (which
/// has the canonical project dir) land on the SAME vault path. The keyring
/// session is keyed by a hash of that path, so any difference would make an
/// unlock invisible to deploy.
fn paths(dir: &Path) -> VaultPaths {
    let dir = appz_core::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    VaultPaths::project(APP, &dir, |p| {
        p.join(".appz").exists() || p.join(".git").exists() || p.join("appz.jsonc").exists()
    })
}

/// The vault file for `--global` or the project containing `dir`.
pub fn vault_path(dir: &Path, global: bool) -> Result<PathBuf, String> {
    let p = paths(dir);
    if global {
        Ok(p.global_vault_path())
    } else {
        p.project_vault_path().ok_or_else(|| {
            "not inside a project (no .git, .appz or appz.jsonc); use --global".into()
        })
    }
}

fn passphrase(prompt: &str) -> Result<String, String> {
    if let Some(pw) = std::env::var(PASSPHRASE_VAR).ok().filter(|p| !p.is_empty()) {
        return Ok(pw);
    }
    if std::io::stdin().is_terminal() {
        return cliclack::password(prompt)
            .interact()
            .map_err(|e| e.to_string());
    }
    Err(format!(
        "vault is locked: run appz env unlock (or set {PASSPHRASE_VAR})"
    ))
}

/// A project vault lives in `<root>/.appz/`, which appz keeps git-ignored.
fn gitignore_project_vault(vault: &Path) {
    if let Some(root) = vault.parent().and_then(Path::parent)
        && root.ancestors().any(|a| a.join(".git").exists())
        && let Err(e) = appz_core::ensure_gitignored(root)
    {
        crate::warning(&format!("Could not git-ignore the project vault: {e}"));
    }
}

/// Unlock `path` (creating it on first use) and cache the key in the OS
/// keyring. The `bool` is whether the key was actually cached: `false` means
/// the keyring is unavailable and only this process is unlocked.
pub fn unlock(path: &Path) -> Result<([u8; 32], bool), String> {
    let e = |e: VaultError| e.to_string();
    if !path.exists() {
        let pw = passphrase("New vault passphrase")?;
        if std::env::var_os(PASSPHRASE_VAR).is_none() {
            let again = passphrase("Repeat passphrase")?;
            if again != pw {
                return Err("passphrases do not match".into());
            }
        }
        std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?;
        match create_vault(path, &pw) {
            Ok(()) => gitignore_project_vault(path),
            // Another process created it between our check and now: use theirs.
            Err(VaultError::AlreadyInitialized(_)) => {}
            Err(err) => return Err(err.to_string()),
        }
    }
    let dek = open_vault(path, &passphrase("Vault passphrase")?)
        .map_err(e)?
        .dek;
    let cached = appz_vault::session::store_session(APP, path, &dek);
    Ok((dek, cached))
}

pub fn lock(path: &Path) {
    appz_vault::session::clear_session(APP, path);
}

fn dek_for(path: &Path) -> Result<[u8; 32], String> {
    match open_vault_with_dek(path, APP) {
        Ok(d) if path.exists() => Ok(d.dek),
        _ => unlock(path).map(|(dek, _)| dek),
    }
}

pub fn valid_name(name: &str) -> bool {
    let mut c = name.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphabetic() || f == '_')
        && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// `value` of `None` is read from a hidden prompt, or stdin when piped, so it
/// stays out of shell history.
pub fn set(
    path: &Path,
    name: &str,
    value: Option<String>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<(), String> {
    if !valid_name(name) {
        return Err(format!("'{name}' is not a valid variable name"));
    }
    let value = match value {
        Some(v) => v,
        None if std::io::stdin().is_terminal() => cliclack::password(format!("Value for {name}"))
            .interact()
            .map_err(|e| e.to_string())?,
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| e.to_string())?;
            s.trim_end_matches(['\r', '\n']).to_string()
        }
    };
    let dek = dek_for(path)?;
    update_vault_body(path, |b| {
        set_secret_value(b, &dek, name, &value)?;
        set_secret_expiry(b, name, expires_at)
    })
    .map_err(|e| e.to_string())
}

/// Change a stored secret's recorded expiry; its value is not touched, so no
/// unlock is needed.
pub fn set_expiry(
    path: &Path,
    name: &str,
    expires_at: Option<DateTime<Utc>>,
) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("{name} is not stored"));
    }
    update_vault_body(path, |b| set_secret_expiry(b, name, expires_at)).map_err(|e| e.to_string())
}

/// Every secret's recorded expiry and status. Needs no unlock.
pub fn expiries(path: &Path, within: Duration) -> Result<Vec<SecretExpiry>, String> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let body = read_vault_body(path).map_err(|e| e.to_string())?;
    Ok(secret_expiries(&body, Utc::now(), within))
}

/// `--expires 2027-02-18` (that day, 00:00 UTC) or `--ttl 90d` from `now`.
pub fn parse_expiry(
    expires: Option<&str>,
    ttl: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, String> {
    match (expires, ttl) {
        (Some(_), Some(_)) => Err("use either --expires or --ttl, not both".into()),
        (Some(d), None) => {
            let day = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_err(|_| format!("'{d}' is not a date; use YYYY-MM-DD"))?;
            Ok(Some(day.and_hms_opt(0, 0, 0).unwrap().and_utc()))
        }
        (None, Some(t)) => Ok(Some(now + parse_duration(t)?)),
        (None, None) => Ok(None),
    }
}

/// `90d`, `2w` or `12h`.
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let bad = || format!("'{s}' is not a duration; use e.g. 90d, 2w or 12h");
    let (num, unit) = s.split_at(s.len().saturating_sub(1));
    let n: i64 = num.parse().map_err(|_| bad())?;
    match unit {
        "d" => Ok(Duration::days(n)),
        "w" => Ok(Duration::weeks(n)),
        "h" => Ok(Duration::hours(n)),
        _ => Err(bad()),
    }
}

/// Every secret name stored in the global or project vault for `project_dir`.
/// Reads only the unencrypted key index (never prompts or decrypts values).
pub(crate) fn known_names(project_dir: &Path) -> std::collections::BTreeSet<String> {
    let p = paths(project_dir);
    let mut names = std::collections::BTreeSet::new();
    for path in std::iter::once(p.global_vault_path())
        .chain(p.project_vault_path())
        .filter(|f| f.exists())
    {
        if let Ok(body) = read_vault_body(&path) {
            names.extend(body.keys().cloned());
        }
    }
    names
}

pub fn remove(path: &Path, name: &str) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    update_vault_body(path, |b| remove_secret_value(b, name)).map_err(|e| e.to_string())
}

#[derive(Clone)]
pub(crate) enum SecretStatus {
    Missing,
    Locked,
    Found(String),
}

/// Read one secret from `path` using the active keyring session (never prompts),
/// distinguishing missing secrets from a locked vault.
pub(crate) fn get_status(path: &Path, name: &str) -> Result<SecretStatus, String> {
    if !valid_name(name) {
        return Err(format!("'{name}' is not a valid variable name"));
    }
    if !path.exists() {
        return Ok(SecretStatus::Missing);
    }
    let dek = match open_vault_with_dek(path, APP) {
        Ok(d) => d.dek,
        Err(VaultError::Locked) => {
            if let Some(pw) = std::env::var(PASSPHRASE_VAR).ok().filter(|p| !p.is_empty()) {
                open_vault(path, &pw).map_err(|e| e.to_string())?.dek
            } else {
                return Ok(SecretStatus::Locked);
            }
        }
        Err(e) => return Err(e.to_string()),
    };
    let body = read_vault_body(path).map_err(|e| e.to_string())?;
    match get_secret_value(&body, &dek, name).map_err(|e| e.to_string())? {
        Some(v) => Ok(SecretStatus::Found(v.to_string())),
        None => Ok(SecretStatus::Missing),
    }
}

/// Read one secret value from `path` using an already-unlocked vault session.
#[allow(dead_code)]
pub fn get(path: &Path, name: &str) -> Result<Option<String>, String> {
    match get_status(path, name)? {
        SecretStatus::Found(v) => Ok(Some(v)),
        SecretStatus::Missing => Ok(None),
        SecretStatus::Locked => Err(VaultError::Locked.to_string()),
    }
}

pub struct VaultEnv {
    pub vars: HashMap<String, String>,
    /// A vault exists but is not unlocked in this session.
    pub locked: bool,
}

/// Values for exactly the names in `allowed`, project vault over global.
/// Never prompts; a locked vault is reported, not fatal.
pub fn deploy_env(project_dir: &Path, allowed: &[String]) -> Result<VaultEnv, String> {
    let p = paths(project_dir);
    let mut out = VaultEnv {
        vars: HashMap::new(),
        locked: false,
    };
    let files = std::iter::once(p.global_vault_path()).chain(p.project_vault_path());
    for path in files.filter(|f| f.exists()) {
        let dek = match open_vault_with_dek(&path, APP) {
            Ok(d) => d.dek,
            Err(VaultError::Locked) => {
                out.locked = true;
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let body = read_vault_body(&path).map_err(|e| e.to_string())?;
        for name in allowed {
            if let Some(v) = get_secret_value(&body, &dek, name).map_err(|e| e.to_string())? {
                if !out.vars.contains_key(name)
                    && let Some(notice) = body
                        .get(name)
                        .and_then(|e| expiry_notice(name, e, Utc::now()))
                {
                    crate::warning(&notice);
                }
                out.vars.insert(name.clone(), v.to_string());
            }
        }
    }
    Ok(out)
}

/// How soon before expiry a stored secret starts to warn when it is used.
const USE_WARN_WINDOW_DAYS: i64 = 14;

/// The warning to print when `entry` is about to be used, if it is expired or
/// expiring soon.
fn expiry_notice(name: &str, entry: &SecretEntry, now: DateTime<Utc>) -> Option<String> {
    let at = entry.expires_at?;
    match entry.expiry_status(now, Duration::days(USE_WARN_WINDOW_DAYS)) {
        ExpiryStatus::Expired => Some(format!(
            "{name} expired on {}: the provider will likely reject it. Rotate it, then `appz vault set {name} --expires YYYY-MM-DD`",
            at.format("%Y-%m-%d")
        )),
        ExpiryStatus::Expiring => Some(format!(
            "{name} expires in {}d ({}): rotate it soon, then `appz vault set {name} --expires YYYY-MM-DD`",
            (at - now).num_days(),
            at.format("%Y-%m-%d")
        )),
        _ => None,
    }
}

/// Resolve `rel_file` strictly within `root`, rejecting absolute paths,
/// parent traversal (`..`), NUL bytes, and symlinks pointing outside `root`.
pub fn resolve_project_file(root: &Path, rel_file: impl AsRef<Path>) -> Result<PathBuf, String> {
    let rel = rel_file.as_ref();
    let rel_display = rel.display();
    if rel.as_os_str().is_empty() || rel.to_string_lossy().contains('\0') {
        return Err("target file path cannot be empty or contain NUL".into());
    }
    if rel.is_absolute() {
        return Err(format!(
            "refusing absolute path outside project: {rel_display}"
        ));
    }
    let canonical_root = appz_core::canonicalize(root)
        .map_err(|e| format!("cannot resolve project root {}: {e}", root.display()))?;
    let mut clean = PathBuf::new();
    for c in rel.components() {
        match c {
            std::path::Component::Normal(p) => clean.push(p),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(format!("refusing path outside project: {rel_display}"));
            }
        }
    }
    if clean.as_os_str().is_empty() {
        return Err("target file path cannot be empty".into());
    }
    let target = canonical_root.join(&clean);
    if let Some(parent) = target.parent()
        && parent.exists()
    {
        let canonical_parent = appz_core::canonicalize(parent)
            .map_err(|e| format!("cannot resolve parent dir: {e}"))?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(format!(
                "refusing path escaping project root: {rel_display}"
            ));
        }
    }
    if let Ok(meta) = std::fs::symlink_metadata(&target)
        && meta.file_type().is_symlink()
    {
        let real = appz_core::canonicalize(&target)
            .map_err(|e| format!("cannot resolve symlink {}: {e}", target.display()))?;
        if !real.starts_with(&canonical_root) {
            return Err(format!(
                "refusing symlink target outside project root: {rel_display}"
            ));
        }
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `appz env` (run from the root, a subfolder, or via `..`) and `appz
    /// deploy` must resolve the same vault path: the keyring session is keyed
    /// by it, so any difference makes an unlock invisible to deploy.
    #[test]
    fn every_entry_point_resolves_the_same_project_vault() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("src/deep")).unwrap();

        let from_root = vault_path(root, false).unwrap();
        assert_eq!(
            vault_path(&root.join("src/deep"), false).unwrap(),
            from_root
        );
        assert_eq!(
            vault_path(&root.join("src/deep/../.."), false).unwrap(),
            from_root
        );
        assert!(from_root.ends_with(Path::new(".appz").join("vault.json")));
    }

    #[test]
    fn names_are_validated() {
        assert!(valid_name("VERCEL_TOKEN"));
        assert!(valid_name("_x1"));
        assert!(!valid_name("1X"));
        assert!(!valid_name("A-B"));
        assert!(!valid_name(""));
    }

    #[test]
    fn the_passphrase_variable_is_never_inherited_by_children() {
        // `command` cannot depend on the vault crate, so its deny-list holds a
        // copy of the name: this keeps the two in step.
        assert!(command::DENIED_CHILD_ENV.contains(&PASSPHRASE_VAR));
    }

    fn listed(path: &Path) -> Vec<String> {
        let mut v: Vec<_> = expiries(path, Duration::days(14))
            .unwrap()
            .into_iter()
            .map(|r| r.name)
            .collect();
        v.sort();
        v
    }

    #[test]
    fn set_ls_rm_and_deploy_env_filter() {
        // Passphrase from env + keyring off: no prompts, no OS keyring.
        // SAFETY: single test touching these vars in this binary.
        unsafe {
            std::env::set_var(PASSPHRASE_VAR, "pw");
            std::env::set_var("APPZ_VAULT_KEYRING", "off");
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".appz").join("vault.json");
        assert_eq!(get(&path, "MISSING").unwrap(), None);
        set(&path, "VERCEL_TOKEN", Some("tok".into()), None).unwrap();
        set(&path, "OTHER", Some("x".into()), None).unwrap();
        assert_eq!(get(&path, "VERCEL_TOKEN").unwrap().as_deref(), Some("tok"));
        assert_eq!(get(&path, "NOT_SET").unwrap(), None);
        assert_eq!(listed(&path), vec!["OTHER", "VERCEL_TOKEN"]);
        assert!(remove(&path, "OTHER").unwrap());
        assert!(!remove(&path, "OTHER").unwrap());
        assert_eq!(listed(&path), vec!["VERCEL_TOKEN"]);

        // Inside a git repo the vault dir gets ignored, once.
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target").unwrap();
        let path2 = dir.path().join(".appz").join("v2.json");
        set(&path2, "A", Some("1".into()), None).unwrap();
        gitignore_project_vault(&path2);
        let gi = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(gi, "target\n# appz local state (not shared)\n.appz/\n");
    }

    #[test]
    fn expiry_flags_parse_and_conflict() {
        let now = DateTime::parse_from_rfc3339("2026-11-19T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parse_expiry(None, None, now).unwrap(), None);
        assert_eq!(
            parse_expiry(Some("2027-02-18"), None, now)
                .unwrap()
                .unwrap()
                .to_rfc3339(),
            "2027-02-18T00:00:00+00:00"
        );
        assert_eq!(
            parse_expiry(None, Some("90d"), now).unwrap().unwrap(),
            now + Duration::days(90)
        );
        assert_eq!(parse_duration("2w").unwrap(), Duration::weeks(2));
        assert_eq!(parse_duration("12h").unwrap(), Duration::hours(12));
        assert!(parse_expiry(Some("2027-02-18"), Some("90d"), now).is_err());
        assert!(parse_expiry(Some("18/02/2027"), None, now).is_err());
        assert!(parse_duration("90").is_err());
        assert!(parse_duration("d").is_err());
        assert!(parse_duration("").is_err());
    }

    #[test]
    fn set_records_expiry_that_ls_reads_without_unlock() {
        // SAFETY: only this test and the one above touch these vars, with the same values.
        unsafe {
            std::env::set_var(PASSPHRASE_VAR, "pw");
            std::env::set_var("APPZ_VAULT_KEYRING", "off");
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".appz").join("vault.json");
        let soon = Utc::now() + Duration::days(3);
        set(&path, "NPM_TOKEN", Some("t".into()), Some(soon)).unwrap();
        set(&path, "PLAIN", Some("p".into()), None).unwrap();
        let rows = expiries(&path, Duration::days(14)).unwrap();
        let find = |n: &str| rows.iter().find(|r| r.name == n).unwrap().status;
        assert_eq!(find("NPM_TOKEN"), ExpiryStatus::Expiring);
        assert_eq!(find("PLAIN"), ExpiryStatus::Untracked);
        // Overwriting without a date clears the recorded expiry.
        set(&path, "NPM_TOKEN", Some("t2".into()), None).unwrap();
        let rows = expiries(&path, Duration::days(14)).unwrap();
        assert_eq!(
            rows.iter().find(|r| r.name == "NPM_TOKEN").unwrap().status,
            ExpiryStatus::Untracked
        );
    }

    #[test]
    fn using_a_secret_near_or_past_expiry_warns_but_a_healthy_one_does_not() {
        let now = Utc::now();
        let entry = |at: Option<DateTime<Utc>>| SecretEntry {
            value: vec![],
            added_at: now,
            rotated_at: None,
            aad: true,
            expires_at: at,
        };
        assert_eq!(expiry_notice("T", &entry(None), now), None);
        assert_eq!(
            expiry_notice("T", &entry(Some(now + Duration::days(90))), now),
            None
        );
        let soon = expiry_notice(
            "T",
            &entry(Some(now + Duration::days(3) + Duration::hours(1))),
            now,
        )
        .unwrap();
        assert!(soon.starts_with("T expires in 3d"), "{soon}");
        let gone = expiry_notice("T", &entry(Some(now - Duration::days(2))), now).unwrap();
        assert!(gone.starts_with("T expired on"), "{gone}");
    }

    #[test]
    fn resolve_project_file_rejects_traversal_and_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let ok = resolve_project_file(dir.path(), ".env.local").unwrap();
        assert!(ok.ends_with(".env.local"));
        let nested = resolve_project_file(dir.path(), "sub/.env").unwrap();
        assert!(nested.ends_with(".env"));

        for bad in ["", "../secret", "a/../../secret", "/etc/passwd", "a\0b"] {
            assert!(resolve_project_file(dir.path(), bad).is_err(), "{bad}");
        }
    }
}
