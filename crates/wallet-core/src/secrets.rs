use std::sync::Mutex;

use crate::error::WalletError;

/// Stores the wallet mnemonic outside SQLite (Keychain / in-memory for tests).
pub trait SecretStore: Send + Sync {
    fn set_mnemonic(&self, mnemonic: &str) -> Result<(), WalletError>;
    fn get_mnemonic(&self) -> Result<Option<String>, WalletError>;
    fn delete_mnemonic(&self) -> Result<(), WalletError>;
}

const KEYRING_SERVICE: &str = "com.indigonakamoto.ltc-wallet";
const KEYRING_USER: &str = "mnemonic";

/// macOS Keychain-backed mnemonic store.
pub struct KeyringStore {
    service: String,
    user: String,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self {
            service: KEYRING_SERVICE.to_string(),
            user: KEYRING_USER.to_string(),
        }
    }

    pub fn with_identity(service: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            user: user.into(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, WalletError> {
        keyring::Entry::new(&self.service, &self.user)
            .map_err(|e| WalletError::SecretStore(e.to_string()))
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringStore {
    fn set_mnemonic(&self, mnemonic: &str) -> Result<(), WalletError> {
        self.entry()?
            .set_password(mnemonic)
            .map_err(|e| WalletError::SecretStore(e.to_string()))
    }

    fn get_mnemonic(&self) -> Result<Option<String>, WalletError> {
        match self.entry()?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(WalletError::SecretStore(e.to_string())),
        }
    }

    fn delete_mnemonic(&self) -> Result<(), WalletError> {
        match self.entry()?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
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
