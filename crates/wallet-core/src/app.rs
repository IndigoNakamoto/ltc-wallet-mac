use std::path::Path;
use std::sync::{Arc, Mutex};

use bdk_wallet::rusqlite::Connection;
use bdk_wallet::{KeychainKind, PersistedWallet};

use crate::descriptors::{self, create_params, load_params, parse_mnemonic};
use crate::dto::{
    CreateWalletRequest, CreateWalletResponse, RestoreWalletRequest, SendRequest, SendResult,
    SyncResult, WalletSummary,
};
use crate::error::WalletError;
use crate::meta::{self, WalletMeta};
use crate::network::WalletNetwork;
use crate::secrets::{KeyringStore, SecretStore};

struct WalletState {
    wallet: PersistedWallet<Connection>,
    db: Connection,
    network: WalletNetwork,
    #[allow(dead_code)]
    electrum_url: String,
}

/// Application-facing wallet handle. BDK types stay private.
pub struct WalletApp {
    state: Mutex<Option<WalletState>>,
    secrets: Arc<dyn SecretStore>,
}

impl Default for WalletApp {
    fn default() -> Self {
        Self::new()
    }
}

impl WalletApp {
    /// Create a wallet app using the macOS Keychain for mnemonic storage.
    pub fn new() -> Self {
        Self::with_secrets(Arc::new(KeyringStore::new()))
    }

    /// Create a wallet app with a custom secret store (tests use [`crate::secrets::MemoryStore`]).
    pub fn with_secrets(secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            state: Mutex::new(None),
            secrets,
        }
    }

    pub fn exists(&self, data_dir: &Path) -> bool {
        meta::wallet_files_exist(data_dir)
    }

    pub fn create(
        &self,
        data_dir: &Path,
        req: CreateWalletRequest,
    ) -> Result<CreateWalletResponse, WalletError> {
        let mnemonic = descriptors::generate_mnemonic()?;
        let summary = self.create_or_restore(data_dir, &mnemonic, req.network, req.electrum_url)?;
        Ok(CreateWalletResponse { mnemonic, summary })
    }

    pub fn restore(
        &self,
        data_dir: &Path,
        req: RestoreWalletRequest,
    ) -> Result<WalletSummary, WalletError> {
        self.create_or_restore(data_dir, &req.mnemonic, req.network, req.electrum_url)
    }

    pub fn load(&self, data_dir: &Path) -> Result<WalletSummary, WalletError> {
        if !self.exists(data_dir) {
            return Err(WalletError::NotFound);
        }

        let meta = meta::read_meta(data_dir)?;
        let mnemonic_str = self
            .secrets
            .get_mnemonic()?
            .ok_or_else(|| WalletError::SecretStore("mnemonic not found in secret store".into()))?;
        let mnemonic = parse_mnemonic(&mnemonic_str)?;

        let mut db = Connection::open(meta::db_path(data_dir))
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        let params = load_params(&mnemonic, meta.network)?;
        let wallet = PersistedWallet::load(&mut db, params)
            .map_err(|e| WalletError::Persist(e.to_string()))?
            .ok_or(WalletError::NotFound)?;

        let mut state = WalletState {
            wallet,
            db,
            network: meta.network,
            electrum_url: meta.electrum_url,
        };
        let summary = build_summary(&mut state)?;
        *self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))? =
            Some(state);
        Ok(summary)
    }

    pub fn summary(&self) -> Result<WalletSummary, WalletError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        build_summary(state)
    }

    pub fn receive_address(&self) -> Result<String, WalletError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let address = state
            .wallet
            .next_unused_address(KeychainKind::External)
            .to_string();
        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        Ok(address)
    }

    pub fn sync(&self) -> Result<SyncResult, WalletError> {
        Err(WalletError::NotImplemented("sync"))
    }

    pub fn send(&self, _req: SendRequest) -> Result<SendResult, WalletError> {
        Err(WalletError::NotImplemented("send"))
    }

    fn create_or_restore(
        &self,
        data_dir: &Path,
        mnemonic_str: &str,
        network: WalletNetwork,
        electrum_url: Option<String>,
    ) -> Result<WalletSummary, WalletError> {
        if self.exists(data_dir) {
            return Err(WalletError::AlreadyExists);
        }

        let mnemonic = parse_mnemonic(mnemonic_str)?;
        std::fs::create_dir_all(data_dir)?;

        let db_path = meta::db_path(data_dir);
        let mut db =
            Connection::open(&db_path).map_err(|e| WalletError::Persist(e.to_string()))?;
        let params = create_params(&mnemonic, network)?;
        let mut wallet = PersistedWallet::create(&mut db, params)
            .map_err(|e| WalletError::Persist(e.to_string()))?;

        let _ = wallet.next_unused_address(KeychainKind::External);
        wallet
            .persist(&mut db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;

        let meta = WalletMeta::new(network, electrum_url);
        meta::write_meta(data_dir, &meta)?;
        self.secrets.set_mnemonic(mnemonic_str)?;

        let mut state = WalletState {
            wallet,
            db,
            network,
            electrum_url: meta.electrum_url,
        };
        let summary = build_summary(&mut state)?;
        *self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))? =
            Some(state);
        Ok(summary)
    }
}

fn build_summary(state: &mut WalletState) -> Result<WalletSummary, WalletError> {
    let balance = state.wallet.balance();
    let tip_height = state.wallet.local_chain().tip().height();
    let receive_address = state
        .wallet
        .next_unused_address(KeychainKind::External)
        .to_string();
    state
        .wallet
        .persist(&mut state.db)
        .map_err(|e| WalletError::Persist(e.to_string()))?;

    Ok(WalletSummary {
        network: state.network,
        confirmed_sats: balance.confirmed.to_sat(),
        trusted_pending_sats: balance.trusted_pending.to_sat(),
        untrusted_pending_sats: balance.untrusted_pending.to_sat(),
        immature_sats: balance.immature.to_sat(),
        total_sats: balance.total().to_sat(),
        tip_height,
        receive_address,
    })
}
