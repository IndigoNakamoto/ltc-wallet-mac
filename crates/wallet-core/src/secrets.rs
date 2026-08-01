use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;
use zeroize::{Zeroize, Zeroizing};

use crate::error::WalletError;

/// Stores the wallet mnemonic outside SQLite.
pub trait SecretStore: Send + Sync {
    fn set_mnemonic(&self, mnemonic: &str) -> Result<(), WalletError>;
    fn get_mnemonic(&self) -> Result<Option<String>, WalletError>;
    fn delete_mnemonic(&self) -> Result<(), WalletError>;
}

/// Optional unlockable store (encrypted at rest).
pub trait UnlockableSecretStore: SecretStore {
    fn is_locked(&self) -> bool;
    fn needs_migration(&self) -> bool;
    fn unlock(&self, passphrase: &str) -> Result<(), WalletError>;
    fn lock(&self);
    /// Encrypt plaintext mnemonic in place; requires unlock afterward is already done.
    fn migrate_encrypt(&self, passphrase: &str) -> Result<(), WalletError>;
    /// Key material for sealing related stores (e.g. MWEB); `None` when locked/plaintext.
    fn sealing_key(&self) -> Option<[u8; 32]>;
}

/// Legacy v1 format: Argon2id (19 MiB, t=2), plaintext = mnemonic only, and the
/// passphrase-derived key doubled as the sealing key for related stores.
const MAGIC_V1: &[u8; 8] = b"LTCMNEM1";
/// Current v2 format: Argon2id (64 MiB, t=3), plaintext = 32-byte random sealing
/// key || mnemonic. Keeping the sealing key independent of the KDF output means
/// future KDF parameter bumps never orphan data sealed under it.
const MAGIC_V2: &[u8; 8] = b"LTCMNEM2";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

/// Result of decrypting a mnemonic blob of any supported version.
struct Opened {
    mnemonic: Zeroizing<String>,
    sealing_key: Zeroizing<[u8; KEY_LEN]>,
    /// True when the blob is v1 and should be transparently upgraded to v2.
    legacy: bool,
}

/// File-backed mnemonic store (mode `0600`) — plaintext (legacy / tests with MemoryStore preferred).
pub struct FileSecretStore {
    path: PathBuf,
}

impl FileSecretStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SecretStore for FileSecretStore {
    fn set_mnemonic(&self, mnemonic: &str) -> Result<(), WalletError> {
        write_bytes(&self.path, mnemonic.trim().as_bytes())
    }

    fn get_mnemonic(&self) -> Result<Option<String>, WalletError> {
        match fs::read(&self.path) {
            Ok(bytes) => {
                let trimmed = String::from_utf8_lossy(&bytes).trim().to_string();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(WalletError::SecretStore(e.to_string())),
        }
    }

    fn delete_mnemonic(&self) -> Result<(), WalletError> {
        remove_if_exists(&self.path)
    }
}

/// Encrypted file-backed store: Argon2id + ChaCha20-Poly1305.
///
/// Layout: `LTCMNEM1` || salt(16) || nonce(12) || ciphertext.
pub struct EncryptedFileSecretStore {
    path: PathBuf,
    plaintext_path: PathBuf,
    inner: Mutex<EncryptedInner>,
}

struct EncryptedInner {
    unlocked_mnemonic: Option<Zeroizing<String>>,
    sealing_key: Option<Zeroizing<[u8; KEY_LEN]>>,
}

impl EncryptedFileSecretStore {
    pub fn new(encrypted_path: impl Into<PathBuf>, plaintext_path: impl Into<PathBuf>) -> Self {
        Self {
            path: encrypted_path.into(),
            plaintext_path: plaintext_path.into(),
            inner: Mutex::new(EncryptedInner {
                unlocked_mnemonic: None,
                sealing_key: None,
            }),
        }
    }

    fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, EncryptedInner>, WalletError> {
        self.inner
            .lock()
            .map_err(|_| WalletError::SecretStore("secret store lock poisoned".into()))
    }

    fn encrypted_exists(&self) -> bool {
        self.path.is_file()
    }

    fn plaintext_exists(&self) -> bool {
        self.plaintext_path.is_file()
    }

    fn derive_key_v1(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], WalletError> {
        Self::derive_key(passphrase, salt, 19_456, 2)
    }

    /// v2 parameters: 64 MiB, t=3, p=1 — comfortably above the OWASP minimum
    /// while staying under a second on typical desktop hardware.
    fn derive_key_v2(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], WalletError> {
        Self::derive_key(passphrase, salt, 65_536, 3)
    }

    fn derive_key(
        passphrase: &str,
        salt: &[u8],
        m_kib: u32,
        t_cost: u32,
    ) -> Result<[u8; KEY_LEN], WalletError> {
        let params = Params::new(m_kib, t_cost, 1, Some(KEY_LEN))
            .map_err(|e| WalletError::SecretStore(format!("argon2 params: {e}")))?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = [0u8; KEY_LEN];
        argon
            .hash_password_into(passphrase.as_bytes(), salt, &mut key)
            .map_err(|e| WalletError::SecretStore(format!("argon2: {e}")))?;
        Ok(key)
    }

    /// Seal in the current (v2) format: plaintext = sealing_key || mnemonic.
    fn seal(
        passphrase: &str,
        mnemonic: &str,
        sealing_key: &[u8; KEY_LEN],
    ) -> Result<Vec<u8>, WalletError> {
        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let mut key = Self::derive_key_v2(passphrase, &salt)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut plain =
            Zeroizing::new(Vec::with_capacity(KEY_LEN + mnemonic.trim().len()));
        plain.extend_from_slice(sealing_key);
        plain.extend_from_slice(mnemonic.trim().as_bytes());
        let ciphertext = cipher
            .encrypt(nonce, plain.as_slice())
            .map_err(|_| WalletError::SecretStore("encrypt failed".into()))?;
        key.zeroize();
        let mut out =
            Vec::with_capacity(MAGIC_V2.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
        out.extend_from_slice(MAGIC_V2);
        out.extend_from_slice(&salt);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Decrypt a blob of either supported version.
    fn open(passphrase: &str, blob: &[u8]) -> Result<Opened, WalletError> {
        if blob.len() < MAGIC_V2.len() + SALT_LEN + NONCE_LEN + 16 {
            return Err(WalletError::SecretStore(
                "encrypted mnemonic file is corrupt or truncated".into(),
            ));
        }
        let magic = &blob[..MAGIC_V2.len()];
        let legacy = if magic == MAGIC_V2 {
            false
        } else if magic == MAGIC_V1 {
            true
        } else {
            return Err(WalletError::SecretStore(
                "encrypted mnemonic file has unknown format".into(),
            ));
        };
        let salt = &blob[MAGIC_V2.len()..MAGIC_V2.len() + SALT_LEN];
        let nonce_bytes = &blob[MAGIC_V2.len() + SALT_LEN..MAGIC_V2.len() + SALT_LEN + NONCE_LEN];
        let ciphertext = &blob[MAGIC_V2.len() + SALT_LEN + NONCE_LEN..];
        let mut key = if legacy {
            Self::derive_key_v1(passphrase, salt)?
        } else {
            Self::derive_key_v2(passphrase, salt)?
        };
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let nonce = Nonce::from_slice(nonce_bytes);
        let plain = Zeroizing::new(
            cipher
                .decrypt(nonce, ciphertext)
                .map_err(|_| WalletError::IncorrectPassphrase)?,
        );
        if legacy {
            // v1 carried only the mnemonic; the derived key doubled as the sealing key.
            let mnemonic = String::from_utf8(plain.to_vec())
                .map_err(|_| WalletError::SecretStore("mnemonic is not valid UTF-8".into()))?;
            Ok(Opened {
                mnemonic: Zeroizing::new(mnemonic),
                sealing_key: Zeroizing::new(key),
                legacy: true,
            })
        } else {
            key.zeroize();
            if plain.len() < KEY_LEN {
                return Err(WalletError::SecretStore(
                    "encrypted mnemonic payload is truncated".into(),
                ));
            }
            let mut sealing_key = [0u8; KEY_LEN];
            sealing_key.copy_from_slice(&plain[..KEY_LEN]);
            let mnemonic = String::from_utf8(plain[KEY_LEN..].to_vec())
                .map_err(|_| WalletError::SecretStore("mnemonic is not valid UTF-8".into()))?;
            Ok(Opened {
                mnemonic: Zeroizing::new(mnemonic),
                sealing_key: Zeroizing::new(sealing_key),
                legacy: false,
            })
        }
    }
}

impl SecretStore for EncryptedFileSecretStore {
    fn set_mnemonic(&self, mnemonic: &str) -> Result<(), WalletError> {
        let mut inner = self.lock_inner()?;
        let key = inner
            .sealing_key
            .as_ref()
            .ok_or(WalletError::Locked)?;
        // Re-seal with a fresh salt using the current passphrase-derived key is not possible
        // without the passphrase; instead keep unlocked mnemonic and require passphrase on migrate.
        // For create/restore after unlock/with passphrase path, caller uses migrate_encrypt or
        // set_with_passphrase via UnlockableSecretStore.
        let _ = key;
        inner.unlocked_mnemonic = Some(Zeroizing::new(mnemonic.trim().to_string()));
        Err(WalletError::SecretStore(
            "encrypted store requires set_with_passphrase / migrate_encrypt".into(),
        ))
    }

    fn get_mnemonic(&self) -> Result<Option<String>, WalletError> {
        let inner = self.lock_inner()?;
        if self.encrypted_exists() {
            return Ok(inner.unlocked_mnemonic.as_ref().map(|m| m.to_string()));
        }
        // Plaintext fallback while awaiting migration.
        drop(inner);
        match fs::read_to_string(&self.plaintext_path) {
            Ok(contents) => {
                let trimmed = contents.trim().to_string();
                if trimmed.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(trimmed))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(WalletError::SecretStore(e.to_string())),
        }
    }

    fn delete_mnemonic(&self) -> Result<(), WalletError> {
        {
            let mut inner = self.lock_inner()?;
            if let Some(ref mut m) = inner.unlocked_mnemonic {
                m.zeroize();
            }
            inner.unlocked_mnemonic = None;
            if let Some(ref mut k) = inner.sealing_key {
                k.zeroize();
            }
            inner.sealing_key = None;
        }
        remove_if_exists(&self.path)?;
        remove_if_exists(&self.plaintext_path)?;
        Ok(())
    }
}

impl EncryptedFileSecretStore {
    /// Create or overwrite encrypted mnemonic with passphrase (create/restore path).
    pub fn set_with_passphrase(&self, passphrase: &str, mnemonic: &str) -> Result<(), WalletError> {
        if passphrase.is_empty() {
            return Err(WalletError::SecretStore("passphrase must not be empty".into()));
        }
        let mut sealing_key = [0u8; KEY_LEN];
        rand::thread_rng().fill_bytes(&mut sealing_key);
        let blob = Self::seal(passphrase, mnemonic, &sealing_key)?;
        write_bytes(&self.path, &blob)?;
        // Verify round-trip before deleting any plaintext.
        let opened = Self::open(passphrase, &blob)?;
        if opened.mnemonic.trim() != mnemonic.trim() {
            let _ = remove_if_exists(&self.path);
            return Err(WalletError::SecretStore(
                "encrypted store did not persist mnemonic".into(),
            ));
        }
        sealing_key.zeroize();
        let mut inner = self.lock_inner()?;
        inner.unlocked_mnemonic = Some(opened.mnemonic);
        inner.sealing_key = Some(opened.sealing_key);
        let _ = remove_if_exists(&self.plaintext_path);
        Ok(())
    }
}

impl UnlockableSecretStore for EncryptedFileSecretStore {
    fn is_locked(&self) -> bool {
        if !self.encrypted_exists() {
            return false;
        }
        self.lock_inner()
            .map(|g| g.unlocked_mnemonic.is_none())
            .unwrap_or(true)
    }

    fn needs_migration(&self) -> bool {
        !self.encrypted_exists() && self.plaintext_exists()
    }

    fn unlock(&self, passphrase: &str) -> Result<(), WalletError> {
        if !self.encrypted_exists() {
            return Err(WalletError::SecretStore(
                "no encrypted mnemonic to unlock".into(),
            ));
        }
        let blob = fs::read(&self.path).map_err(|e| WalletError::SecretStore(e.to_string()))?;
        let mut opened = Self::open(passphrase, &blob)?;
        if opened.legacy {
            // Transparent v1 → v2 upgrade: fresh random sealing key, stronger KDF.
            // Only adopt the new key once the rewritten file verifies; on any
            // failure keep the legacy key so the on-disk v1 file stays usable.
            let mut new_key = [0u8; KEY_LEN];
            rand::thread_rng().fill_bytes(&mut new_key);
            let upgraded = Self::seal(passphrase, &opened.mnemonic, &new_key)
                .and_then(|blob2| {
                    let check = Self::open(passphrase, &blob2)?;
                    if check.mnemonic.trim() != opened.mnemonic.trim() {
                        return Err(WalletError::SecretStore("upgrade verify failed".into()));
                    }
                    write_bytes(&self.path, &blob2)
                });
            match upgraded {
                Ok(()) => {
                    opened.sealing_key = Zeroizing::new(new_key);
                    eprintln!("secret store: upgraded mnemonic file to v2 format");
                }
                Err(e) => {
                    new_key.zeroize();
                    eprintln!("secret store: v2 upgrade failed, staying on v1 ({e})");
                }
            }
        }
        let mut inner = self.lock_inner()?;
        inner.unlocked_mnemonic = Some(opened.mnemonic);
        inner.sealing_key = Some(opened.sealing_key);
        Ok(())
    }

    fn lock(&self) {
        if let Ok(mut inner) = self.lock_inner() {
            if let Some(ref mut m) = inner.unlocked_mnemonic {
                m.zeroize();
            }
            inner.unlocked_mnemonic = None;
            if let Some(ref mut k) = inner.sealing_key {
                k.zeroize();
            }
            inner.sealing_key = None;
        }
    }

    fn migrate_encrypt(&self, passphrase: &str) -> Result<(), WalletError> {
        if self.encrypted_exists() {
            // Encrypted wins; drop leftover plaintext after verifying we can unlock.
            self.unlock(passphrase)?;
            let _ = remove_if_exists(&self.plaintext_path);
            return Ok(());
        }
        let plain = match fs::read_to_string(&self.plaintext_path) {
            Ok(s) => s.trim().to_string(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(WalletError::MissingMnemonic);
            }
            Err(e) => return Err(WalletError::SecretStore(e.to_string())),
        };
        if plain.is_empty() {
            return Err(WalletError::MissingMnemonic);
        }
        self.set_with_passphrase(passphrase, &plain)?;
        Ok(())
    }

    fn sealing_key(&self) -> Option<[u8; 32]> {
        self.lock_inner()
            .ok()
            .and_then(|g| g.sealing_key.as_ref().map(|k| **k))
    }
}

/// In-memory store for tests (never use in production).
#[derive(Default)]
pub struct MemoryStore {
    mnemonic: Mutex<Option<String>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemoryStore {
    fn set_mnemonic(&self, mnemonic: &str) -> Result<(), WalletError> {
        *self
            .mnemonic
            .lock()
            .map_err(|_| WalletError::SecretStore("memory store lock poisoned".into()))? =
            Some(mnemonic.to_string());
        Ok(())
    }

    fn get_mnemonic(&self) -> Result<Option<String>, WalletError> {
        Ok(self
            .mnemonic
            .lock()
            .map_err(|_| WalletError::SecretStore("memory store lock poisoned".into()))?
            .clone())
    }

    fn delete_mnemonic(&self) -> Result<(), WalletError> {
        *self
            .mnemonic
            .lock()
            .map_err(|_| WalletError::SecretStore("memory store lock poisoned".into()))? = None;
        Ok(())
    }
}

pub(crate) fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), WalletError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    // Create with 0600 atomically rather than chmod-after-create, so the file
    // is never observable with default (umask) permissions.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| WalletError::SecretStore(e.to_string()))?;
    // mode() only applies on create; normalize pre-existing files too.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|e| WalletError::SecretStore(e.to_string()))?;
    }
    file.write_all(bytes)
        .map_err(|e| WalletError::SecretStore(e.to_string()))?;
    file.sync_all()
        .map_err(|e| WalletError::SecretStore(e.to_string()))?;
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), WalletError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(WalletError::SecretStore(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn encrypted_roundtrip_and_wrong_passphrase() {
        let dir = tempdir().unwrap();
        let store = EncryptedFileSecretStore::new(
            dir.path().join("wallet.mnemonic.enc"),
            dir.path().join("wallet.mnemonic"),
        );
        store
            .set_with_passphrase("correct horse", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
            .unwrap();
        assert!(!store.is_locked());
        store.lock();
        assert!(store.is_locked());
        assert!(matches!(
            store.unlock("wrong"),
            Err(WalletError::IncorrectPassphrase)
        ));
        store.unlock("correct horse").unwrap();
        let got = store.get_mnemonic().unwrap().unwrap();
        assert!(got.starts_with("abandon"));
    }

    /// Build a legacy v1 blob (Argon2id 19 MiB/t=2, mnemonic-only payload).
    fn seal_v1(passphrase: &str, mnemonic: &str) -> Vec<u8> {
        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut salt);
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let key = EncryptedFileSecretStore::derive_key_v1(passphrase, &salt).unwrap();
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), mnemonic.trim().as_bytes())
            .unwrap();
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC_V1);
        out.extend_from_slice(&salt);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        out
    }

    #[test]
    fn v1_file_unlocks_and_upgrades_to_v2() {
        let dir = tempdir().unwrap();
        let enc = dir.path().join("wallet.mnemonic.enc");
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        fs::write(&enc, seal_v1("pass", mnemonic)).unwrap();
        let store =
            EncryptedFileSecretStore::new(&enc, dir.path().join("wallet.mnemonic"));
        store.unlock("pass").unwrap();
        assert_eq!(store.get_mnemonic().unwrap().unwrap(), mnemonic);
        let key_after_upgrade = store.sealing_key().unwrap();
        // File must now be v2 and unlock again with the same passphrase and
        // the same (persisted) sealing key.
        let blob = fs::read(&enc).unwrap();
        assert_eq!(&blob[..8], MAGIC_V2);
        store.lock();
        store.unlock("pass").unwrap();
        assert_eq!(store.sealing_key().unwrap(), key_after_upgrade);
        assert!(matches!(
            store.unlock("wrong"),
            Err(WalletError::IncorrectPassphrase)
        ));
    }

    #[test]
    fn sealing_key_survives_reencryption() {
        let dir = tempdir().unwrap();
        let store = EncryptedFileSecretStore::new(
            dir.path().join("wallet.mnemonic.enc"),
            dir.path().join("wallet.mnemonic"),
        );
        store.set_with_passphrase("pw", "abandon ability able").unwrap();
        let key = store.sealing_key().unwrap();
        store.lock();
        store.unlock("pw").unwrap();
        assert_eq!(store.sealing_key().unwrap(), key);
    }

    #[test]
    fn migrate_plaintext_then_delete() {
        let dir = tempdir().unwrap();
        let plain = dir.path().join("wallet.mnemonic");
        let enc = dir.path().join("wallet.mnemonic.enc");
        fs::write(&plain, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
        let store = EncryptedFileSecretStore::new(&enc, &plain);
        assert!(store.needs_migration());
        store.migrate_encrypt("secret").unwrap();
        assert!(!plain.exists());
        assert!(enc.exists());
        store.lock();
        store.unlock("secret").unwrap();
    }
}
