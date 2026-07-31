use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use bdk_wallet::bitcoin::{Address, Amount, FeeRate};
use bdk_wallet::psbt::PsbtUtils;
use bdk_wallet::rusqlite::Connection;
use bdk_wallet::{KeychainKind, PersistedWallet, SignOptions};

use crate::descriptors::{self, create_params, load_params, parse_mnemonic};
use crate::dto::{
    CreateWalletRequest, CreateWalletResponse, RestoreWalletRequest, SendRequest, SendResult,
    SyncResult, WalletSummary,
};
use crate::electrum::{self, BATCH_SIZE, STOP_GAP};
use crate::error::WalletError;
use crate::meta::{self, WalletMeta};
use crate::network::WalletNetwork;
use crate::secrets::{FileSecretStore, SecretStore};
use crate::MNEMONIC_FILE;

struct WalletState {
    wallet: PersistedWallet<Connection>,
    db: Connection,
    network: WalletNetwork,
    electrum_url: String,
    data_dir: PathBuf,
    needs_full_scan: bool,
}

/// Application-facing wallet handle. BDK types stay private.
pub struct WalletApp {
    state: Mutex<Option<WalletState>>,
    secrets: Arc<dyn SecretStore>,
}

impl WalletApp {
    /// Create a wallet app with file-backed mnemonic storage under `data_dir`.
    pub fn new(data_dir: &Path) -> Self {
        Self::with_secrets(Arc::new(FileSecretStore::new(data_dir.join(MNEMONIC_FILE))))
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

    /// Delete wallet files and mnemonic secret. Safe if already absent.
    pub fn wipe(&self, data_dir: &Path) -> Result<(), WalletError> {
        *self.lock_state()? = None;
        self.secrets.delete_mnemonic()?;
        meta::remove_wallet_files(data_dir)?;
        Ok(())
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
            .ok_or(WalletError::MissingMnemonic)?;
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
            data_dir: data_dir.to_path_buf(),
            needs_full_scan: meta.needs_full_scan,
        };
        let summary = build_summary(&mut state)?;
        *self.lock_state()? = Some(state);
        Ok(summary)
    }

    pub fn summary(&self) -> Result<WalletSummary, WalletError> {
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        build_summary(state)
    }

    pub fn receive_address(&self) -> Result<String, WalletError> {
        let mut guard = self.lock_state()?;
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
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;

        let client = electrum::connect(&state.electrum_url)?;
        client.populate_tx_cache(state.wallet.tx_graph().full_txs().map(|tx_node| tx_node.tx));

        let tx_count_before = state.wallet.transactions().count();
        let did_full_scan = state.needs_full_scan;

        if did_full_scan {
            let request = state.wallet.start_full_scan();
            let update = client
                .full_scan(request, STOP_GAP, BATCH_SIZE, false)
                .map_err(|e| WalletError::Electrum(e.to_string()))?;
            state
                .wallet
                .apply_update(update)
                .map_err(|e| WalletError::Electrum(e.to_string()))?;
        } else {
            let request = state.wallet.start_sync_with_revealed_spks();
            let update = client
                .sync(request, BATCH_SIZE, false)
                .map_err(|e| WalletError::Electrum(e.to_string()))?;
            state
                .wallet
                .apply_update(update)
                .map_err(|e| WalletError::Electrum(e.to_string()))?;
        }

        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;

        if did_full_scan {
            state.needs_full_scan = false;
            let meta = WalletMeta {
                network: state.network,
                electrum_url: state.electrum_url.clone(),
                needs_full_scan: false,
            };
            meta::write_meta(&state.data_dir, &meta)?;
        }

        let tx_count_after = state.wallet.transactions().count();
        let new_txs = tx_count_after.saturating_sub(tx_count_before) as u32;
        let summary = build_summary(state)?;
        Ok(SyncResult { summary, new_txs })
    }

    pub fn send(&self, req: SendRequest) -> Result<SendResult, WalletError> {
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;

        let network = state.network.to_bitcoin_network();
        let address = Address::from_str(&req.address)
            .map_err(|e| WalletError::InvalidAddress(e.to_string()))?
            .require_network(network)
            .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;

        let fee_rate = FeeRate::from_sat_per_vb(req.fee_rate_sat_vb)
            .ok_or_else(|| WalletError::BuildTx("fee_rate_sat_vb must be non-zero".into()))?;
        let amount = Amount::from_sat(req.amount_sats);

        let mut tx_builder = state.wallet.build_tx();
        tx_builder.add_recipient(address.script_pubkey(), amount);
        tx_builder.fee_rate(fee_rate);
        let mut psbt = tx_builder
            .finish()
            .map_err(|e| WalletError::BuildTx(e.to_string()))?;

        let finalized = state
            .wallet
            .sign(&mut psbt, SignOptions::default())
            .map_err(|e| WalletError::Sign(e.to_string()))?;
        if !finalized {
            return Err(WalletError::Sign("transaction not fully signed".into()));
        }

        let fee_sats = psbt
            .fee_amount()
            .ok_or_else(|| WalletError::BuildTx("unable to compute fee".into()))?
            .to_sat();
        let tx = psbt
            .extract_tx()
            .map_err(|e| WalletError::BuildTx(e.to_string()))?;

        let client = electrum::connect(&state.electrum_url)?;
        client
            .transaction_broadcast(&tx)
            .map_err(|e| WalletError::Electrum(e.to_string()))?;

        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;

        Ok(SendResult {
            txid: tx.compute_txid().to_string(),
            fee_sats,
        })
    }

    fn create_or_restore(
        &self,
        data_dir: &Path,
        mnemonic_str: &str,
        network: WalletNetwork,
        electrum_url: Option<String>,
    ) -> Result<WalletSummary, WalletError> {
        if self.exists(data_dir) {
            // Orphaned DB (files without mnemonic secret) — clear and continue.
            if self.secrets.get_mnemonic()?.is_none() {
                self.wipe(data_dir)?;
            } else {
                return Err(WalletError::AlreadyExists);
            }
        }

        let mnemonic = parse_mnemonic(mnemonic_str)?;

        // Persist mnemonic first and verify round-trip before creating the DB.
        self.secrets.set_mnemonic(mnemonic_str)?;
        let stored = self.secrets.get_mnemonic()?;
        if stored.as_deref() != Some(mnemonic_str) {
            let _ = self.secrets.delete_mnemonic();
            return Err(WalletError::SecretStore(
                "secret store did not persist mnemonic".into(),
            ));
        }

        std::fs::create_dir_all(data_dir)?;

        let create_db = || -> Result<(PersistedWallet<Connection>, Connection, WalletMeta), WalletError> {
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

            let meta = WalletMeta::new(network, electrum_url.clone());
            meta::write_meta(data_dir, &meta)?;
            Ok((wallet, db, meta))
        };

        let (wallet, db, meta) = match create_db() {
            Ok(v) => v,
            Err(e) => {
                let _ = meta::remove_wallet_files(data_dir);
                let _ = self.secrets.delete_mnemonic();
                return Err(e);
            }
        };

        let mut state = WalletState {
            wallet,
            db,
            network,
            electrum_url: meta.electrum_url,
            data_dir: data_dir.to_path_buf(),
            needs_full_scan: meta.needs_full_scan,
        };
        let summary = build_summary(&mut state)?;
        *self.lock_state()? = Some(state);
        Ok(summary)
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<WalletState>>, WalletError> {
        self.state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))
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
