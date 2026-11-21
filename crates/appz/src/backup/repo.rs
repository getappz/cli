//! Open or initialise the per-project rustic backup repository.
//!
//! Password resolution order: `APPZ_BACKUP_PASSWORD` env var > global vault
//! entry `APPZ_BACKUP_<PROJECT_ID>`. Initialisation only runs when
//! [`Interactive::Yes`]; non-interactive runs (`MCP`, `CI`, `--json`) skip with
//! a note rather than prompting or creating repositories.

// Consumed by upcoming backup tasks (create, after_deploy, list, restore).
#![allow(dead_code)]

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use rand::RngCore;

use crate::deploy::Consent;
use crate::secrets::{self, SecretStatus};

use super::config::{self, BackupConfig};
use super::engine::{self, EngineError, Rustic, RusticCli};

/// Environment variable that overrides the vault-stored backup password.
pub const PASSWORD_ENV: &str = engine::ENV_PASSWORD;

/// Whether interactive prompts and first-time repository initialisation are permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interactive {
    Yes,
    No,
}

/// Vault key for storing a project's generated backup password in the global vault.
pub fn vault_key(project_id: &str) -> String {
    format!("APPZ_BACKUP_{}", project_id.to_ascii_uppercase())
}

trait BackupVault {
    fn get_status(&self, key: &str) -> Result<SecretStatus, String>;
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
}

struct GlobalVault {
    path: PathBuf,
}

impl BackupVault for GlobalVault {
    fn get_status(&self, key: &str) -> Result<SecretStatus, String> {
        secrets::get_status(&self.path, key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        secrets::set(&self.path, key, Some(value.to_string()), None)
    }
}

/// Open an existing backup repository or (when [`Interactive::Yes`]) initialise
/// a new one, storing a freshly generated password in the global vault first.
///
/// Returns `Ok(None)` when backup should be skipped cleanly (disabled, missing
/// rustic without consent, locked vault, or uninitialised repo in
/// non-interactive mode).
pub fn open_repo(
    root: &Path,
    cfg: &BackupConfig,
    mode: Interactive,
) -> Result<Option<RusticCli>, String> {
    open_repo_noting(root, cfg, mode, |msg| eprintln!("{msg}"))
}

pub(super) fn open_repo_noting(
    root: &Path,
    cfg: &BackupConfig,
    mode: Interactive,
    note: impl FnMut(&str),
) -> Result<Option<RusticCli>, String> {
    let vault_path = secrets::vault_path(root, true)?;
    let vault = GlobalVault { path: vault_path };
    let consent = match mode {
        Interactive::No => Consent::Never,
        Interactive::Yes if crate::assume_yes() => Consent::Yes,
        Interactive::Yes if crate::is_tty() => Consent::Ask,
        Interactive::Yes => Consent::Never,
    };
    open_repo_with(
        root,
        cfg,
        mode,
        engine::env_password(),
        &vault,
        |dir| engine::ensure_installed(dir, consent),
        |bin, repo, password, cwd| RusticCli::new(bin, repo, password, cwd),
        note,
        |recovery| {
            if std::io::stderr().is_terminal() {
                let _ = cliclack::note(
                    "Backup recovery key (save this somewhere off this machine)",
                    recovery,
                );
            }
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn open_repo_with<E: Rustic>(
    root: &Path,
    cfg: &BackupConfig,
    mode: Interactive,
    env_pw: Result<Option<String>, String>,
    vault: &dyn BackupVault,
    ensure_bin: impl FnOnce(&Path) -> Result<PathBuf, String>,
    make_client: impl FnOnce(PathBuf, &str, &str, &Path) -> E,
    mut note: impl FnMut(&str),
    show_recovery: impl FnOnce(&str),
) -> Result<Option<E>, String> {
    if !cfg.is_enabled() {
        return Ok(None);
    }

    let env_pw = env_pw?.filter(|s| !s.is_empty());
    let project_id = config::project_id(root);
    let key = vault_key(&project_id);
    let repo = config::repo_location(cfg, root);

    let password_from_env_or_vault = match env_pw {
        Some(pw) => Some(pw),
        None => match vault.get_status(&key)? {
            SecretStatus::Found(pw) if !pw.is_empty() => Some(pw),
            SecretStatus::Found(_) | SecretStatus::Missing => None,
            SecretStatus::Locked => {
                note(&format!(
                    "backup skipped: global vault is locked (run `appz vault unlock --global` or set {PASSWORD_ENV})"
                ));
                return Ok(None);
            }
        },
    };

    let bin = match ensure_bin(root) {
        Ok(bin) => bin,
        Err(msg) if is_consent_skip(&msg) => {
            note(&format!("backup skipped: {msg}"));
            return Ok(None);
        }
        Err(msg) => return Err(msg),
    };

    match password_from_env_or_vault {
        Some(password) => {
            let client = make_client(bin, &repo, &password, root);
            match client.run(&["snapshots", "--json"]) {
                Ok(_) => Ok(Some(client)),
                Err(EngineError::Missing) => {
                    note("backup skipped: rustic binary is not installed");
                    Ok(None)
                }
                Err(EngineError::Locked) => {
                    note("backup skipped: backup repository is locked by another rustic process");
                    Ok(None)
                }
                Err(EngineError::WrongPassword) => Err(EngineError::WrongPassword.to_string()),
                Err(EngineError::Failed(msg)) if is_uninitialized_repo_error(&msg) => match mode {
                    Interactive::No => {
                        note(
                            "backup skipped: repository is not initialized (run `appz backup init`)",
                        );
                        Ok(None)
                    }
                    Interactive::Yes => {
                        client.run(&["init"]).map_err(|e| e.to_string())?;
                        Ok(Some(client))
                    }
                },
                Err(EngineError::Failed(msg)) => Err(msg),
            }
        }
        None => match mode {
            Interactive::No => {
                note(
                    "backup skipped: repository is not initialized (run `appz backup` interactively first)",
                );
                Ok(None)
            }
            Interactive::Yes => {
                if local_repo_exists(&repo) {
                    return Err(format!(
                        "backup repository already exists at {repo}, but {key} is missing from the global vault (set {PASSWORD_ENV})"
                    ));
                }
                let password = generate_recovery_password();
                // Persist password to the vault before initializing the repo so
                // an interrupted init never strands an encrypted repository.
                vault.set(&key, &password)?;
                let client = make_client(bin, &repo, &password, root);
                client.run(&["init"]).map_err(|e| e.to_string())?;
                show_recovery(&password);
                Ok(Some(client))
            }
        },
    }
}

fn local_repo_exists(repo: &str) -> bool {
    let local = repo.strip_prefix("local:").unwrap_or(repo);
    Path::new(local).join("config").is_file()
}

fn is_consent_skip(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("not installed") || lower.contains("declined") || lower.contains("cancelled")
}

fn is_uninitialized_repo_error(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("no repository config file found")
        || lower.contains("does not exist")
        || lower.contains("is not a repository")
        || lower.contains("no such file")
}

fn generate_recovery_password() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    encode_base64(&bytes)
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::backup::engine::RunOut;

    #[derive(Clone)]
    struct FakeVault {
        status: RefCell<SecretStatus>,
        events: Rc<RefCell<Vec<String>>>,
        stored: Rc<RefCell<Vec<(String, String)>>>,
    }

    impl FakeVault {
        fn missing(events: Rc<RefCell<Vec<String>>>) -> Self {
            Self {
                status: RefCell::new(SecretStatus::Missing),
                events,
                stored: Default::default(),
            }
        }

        fn found(value: &str, events: Rc<RefCell<Vec<String>>>) -> Self {
            Self {
                status: RefCell::new(SecretStatus::Found(value.to_string())),
                events,
                stored: Default::default(),
            }
        }

        fn locked(events: Rc<RefCell<Vec<String>>>) -> Self {
            Self {
                status: RefCell::new(SecretStatus::Locked),
                events,
                stored: Default::default(),
            }
        }
    }

    impl BackupVault for FakeVault {
        fn get_status(&self, _key: &str) -> Result<SecretStatus, String> {
            Ok(match &*self.status.borrow() {
                SecretStatus::Missing => SecretStatus::Missing,
                SecretStatus::Locked => SecretStatus::Locked,
                SecretStatus::Found(v) => SecretStatus::Found(v.clone()),
            })
        }

        fn set(&self, key: &str, value: &str) -> Result<(), String> {
            self.events.borrow_mut().push(format!("vault_set:{key}"));
            self.stored
                .borrow_mut()
                .push((key.to_string(), value.to_string()));
            *self.status.borrow_mut() = SecretStatus::Found(value.to_string());
            Ok(())
        }
    }

    struct FakeRustic {
        password: String,
        repo_initialized: RefCell<bool>,
        events: Rc<RefCell<Vec<String>>>,
    }

    impl Rustic for FakeRustic {
        fn run(&self, args: &[&str]) -> Result<RunOut, EngineError> {
            self.events
                .borrow_mut()
                .push(format!("rustic:{}", args.join(" ")));
            match args {
                ["snapshots", "--json"] => {
                    if *self.repo_initialized.borrow() {
                        Ok(RunOut {
                            stdout: "[]".to_string(),
                            stderr: String::new(),
                        })
                    } else {
                        Err(EngineError::Failed(
                            "No repository config file found".to_string(),
                        ))
                    }
                }
                ["init"] => {
                    *self.repo_initialized.borrow_mut() = true;
                    Ok(RunOut {
                        stdout: String::new(),
                        stderr: String::new(),
                    })
                }
                other => Err(EngineError::Failed(format!("unexpected args: {other:?}"))),
            }
        }
    }

    #[test]
    fn vault_key_uppercases_project_id() {
        assert_eq!(
            vault_key("a1b2c3d4e5f60708"),
            "APPZ_BACKUP_A1B2C3D4E5F60708"
        );
    }

    #[test]
    fn base64_encoding_matches_rfc4648() {
        assert_eq!(encode_base64(b"M"), "TQ==");
        assert_eq!(encode_base64(b"Ma"), "TWE=");
        assert_eq!(encode_base64(b"Man"), "TWFu");
        assert_eq!(encode_base64(&[0u8; 32]).len(), 44);
    }

    #[test]
    fn non_interactive_without_repo_skips() {
        let dir = tempfile::tempdir().unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let vault = FakeVault::missing(events.clone());
        let mut notes = Vec::new();

        let res = open_repo_with(
            dir.path(),
            &BackupConfig::default(),
            Interactive::No,
            Ok(None),
            &vault,
            |_| Ok(PathBuf::from("rustic")),
            |_, _, pw, _| FakeRustic {
                password: pw.to_string(),
                repo_initialized: RefCell::new(false),
                events: events.clone(),
            },
            |msg| notes.push(msg.to_string()),
            |_| {},
        )
        .unwrap();

        assert!(res.is_none());
        assert!(events.borrow().is_empty());
        assert!(vault.stored.borrow().is_empty());
        assert_eq!(notes.len(), 1);

        // Even with an env password, non-interactive must not init an uninitialized repo.
        let mut notes2 = Vec::new();
        let res2 = open_repo_with(
            dir.path(),
            &BackupConfig::default(),
            Interactive::No,
            Ok(Some("env-pw".to_string())),
            &vault,
            |_| Ok(PathBuf::from("rustic")),
            |_, _, pw, _| FakeRustic {
                password: pw.to_string(),
                repo_initialized: RefCell::new(false),
                events: events.clone(),
            },
            |msg| notes2.push(msg.to_string()),
            |_| {},
        )
        .unwrap();

        assert!(res2.is_none());
        assert_eq!(*events.borrow(), vec!["rustic:snapshots --json"]);
        assert!(vault.stored.borrow().is_empty());
        assert_eq!(notes2.len(), 1);
    }

    #[test]
    fn env_password_overrides_vault() {
        let dir = tempfile::tempdir().unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let vault = FakeVault::found("vault-pw", events.clone());

        let client = open_repo_with(
            dir.path(),
            &BackupConfig::default(),
            Interactive::No,
            Ok(Some("env-pw".to_string())),
            &vault,
            |_| Ok(PathBuf::from("rustic")),
            |_, _, pw, _| FakeRustic {
                password: pw.to_string(),
                repo_initialized: RefCell::new(true),
                events: events.clone(),
            },
            |_| {},
            |_| {},
        )
        .unwrap()
        .expect("should open existing repo with env password");

        assert_eq!(client.password, "env-pw");
        assert_eq!(*events.borrow(), vec!["rustic:snapshots --json"]);
    }

    #[test]
    fn locked_vault_skips_with_note() {
        let dir = tempfile::tempdir().unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let vault = FakeVault::locked(events.clone());
        let mut notes = Vec::new();

        let res = open_repo_with(
            dir.path(),
            &BackupConfig::default(),
            Interactive::Yes,
            Ok(None),
            &vault,
            |_| Ok(PathBuf::from("rustic")),
            |_, _, pw, _| FakeRustic {
                password: pw.to_string(),
                repo_initialized: RefCell::new(false),
                events: events.clone(),
            },
            |msg| notes.push(msg.to_string()),
            |_| {},
        )
        .unwrap();

        assert!(res.is_none());
        assert!(events.borrow().is_empty());
        assert!(vault.stored.borrow().is_empty());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("locked"), "unexpected note: {}", notes[0]);
    }

    #[test]
    fn init_stores_password_then_inits() {
        let dir = tempfile::tempdir().unwrap();
        let events = Rc::new(RefCell::new(Vec::new()));
        let vault = FakeVault::missing(events.clone());
        let mut shown_recovery = None;
        let expected_key = vault_key(&config::project_id(dir.path()));

        let client = open_repo_with(
            dir.path(),
            &BackupConfig::default(),
            Interactive::Yes,
            Ok(None),
            &vault,
            |_| Ok(PathBuf::from("rustic")),
            |_, _, pw, _| FakeRustic {
                password: pw.to_string(),
                repo_initialized: RefCell::new(false),
                events: events.clone(),
            },
            |_| {},
            |key| shown_recovery = Some(key.to_string()),
        )
        .unwrap()
        .expect("interactive init should return client");

        assert_eq!(
            *events.borrow(),
            vec![
                format!("vault_set:{expected_key}"),
                "rustic:init".to_string()
            ]
        );
        let stored = vault.stored.borrow();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].0, expected_key);
        assert_eq!(stored[0].1.len(), 44);
        assert_eq!(client.password, stored[0].1);
        assert_eq!(shown_recovery.as_deref(), Some(stored[0].1.as_str()));
    }
}
