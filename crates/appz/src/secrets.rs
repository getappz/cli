//! `appz env` storage and deploy-time lookup, on top of `appz-vault`.
//! Global vault `~/.appz/vault.json`, project vault `<root>/.appz/vault.json`.

use std::collections::HashMap;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

use appz_vault::{
    VaultError, VaultPaths, create_vault, get_secret_value, list_secret_names, open_vault,
    open_vault_with_dek, read_vault_body, remove_secret_value, set_secret_value, update_vault_body,
};

const APP: &str = "appz";
const PASSPHRASE_VAR: &str = "APPZ_VAULT_PASSPHRASE";

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
        && root.join(".git").exists()
    {
        appz_core::ensure_gitignored(root);
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
pub fn set(path: &Path, name: &str, value: Option<String>) -> Result<(), String> {
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
    update_vault_body(path, |b| set_secret_value(b, &dek, name, &value)).map_err(|e| e.to_string())
}

/// Names only; values are never printed. Needs no unlock.
pub fn names(path: &Path) -> Result<Vec<String>, String> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let body = read_vault_body(path).map_err(|e| e.to_string())?;
    Ok(list_secret_names(&body))
}

pub fn remove(path: &Path, name: &str) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    update_vault_body(path, |b| remove_secret_value(b, name)).map_err(|e| e.to_string())
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
                out.vars.insert(name.clone(), v.to_string());
            }
        }
    }
    Ok(out)
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
    fn set_ls_rm_and_deploy_env_filter() {
        // Passphrase from env + keyring off: no prompts, no OS keyring.
        // SAFETY: single test touching these vars in this binary.
        unsafe {
            std::env::set_var(PASSPHRASE_VAR, "pw");
            std::env::set_var("APPZ_VAULT_KEYRING", "off");
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".appz").join("vault.json");
        set(&path, "VERCEL_TOKEN", Some("tok".into())).unwrap();
        set(&path, "OTHER", Some("x".into())).unwrap();
        assert_eq!(names(&path).unwrap(), vec!["OTHER", "VERCEL_TOKEN"]);
        assert!(remove(&path, "OTHER").unwrap());
        assert!(!remove(&path, "OTHER").unwrap());
        assert_eq!(names(&path).unwrap(), vec!["VERCEL_TOKEN"]);

        // Inside a git repo the vault dir gets ignored, once.
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target").unwrap();
        let path2 = dir.path().join(".appz").join("v2.json");
        set(&path2, "A", Some("1".into())).unwrap();
        gitignore_project_vault(&path2);
        let gi = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(gi, "target\n# appz local state (not shared)\n.appz/\n");
    }
}
