use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::WalletError;

/// Stores the wallet mnemonic outside SQLite.
pub trait SecretStore: Send + Sync {
    fn set_mnemonic(&self, mnemonic: &str) -> Result<(), WalletError>;
    fn get_mnemonic(&self) -> Result<Option<String>, WalletError>;
    fn delete_mnemonic(&self) -> Result<(), WalletError>;
}

/// File-backed mnemonic store (mode `0600`).
///
/// Used instead of macOS Keychain for now: `keyring` 3 + security-framework on current
/// macOS accepts `set_password` but does not persist across `Entry` instances / process
/// restarts (get returns `NoEntry`). App Support + 0600 matches the "never in sqlite"
/// boundary until Keychain storage is reliable again.
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
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .map_err(|e| WalletError::SecretStore(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600))
                .map_err(|e| WalletError::SecretStore(e.to_string()))?;
        }
        file.write_all(mnemonic.trim().as_bytes())
            .map_err(|e| WalletError::SecretStore(e.to_string()))?;
        file.sync_all()
            .map_err(|e| WalletError::SecretStore(e.to_string()))?;
        Ok(())
    }

    fn get_mnemonic(&self) -> Result<Option<String>, WalletError> {
        match fs::read_to_string(&self.path) {
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
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(WalletError::SecretStore(e.to_string())),
        }
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
