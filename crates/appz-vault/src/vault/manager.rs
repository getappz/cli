use crate::crypto::aead::{decrypt_dek, decrypt_value, encrypt_dek, encrypt_value, KEY_CHECK_AAD};
use crate::crypto::kdf::{derive_kek, KdfParams};
use crate::error::{VaultError, VaultResult};
use crate::session;
use crate::vault::file::{create_vault_file, read_vault_file, vault_file_exists, write_vault_file};
use crate::vault::model::{SecretEntry, VaultBody, VaultFile, FORMAT_VERSION};
use chrono::Utc;
use rand::Rng;
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const SALT_SIZE: usize = 32;
const DEK_SIZE: usize = 32;
/// Upper bounds on Argon2 parameters read from a vault file (1 GiB, 20 passes, 16 lanes).
const MAX_KDF_MEMORY_KIB: u32 = 1 << 20;
const MAX_KDF_TIME_COST: u32 = 20;
const MAX_KDF_PARALLELISM: u32 = 16;
/// Plaintext of `VaultFile::key_check`.
const KEY_CHECK: &[u8] = b"appz-vault key check v1";

#[derive(ZeroizeOnDrop)]
pub struct VaultDek {
    pub dek: [u8; 32],
}

pub struct VaultPaths {
    pub global_dir: PathBuf,
    pub project_dir: Option<PathBuf>,
}

impl VaultPaths {
    pub fn global(app_name: &str) -> Self {
        let home = crate::paths::home_dir();
        Self {
            global_dir: home.join(format!(".{app_name}")),
            project_dir: None,
        }
    }

    pub fn project<F>(app_name: &str, start: &Path, is_project_root: F) -> Self
    where
        F: Fn(&Path) -> bool,
    {
        let home = crate::paths::home_dir();
        let global_dir = home.join(format!(".{app_name}"));
        let project_dir = resolve_project_dir(start, &home, is_project_root);
        Self {
            global_dir,
            project_dir,
        }
    }

    pub fn global_vault_path(&self) -> PathBuf {
        self.global_dir.join("vault.json")
    }

    pub fn project_vault_path(&self) -> Option<PathBuf> {
        self.project_dir.as_ref().map(|d| d.join("vault.json"))
    }
}

fn resolve_project_dir<F>(start: &Path, home: &Path, is_project_root: F) -> Option<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    let mut current = Some(start.to_path_buf());
    while let Some(dir) = current {
        if dir == home || dir.parent().is_none() {
            return None;
        }
        if is_project_root(&dir) {
            return Some(dir.join(".appz"));
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Parse a vault file, refusing one written by a newer appz: rewriting it
/// through this build's struct would silently drop the newer fields.
fn parse_vault(data: &[u8]) -> VaultResult<VaultFile> {
    let vault: VaultFile = serde_json::from_slice(data)?;
    if vault.version > FORMAT_VERSION {
        return Err(VaultError::Other(format!(
            "vault file is format v{}, but this appz understands up to v{FORMAT_VERSION}: upgrade appz",
            vault.version
        )));
    }
    Ok(vault)
}

pub fn create_vault(path: &Path, passphrase: &str) -> VaultResult<()> {
    if vault_file_exists(path) {
        return Err(VaultError::AlreadyInitialized(path.display().to_string()));
    }

    let salt = rand::rngs::OsRng.r#gen::<[u8; SALT_SIZE]>().to_vec();

    let params = KdfParams {
        salt: salt.clone(),
        ..Default::default()
    };

    let kek = derive_kek(passphrase, &params).map_err(VaultError::Crypto)?;

    let dek = rand::rngs::OsRng.r#gen::<[u8; DEK_SIZE]>();

    let blob = encrypt_dek(&dek, &kek.key).map_err(VaultError::Crypto)?;
    let mut vault = VaultFile::new(blob.to_bytes(), salt);
    vault.kdf = Some(params);
    vault.key_check = encrypt_value(KEY_CHECK, &dek, KEY_CHECK_AAD).map_err(VaultError::Crypto)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(&vault)?;
    create_vault_file(path, json.as_bytes())
}

pub fn open_vault(path: &Path, passphrase: &str) -> VaultResult<VaultDek> {
    let data = read_vault_file(path)?;
    let vault = parse_vault(&data)?;

    // The top-level salt is the single source of truth; `kdf.salt` is only a
    // copy, so a damaged or hand-edited file cannot make the two disagree.
    let mut params = vault.kdf.clone().unwrap_or_default();
    params.salt = vault.salt.clone();
    // The params come from a file that may be untrusted (a cloned repo's
    // project vault): refuse values that would exhaust memory or CPU.
    if params.memory_cost > MAX_KDF_MEMORY_KIB
        || params.time_cost > MAX_KDF_TIME_COST
        || params.parallelism > MAX_KDF_PARALLELISM
    {
        return Err(VaultError::Crypto(
            "vault KDF parameters out of range".into(),
        ));
    }

    let kek = derive_kek(passphrase, &params).map_err(VaultError::Crypto)?;

    let blob = crate::crypto::aead::EncryptedBlob::from_bytes(&vault.encrypted_dek)
        .map_err(VaultError::Crypto)?;

    let mut dek_bytes = decrypt_dek(&blob, &kek.key).map_err(|_| VaultError::WrongPassphrase)?;
    // Zeroize before propagating so a wrong-length DEK isn't left in memory.
    let dek = <[u8; DEK_SIZE]>::try_from(dek_bytes.as_slice());
    dek_bytes.zeroize();
    let dek = dek.map_err(|_| VaultError::Crypto("vault DEK has an invalid length".into()))?;

    // Best effort: with the key in hand, bind any legacy entries to their names.
    let _ = upgrade_legacy_entries(path, &dek);

    Ok(VaultDek { dek })
}

/// Re-encrypt entries written before AAD binding (`aad == false`) so they are
/// bound to their names like new ones. Until then anyone who can edit the file
/// could swap two such ciphertexts and have the wrong value served. Entries
/// that fail to decrypt are left untouched.
fn upgrade_legacy_entries(path: &Path, dek: &[u8; DEK_SIZE]) -> VaultResult<()> {
    if read_vault_body(path)?.values().all(|e| e.aad) {
        return Ok(());
    }
    update_vault_body(path, |body| {
        for (name, entry) in body.iter_mut().filter(|(_, e)| !e.aad) {
            let Ok(plain) = decrypt_value(&entry.value, dek, &[]).map(Zeroizing::new) else {
                continue;
            };
            if let Ok(bound) = encrypt_value(&plain, dek, name.as_bytes()) {
                entry.value = bound;
                entry.aad = true;
            }
        }
        Ok(())
    })
}

/// The cached session key for `path`, if there is one AND it is this file's
/// key. A vault that was deleted and recreated, or restored from a backup,
/// has a different DEK than the one the keyring remembers; using the stale
/// one would write secrets that can never be read back.
pub fn open_vault_with_dek(path: &Path, app_name: &str) -> VaultResult<VaultDek> {
    let Some(dek) = session::load_session(app_name, path) else {
        return Err(VaultError::Locked);
    };
    let dek = VaultDek { dek };
    let vault = parse_vault(&read_vault_file(path)?)?;
    // Empty key_check: a vault from before the check existed; nothing to compare.
    // Vaults created before AAD binding encrypted the check with empty AAD;
    // accept those too, or every pre-existing vault's session reads as stale.
    let check_ok =
        |aad: &[u8]| decrypt_value(&vault.key_check, &dek.dek, aad).is_ok_and(|p| p == KEY_CHECK);
    let matches = vault.key_check.is_empty() || check_ok(KEY_CHECK_AAD) || check_ok(&[]);
    if matches {
        Ok(dek)
    } else {
        session::clear_session(app_name, path);
        Err(VaultError::Locked)
    }
}

pub fn read_vault_body(path: &Path) -> VaultResult<VaultBody> {
    let data = read_vault_file(path)?;
    let vault = parse_vault(&data)?;
    Ok(vault.body)
}

/// Read-modify-write the body under an exclusive cross-process lock, so two
/// concurrent `appz env set` runs cannot lose each other's update.
pub fn update_vault_body<T>(
    path: &Path,
    f: impl FnOnce(&mut VaultBody) -> VaultResult<T>,
) -> VaultResult<T> {
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path.with_extension("lock"))?;
    fs2::FileExt::lock_exclusive(&lock)?;
    let mut body = read_vault_body(path)?;
    let out = f(&mut body)?;
    write_vault_body(path, &body)?;
    Ok(out)
}

pub fn write_vault_body(path: &Path, body: &VaultBody) -> VaultResult<()> {
    let data = read_vault_file(path)?;
    let mut vault = parse_vault(&data)?;
    vault.body = body.clone();
    // Entries written by this build are AAD-bound; label the file so.
    vault.version = FORMAT_VERSION;
    let json = serde_json::to_string_pretty(&vault)?;
    write_vault_file(path, json.as_bytes())
}

/// Returns the decrypted secret wrapped in `Zeroizing` so the plaintext is
/// wiped from memory as soon as the caller is done with it, rather than
/// lingering in the heap for the rest of the process's life.
pub fn get_secret_value(
    body: &VaultBody,
    dek: &[u8; 32],
    name: &str,
) -> VaultResult<Option<Zeroizing<String>>> {
    let entry = match body.get(name) {
        Some(e) => e,
        None => return Ok(None),
    };
    let aad: &[u8] = if entry.aad { name.as_bytes() } else { &[] };
    let plaintext = decrypt_value(&entry.value, dek, aad).map_err(|_| VaultError::Decrypt)?;
    let s = String::from_utf8(plaintext)
        .map_err(|_| VaultError::Crypto("invalid UTF-8 in secret".into()))?;
    Ok(Some(Zeroizing::new(s)))
}

pub fn set_secret_value(
    body: &mut VaultBody,
    dek: &[u8; 32],
    name: &str,
    value: &str,
) -> VaultResult<()> {
    let encrypted =
        encrypt_value(value.as_bytes(), dek, name.as_bytes()).map_err(VaultError::Crypto)?;
    body.insert(
        name.to_string(),
        SecretEntry {
            value: encrypted,
            added_at: Utc::now(),
            rotated_at: None,
            aad: true,
        },
    );
    Ok(())
}

pub fn remove_secret_value(body: &mut VaultBody, name: &str) -> VaultResult<bool> {
    Ok(body.remove(name).is_some())
}

pub fn list_secret_names(body: &VaultBody) -> Vec<String> {
    body.keys().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::paths::test_support::with_temp_home;

    /// Runs `f` with an isolated home, the shared in-memory keyring, and a
    /// vault at a temp path unlocked and cached (`dek` returned for the test).
    fn with_cached_vault(f: impl FnOnce(&Path, [u8; 32])) {
        crate::session::with_keyring_env(|| {
            crate::session::test_support::use_shared_mock_keyring();
            with_temp_home(|| {
                let dir = TempDir::new().unwrap();
                let path = dir.path().join("vault.json");
                create_vault(&path, "pw").unwrap();
                let dek = open_vault(&path, "pw").unwrap().dek;
                assert!(session::store_session("appz-vault-test", &path, &dek));
                f(&path, dek);
                session::clear_session("appz-vault-test", &path);
            });
        });
    }

    #[test]
    fn a_session_key_for_this_vault_is_accepted() {
        with_cached_vault(|path, dek| {
            let got = open_vault_with_dek(path, "appz-vault-test").unwrap();
            assert_eq!(got.dek, dek);
        });
    }

    #[test]
    fn a_stale_session_key_is_rejected_after_the_vault_is_recreated() {
        with_cached_vault(|path, _| {
            // Same path, brand-new vault (new DEK): what a delete+recreate or a
            // restore from backup looks like to the keyring's cached key.
            std::fs::remove_file(path).unwrap();
            create_vault(path, "pw").unwrap();
            assert!(matches!(
                open_vault_with_dek(path, "appz-vault-test"),
                Err(VaultError::Locked)
            ));
            // ...and the stale entry was dropped, so it is not retried.
            assert!(session::load_session("appz-vault-test", path).is_none());
        });
    }

    #[test]
    fn a_vault_without_a_key_check_still_accepts_its_session() {
        with_cached_vault(|path, dek| {
            let mut vault: VaultFile =
                serde_json::from_slice(&read_vault_file(path).unwrap()).unwrap();
            vault.key_check.clear();
            write_vault_file(path, serde_json::to_string(&vault).unwrap().as_bytes()).unwrap();
            assert_eq!(
                open_vault_with_dek(path, "appz-vault-test").unwrap().dek,
                dek
            );
        });
    }

    #[test]
    fn creating_over_an_existing_vault_never_replaces_it() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "first").unwrap();
        let before = std::fs::read(&path).unwrap();
        // The check-then-write race: bypass the existence check and write.
        let err = crate::vault::file::create_vault_file(&path, b"{}").unwrap_err();
        assert!(matches!(err, VaultError::AlreadyInitialized(_)));
        assert!(matches!(
            create_vault(&path, "second"),
            Err(VaultError::AlreadyInitialized(_))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn create_and_open_vault() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "test-pw").unwrap();
        let dek = open_vault(&path, "test-pw").unwrap();
        assert_eq!(dek.dek.len(), 32);
    }

    #[test]
    fn wrong_passphrase_fails() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "correct").unwrap();
        let err = open_vault(&path, "wrong");
        assert!(matches!(err, Err(VaultError::WrongPassphrase)));
    }

    #[test]
    fn double_create_fails() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let err = create_vault(&path, "pw");
        assert!(matches!(err, Err(VaultError::AlreadyInitialized(_))));
    }

    #[test]
    fn set_get_secret() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let dek = open_vault(&path, "pw").unwrap();
        let mut body = VaultBody::default();
        set_secret_value(&mut body, &dek.dek, "api_key", "sk-12345").unwrap();
        let val = get_secret_value(&body, &dek.dek, "api_key").unwrap();
        assert_eq!(val.unwrap().as_str(), "sk-12345");
    }

    #[test]
    fn remove_and_list() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let dek = open_vault(&path, "pw").unwrap();
        let mut body = VaultBody::default();
        set_secret_value(&mut body, &dek.dek, "a", "1").unwrap();
        set_secret_value(&mut body, &dek.dek, "b", "2").unwrap();
        let names = list_secret_names(&body);
        assert_eq!(names.len(), 2);
        assert!(remove_secret_value(&mut body, "a").unwrap());
        assert!(!remove_secret_value(&mut body, "a").unwrap());
    }

    #[test]
    fn save_and_reopen_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let dek = open_vault(&path, "pw").unwrap();
        let mut body = VaultBody::default();
        set_secret_value(&mut body, &dek.dek, "key", "value").unwrap();
        write_vault_body(&path, &body).unwrap();

        let body2 = read_vault_body(&path).unwrap();
        let val = get_secret_value(&body2, &dek.dek, "key").unwrap();
        assert_eq!(val.unwrap().as_str(), "value");
    }

    #[test]
    fn swapping_secret_values_is_detected() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let dek = open_vault(&path, "pw").unwrap();
        let mut body = VaultBody::default();
        set_secret_value(&mut body, &dek.dek, "A", "value-a").unwrap();
        set_secret_value(&mut body, &dek.dek, "B", "value-b").unwrap();

        // Swap the ciphertexts: what an attacker with write access to
        // vault.json can do. Each is bound to its own name as AAD, so
        // decryption must fail rather than silently serve the wrong value.
        let a = body.get("A").unwrap().value.clone();
        let b = body.get("B").unwrap().value.clone();
        body.get_mut("A").unwrap().value = b;
        body.get_mut("B").unwrap().value = a;

        assert!(matches!(
            get_secret_value(&body, &dek.dek, "A"),
            Err(VaultError::Decrypt)
        ));
        assert!(matches!(
            get_secret_value(&body, &dek.dek, "B"),
            Err(VaultError::Decrypt)
        ));
    }

    #[test]
    fn a_legacy_entry_without_aad_still_decrypts() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let dek = open_vault(&path, "pw").unwrap();

        // A v1 entry: encrypted with empty AAD, `aad` flag false. Must still
        // read correctly so existing vaults keep working after the upgrade.
        let value = encrypt_value(b"legacy", &dek.dek, &[]).unwrap();
        let mut body = VaultBody::default();
        body.insert(
            "K".to_string(),
            SecretEntry {
                value,
                added_at: Utc::now(),
                rotated_at: None,
                aad: false,
            },
        );
        assert_eq!(
            get_secret_value(&body, &dek.dek, "K")
                .unwrap()
                .unwrap()
                .as_str(),
            "legacy"
        );
    }

    #[test]
    fn a_legacy_empty_aad_key_check_still_accepts_its_session() {
        with_cached_vault(|path, dek| {
            let mut vault: VaultFile =
                serde_json::from_slice(&read_vault_file(path).unwrap()).unwrap();
            vault.key_check = encrypt_value(KEY_CHECK, &dek, &[]).unwrap();
            write_vault_file(path, serde_json::to_string(&vault).unwrap().as_bytes()).unwrap();
            assert_eq!(
                open_vault_with_dek(path, "appz-vault-test").unwrap().dek,
                dek
            );
        });
    }

    #[test]
    fn absurd_kdf_params_in_a_vault_file_are_rejected() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let mut vault: VaultFile =
            serde_json::from_slice(&read_vault_file(&path).unwrap()).unwrap();
        vault.kdf.as_mut().unwrap().memory_cost = u32::MAX;
        write_vault_file(&path, serde_json::to_string(&vault).unwrap().as_bytes()).unwrap();
        assert!(matches!(
            open_vault(&path, "pw"),
            Err(VaultError::Crypto(_))
        ));
    }

    #[test]
    fn kdf_params_are_recorded_in_the_vault_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let vault: VaultFile = serde_json::from_slice(&read_vault_file(&path).unwrap()).unwrap();
        let kdf = vault.kdf.expect("kdf params recorded at creation");
        let defaults = KdfParams::default();
        assert_eq!(kdf.time_cost, defaults.time_cost);
        assert_eq!(kdf.memory_cost, defaults.memory_cost);
        assert_eq!(kdf.parallelism, defaults.parallelism);
        assert_eq!(kdf.salt, vault.salt);
    }

    #[test]
    fn rewriting_a_v1_vault_labels_it_current() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let mut vault: VaultFile =
            serde_json::from_slice(&read_vault_file(&path).unwrap()).unwrap();
        vault.version = 1;
        write_vault_file(&path, serde_json::to_string(&vault).unwrap().as_bytes()).unwrap();
        update_vault_body(&path, |_| Ok(())).unwrap();
        let after: VaultFile = serde_json::from_slice(&read_vault_file(&path).unwrap()).unwrap();
        assert_eq!(after.version, FORMAT_VERSION);
    }

    #[test]
    fn a_legacy_vault_without_recorded_kdf_params_still_opens() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        // v1 vaults predate the `kdf` field; strip it to simulate one.
        let mut json: serde_json::Value =
            serde_json::from_slice(&read_vault_file(&path).unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("kdf");
        write_vault_file(&path, serde_json::to_string(&json).unwrap().as_bytes()).unwrap();
        assert!(open_vault(&path, "pw").is_ok());
    }

    /// A vault whose `name` secret was written the old way (empty AAD).
    fn vault_with_legacy_secret(path: &Path, name: &str, value: &str) {
        create_vault(path, "pw").unwrap();
        let dek = open_vault(path, "pw").unwrap().dek;
        update_vault_body(path, |body| {
            body.insert(
                name.to_string(),
                SecretEntry {
                    value: encrypt_value(value.as_bytes(), &dek, &[]).unwrap(),
                    added_at: Utc::now(),
                    rotated_at: None,
                    aad: false,
                },
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn unlocking_binds_legacy_secrets_to_their_names() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        vault_with_legacy_secret(&path, "A", "one");
        assert!(!read_vault_body(&path).unwrap()["A"].aad);

        let dek = open_vault(&path, "pw").unwrap().dek;

        let body = read_vault_body(&path).unwrap();
        assert!(body["A"].aad, "legacy entry must be upgraded on unlock");
        assert_eq!(
            get_secret_value(&body, &dek, "A")
                .unwrap()
                .unwrap()
                .as_str(),
            "one"
        );
    }

    #[test]
    fn upgraded_secrets_cannot_be_swapped() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        vault_with_legacy_secret(&path, "A", "one");
        let dek = open_vault(&path, "pw").unwrap().dek;
        update_vault_body(&path, |body| set_secret_value(body, &dek, "B", "two")).unwrap();
        // Swap the two ciphertexts, as someone editing the file could.
        update_vault_body(&path, |body| {
            let a = body["A"].value.clone();
            let b = body["B"].value.clone();
            body.get_mut("A").unwrap().value = b;
            body.get_mut("B").unwrap().value = a;
            Ok(())
        })
        .unwrap();

        let body = read_vault_body(&path).unwrap();
        assert!(matches!(
            get_secret_value(&body, &dek, "A"),
            Err(VaultError::Decrypt)
        ));
    }

    #[test]
    fn a_vault_from_a_newer_appz_is_refused_not_rewritten() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let mut vault: VaultFile =
            serde_json::from_slice(&read_vault_file(&path).unwrap()).unwrap();
        vault.version = FORMAT_VERSION + 1;
        write_vault_file(&path, serde_json::to_string(&vault).unwrap().as_bytes()).unwrap();
        let before = std::fs::read(&path).unwrap();

        assert!(open_vault(&path, "pw").is_err());
        assert!(read_vault_body(&path).is_err());
        assert!(update_vault_body(&path, |_| Ok(())).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before, "file untouched");
    }

    #[test]
    fn the_top_level_salt_wins_over_a_disagreeing_kdf_salt() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.json");
        create_vault(&path, "pw").unwrap();
        let mut vault: VaultFile =
            serde_json::from_slice(&read_vault_file(&path).unwrap()).unwrap();
        vault.kdf.as_mut().unwrap().salt = vec![7; SALT_SIZE];
        write_vault_file(&path, serde_json::to_string(&vault).unwrap().as_bytes()).unwrap();

        assert!(open_vault(&path, "pw").is_ok());
    }
}
