use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

use bdk_mweb::mweb_sync::SyncProgress;

use bdk_wallet::bitcoin::consensus::encode::deserialize;
use bdk_wallet::bitcoin::{Address, Amount, FeeRate, OutPoint, Psbt, ScriptBuf, Txid};
use bdk_wallet::chain::ChainPosition;
use bdk_wallet::psbt::PsbtUtils;
use bdk_wallet::rusqlite::Connection;
use bdk_wallet::{KeychainKind, PersistedWallet, SignOptions};

use crate::descriptors::{self, create_params, load_params};
use crate::dto::{
    AddressReuseHint, CombinedSummary, CreateWalletRequest, CreateWalletResponse, FeeEstimate,
    ContactRecord, DeleteContactRequest, MigrateEncryptRequest, MwebBroadcastResult, MwebScheme,
    MwebSendPreview, MwebSendRequest, PeginPreview, PeginRequest, PeginResult, PegoutPreview,
    PegoutRequest, RestoreWalletRequest, SendPreview, SendRequest, SendResult, SetTxLabelRequest,
    SetUtxoLockedRequest, SyncResult, TxKind, TxRecord, UpsertContactRequest, UtxoRecord,
    UnlockRequest, UpdateSettingsRequest, WalletSettings, WalletSummary, DEFAULT_MWEB_FEE_SATS,
};
use crate::contacts;
use crate::labels;
use crate::electrum::{self, BATCH_SIZE, MIN_FEE_RATE_SAT_VB, STOP_GAP};
use crate::error::WalletError;
use crate::meta::{self, WalletMeta};
use crate::mweb::{self, MwebRuntime};
use crate::network::WalletNetwork;
use crate::secrets::{EncryptedFileSecretStore, SecretStore, UnlockableSecretStore};
use crate::seed::MasterSecret;
use crate::{MNEMONIC_ENC_FILE, MNEMONIC_FILE};

struct WalletState {
    wallet: PersistedWallet<Connection>,
    db: Connection,
    network: WalletNetwork,
    electrum_url: String,
    /// Verify TLS certificates on ssl:// Electrum servers.
    electrum_validate_domain: bool,
    /// Fall back to built-in public servers when the configured one is down.
    electrum_use_public_fallback: bool,
    /// Lock the wallet after this many idle minutes (0 = never); enforced by the UI.
    auto_lock_minutes: u32,
    /// Server that most recently worked this session (tried first to avoid
    /// re-paying a connect timeout on a dead configured server).
    active_electrum_url: Option<String>,
    litecoin_rpc_url: Option<String>,
    mweb_peers: Vec<String>,
    mweb_scheme: MwebScheme,
    explorer_base_url: String,
    show_fiat: bool,
    use_explorer_fee_hints: bool,
    data_dir: PathBuf,
    needs_full_scan: bool,
    needs_mweb_scan: bool,
    mweb: Option<MwebRuntime>,
    last_tip_height: u32,
}

/// Snapshot the persistable subset of `state` (single source of truth for
/// every `write_meta` call).
fn meta_from_state(state: &WalletState) -> WalletMeta {
    WalletMeta {
        network: state.network,
        electrum_url: state.electrum_url.clone(),
        electrum_validate_domain: state.electrum_validate_domain,
        electrum_use_public_fallback: state.electrum_use_public_fallback,
        auto_lock_minutes: state.auto_lock_minutes,
        needs_full_scan: state.needs_full_scan,
        needs_mweb_scan: state.needs_mweb_scan,
        litecoin_rpc_url: state.litecoin_rpc_url.clone(),
        mweb_peers: state.mweb_peers.clone(),
        mweb_scheme: state.mweb_scheme,
        explorer_base_url: state.explorer_base_url.clone(),
        show_fiat: state.show_fiat,
        use_explorer_fee_hints: state.use_explorer_fee_hints,
    }
}

/// Candidate Electrum URLs in try order: last-good this session, configured,
/// then (unless the user opted out) the built-in public defaults.
fn electrum_candidates(state: &WalletState) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    if let Some(active) = &state.active_electrum_url {
        urls.push(active.clone());
    }
    if !urls.contains(&state.electrum_url) {
        urls.push(state.electrum_url.clone());
    }
    if state.electrum_use_public_fallback {
        for default in state.network.default_electrum_urls() {
            let default = default.to_string();
            if !urls.contains(&default) {
                urls.push(default);
            }
        }
    }
    urls
}

/// Connect with fallback across [`electrum_candidates`], remembering what worked.
fn connect_electrum(state: &mut WalletState) -> Result<crate::electrum::ElectrumClient, WalletError> {
    let candidates = electrum_candidates(state);
    let (client, used) = electrum::connect_first(&candidates, state.electrum_validate_domain)?;
    if used != state.electrum_url {
        eprintln!("electrum: configured server unavailable; using fallback {used}");
    }
    state.active_electrum_url = Some(used);
    Ok(client)
}

/// Application-facing wallet handle. BDK types stay private.
pub struct WalletApp {
    state: Mutex<Option<WalletState>>,
    secrets: Arc<EncryptedFileSecretStore>,
    /// Shared MWEB download progress; lives outside the state mutex so it can be
    /// polled while a sync (which holds that mutex) is in flight.
    mweb_progress: Arc<SyncProgress>,
    mweb_sync_active: AtomicBool,
    /// Set when [`Self::lock`] cannot take the state mutex (e.g. sync in flight).
    /// The holder must drop in-memory wallet state before returning.
    pending_lock: AtomicBool,
}

impl WalletApp {
    /// Create a wallet app with encrypted file-backed mnemonic storage under `data_dir`.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            state: Mutex::new(None),
            secrets: Arc::new(EncryptedFileSecretStore::new(
                data_dir.join(MNEMONIC_ENC_FILE),
                data_dir.join(MNEMONIC_FILE),
            )),
            mweb_progress: Arc::new(SyncProgress::default()),
            mweb_sync_active: AtomicBool::new(false),
            pending_lock: AtomicBool::new(false),
        }
    }

    /// Progress of the current MWEB UTXO download. Does not take the wallet state
    /// lock, so it is safe to poll while [`Self::sync`] or [`Self::resync_mweb`] runs.
    pub fn mweb_sync_progress(&self) -> crate::dto::MwebSyncProgress {
        let (fetched, total) = self.mweb_progress.snapshot();
        crate::dto::MwebSyncProgress {
            active: self.mweb_sync_active.load(Ordering::Relaxed),
            fetched,
            total,
        }
    }

    /// Reset shared progress and mark an MWEB pass active/inactive around `f`.
    fn with_mweb_progress<R>(&self, f: impl FnOnce(Arc<SyncProgress>) -> R) -> R {
        self.mweb_progress.fetched.store(0, Ordering::Relaxed);
        self.mweb_progress.total.store(0, Ordering::Relaxed);
        self.mweb_sync_active.store(true, Ordering::Relaxed);
        let out = f(Arc::clone(&self.mweb_progress));
        self.mweb_sync_active.store(false, Ordering::Relaxed);
        out
    }

    /// Test helper using an in-memory secret store (no encryption).
    pub fn with_secrets(secrets: Arc<dyn SecretStore>) -> MemoryBackedApp {
        MemoryBackedApp {
            state: Mutex::new(None),
            secrets,
        }
    }

    pub fn exists(&self, data_dir: &Path) -> bool {
        meta::wallet_files_exist(data_dir)
    }

    pub fn is_locked(&self) -> bool {
        self.secrets.is_locked()
    }

    pub fn needs_migration(&self) -> bool {
        self.secrets.needs_migration()
    }

    pub fn unlock(&self, req: UnlockRequest) -> Result<(), WalletError> {
        self.secrets.unlock(&req.passphrase)
    }

    /// Lock immediately: wipe the decrypted mnemonic, then drop in-memory wallet
    /// state if the state mutex is free. When sync (or another long op) holds the
    /// mutex, set [`Self::pending_lock`] so that holder discards state before return.
    ///
    /// Does **not** block on an in-flight sync.
    pub fn lock(&self) {
        self.secrets.lock();
        self.pending_lock.store(true, Ordering::Release);
        match self.state.try_lock() {
            Ok(mut guard) => {
                *guard = None;
                self.pending_lock.store(false, Ordering::Release);
            }
            Err(TryLockError::WouldBlock) => {
                // Sync/send holds the state; secrets are already cleared.
            }
            Err(TryLockError::Poisoned(poisoned)) => {
                let mut guard = poisoned.into_inner();
                *guard = None;
                self.pending_lock.store(false, Ordering::Release);
            }
        }
    }

    /// If the user locked while we held `guard`, drop wallet state and report locked.
    fn abort_if_locked(
        &self,
        guard: &mut MutexGuard<'_, Option<WalletState>>,
    ) -> Result<(), WalletError> {
        if !self.pending_lock.load(Ordering::Acquire) && !self.secrets.is_locked() {
            return Ok(());
        }
        **guard = None;
        self.secrets.lock();
        self.pending_lock.store(false, Ordering::Release);
        Err(WalletError::Locked)
    }

    pub fn migrate_encrypt(&self, req: MigrateEncryptRequest) -> Result<(), WalletError> {
        self.secrets.migrate_encrypt(&req.passphrase)
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
        passphrase: &str,
    ) -> Result<CreateWalletResponse, WalletError> {
        let mnemonic = descriptors::generate_mnemonic()?;
        let secret = MasterSecret::parse(&mnemonic, None)?;
        let summary = self.create_or_restore(
            data_dir,
            &secret,
            req.network,
            req.electrum_url,
            passphrase,
            MwebScheme::default(),
        )?;
        Ok(CreateWalletResponse { mnemonic, summary })
    }

    pub fn restore(
        &self,
        data_dir: &Path,
        req: RestoreWalletRequest,
        passphrase: &str,
    ) -> Result<WalletSummary, WalletError> {
        let secret = MasterSecret::parse(&req.mnemonic, req.aezeed_passphrase.as_deref())?;
        self.create_or_restore(
            data_dir,
            &secret,
            req.network,
            req.electrum_url,
            passphrase,
            req.mweb_scheme,
        )
    }

    pub fn load(&self, data_dir: &Path) -> Result<WalletSummary, WalletError> {
        self.ensure_unlocked()?;
        if !self.exists(data_dir) {
            return Err(WalletError::NotFound);
        }

        let meta = meta::read_meta(data_dir)?;
        let stored = self
            .secrets
            .get_mnemonic()?
            .ok_or(WalletError::MissingMnemonic)?;
        let secret = MasterSecret::from_stored(&stored)?;

        let mut db = Connection::open(meta::db_path(data_dir))
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        let params = load_params(&secret, meta.network)?;
        let wallet = PersistedWallet::load(&mut db, params)
            .map_err(|e| WalletError::Persist(e.to_string()))?
            .ok_or(WalletError::NotFound)?;

        let mweb = MwebRuntime::open(
            data_dir,
            &secret,
            meta.network,
            meta.mweb_scheme.to_master_scheme(),
            self.secrets.sealing_key(),
        )
        .ok();

        let mut state = WalletState {
            last_tip_height: wallet.latest_checkpoint().height(),
            wallet,
            db,
            network: meta.network,
            electrum_url: meta.electrum_url,
            electrum_validate_domain: meta.electrum_validate_domain,
            electrum_use_public_fallback: meta.electrum_use_public_fallback,
            auto_lock_minutes: meta.auto_lock_minutes,
            active_electrum_url: None,
            litecoin_rpc_url: meta.litecoin_rpc_url,
            mweb_peers: meta.mweb_peers,
            mweb_scheme: meta.mweb_scheme,
            explorer_base_url: meta.explorer_base_url,
            show_fiat: meta.show_fiat,
            use_explorer_fee_hints: meta.use_explorer_fee_hints,
            data_dir: data_dir.to_path_buf(),
            needs_full_scan: meta.needs_full_scan,
            needs_mweb_scan: meta.needs_mweb_scan,
            mweb,
        };
        let summary = build_summary(&mut state)?;
        *self.lock_state()? = Some(state);
        Ok(summary)
    }

    pub fn summary(&self) -> Result<WalletSummary, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        build_summary(state)
    }

    pub fn combined_summary(&self) -> Result<CombinedSummary, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let transparent = build_summary(state)?;
        if let Some(ref mut mweb) = state.mweb {
            mweb.combined_summary(&state.wallet, transparent)
        } else {
            Ok(CombinedSummary {
                transparent,
                mweb_confirmed_sats: 0,
                mweb_unconfirmed_sats: 0,
                mweb_immature_sats: 0,
                mweb_total_sats: 0,
                mweb_receive_address: None,
                mweb_synced_height: None,
                mweb_stale: true,
                mweb_status: "MWEB unavailable".into(),
            })
        }
    }

    pub fn transactions(&self) -> Result<Vec<TxRecord>, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let tip = state.wallet.local_chain().tip().height();
        let mut records: Vec<TxRecord> = state
            .wallet
            .transactions()
            .map(|tx| {
                let (sent, received) = state.wallet.sent_and_received(&tx.tx);
                let fee_sats = state.wallet.calculate_fee(&tx.tx).ok().map(|f| f.to_sat());
                let sent_sats = sent.to_sat();
                let received_sats = received.to_sat();
                let net_sats = received_sats as i64 - sent_sats as i64;
                let (height, confirmations, timestamp) = match &tx.pos {
                    ChainPosition::Confirmed { anchor, .. } => {
                        let h = anchor.block_id.height;
                        let confs = tip.saturating_sub(h).saturating_add(1);
                        (Some(h), confs, Some(anchor.confirmation_time))
                    }
                    ChainPosition::Unconfirmed { first_seen, .. } => (None, 0, *first_seen),
                };
                TxRecord {
                    txid: tx.txid.to_string(),
                    net_sats,
                    sent_sats,
                    received_sats,
                    fee_sats,
                    height,
                    confirmations,
                    timestamp,
                    kind: TxKind::Transparent,
                }
            })
            .collect();
        if let Some(ref mweb) = state.mweb {
            merge_mweb_history(&mut records, mweb, tip);
        }
        records.sort_by(|a, b| match (a.height, b.height) {
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, None) => b.timestamp.cmp(&a.timestamp),
            (Some(ha), Some(hb)) => hb.cmp(&ha).then_with(|| b.txid.cmp(&a.txid)),
        });
        Ok(records)
    }

    /// Reveal and persist the next external receive address.
    ///
    /// Always advances the derivation index so callers (UI "New address", CLI)
    /// get a fresh address even when earlier revealed addresses are still unused.
    pub fn receive_address(&self) -> Result<String, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let address = state
            .wallet
            .reveal_next_address(KeychainKind::External)
            .to_string();
        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        Ok(address)
    }

    pub fn mweb_receive_address(&self) -> Result<String, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let mweb = state.mweb.as_mut().ok_or_else(|| {
            WalletError::Mweb("MWEB runtime not initialized".into())
        })?;
        let addr = mweb.receive_address(state.network)?;
        mweb.persist(&state.data_dir)?;
        Ok(addr)
    }

    pub fn settings(&self) -> Result<WalletSettings, WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        Ok(WalletSettings {
            electrum_url: state.electrum_url.clone(),
            electrum_validate_domain: state.electrum_validate_domain,
            electrum_use_public_fallback: state.electrum_use_public_fallback,
            auto_lock_minutes: state.auto_lock_minutes,
            electrum_active_url: state.active_electrum_url.clone(),
            litecoin_rpc_url: state.litecoin_rpc_url.clone(),
            mweb_peers: state.mweb_peers.clone(),
            mweb_scheme: state.mweb_scheme,
            explorer_base_url: state.explorer_base_url.clone(),
            show_fiat: state.show_fiat,
            use_explorer_fee_hints: state.use_explorer_fee_hints,
        })
    }

    /// True when `address` is a revealed BIP84 receive/change address that has been used.
    /// MWEB stealth addresses always return `reused: false` (reuse is expected).
    pub fn address_reuse_hint(&self, address: &str) -> Result<AddressReuseHint, WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        Ok(compute_address_reuse_hint(&state.wallet, address))
    }

    pub fn get_tx_labels(&self) -> Result<std::collections::HashMap<String, String>, WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        Ok(labels::read_labels(&state.data_dir)?.labels)
    }

    pub fn set_tx_label(&self, req: SetTxLabelRequest) -> Result<(), WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        labels::set_label(&state.data_dir, &req.txid, &req.label)
    }

    /// Non-secret history export (CSV or JSON). Excludes mnemonic and settings secrets.
    pub fn export_history(
        &self,
        format: crate::HistoryExportFormat,
    ) -> Result<String, WalletError> {
        let txs = self.transactions()?;
        let labels = self.get_tx_labels()?;
        crate::history_export::render(format, &txs, &labels)
    }

    pub fn list_contacts(&self) -> Result<Vec<ContactRecord>, WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        Ok(contacts::read_contacts(&state.data_dir)?.contacts)
    }

    pub fn upsert_contact(&self, req: UpsertContactRequest) -> Result<ContactRecord, WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        contacts::upsert_contact(
            &state.data_dir,
            state.network,
            req.id.as_deref(),
            &req.name,
            &req.address,
            req.kind,
        )
    }

    pub fn delete_contact(&self, req: DeleteContactRequest) -> Result<(), WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        contacts::delete_contact(&state.data_dir, &req.id)
    }

    /// List transparent unspent outputs for coin control.
    pub fn list_unspent(&self) -> Result<Vec<UtxoRecord>, WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        Ok(collect_unspent(state))
    }

    /// Freeze or unfreeze a transparent UTXO (persisted in the wallet DB).
    pub fn set_utxo_locked(&self, req: SetUtxoLockedRequest) -> Result<(), WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let outpoint = parse_outpoint(&req.outpoint)?;
        if req.locked {
            state.wallet.lock_outpoint(outpoint);
        } else {
            state.wallet.unlock_outpoint(outpoint);
        }
        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        Ok(())
    }

    pub fn update_settings(&self, req: UpdateSettingsRequest) -> Result<(), WalletError> {
        self.ensure_unlocked()?;
        meta::validate_electrum_url(&req.electrum_url)?;
        let explorer_base_url = crate::explorer::normalize_base_url(&req.explorer_base_url)?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        state.electrum_url = req.electrum_url.trim().to_string();
        state.electrum_validate_domain = req.electrum_validate_domain;
        state.electrum_use_public_fallback = req.electrum_use_public_fallback;
        state.auto_lock_minutes = req.auto_lock_minutes;
        // New configured server should be tried first on the next connection.
        state.active_electrum_url = None;
        state.litecoin_rpc_url = req
            .litecoin_rpc_url
            .map(|s| crate::rpc::normalize_rpc_url(&s))
            .filter(|s| !s.is_empty());
        state.mweb_peers = req
            .mweb_peers
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        state.explorer_base_url = explorer_base_url;
        state.show_fiat = req.show_fiat;
        state.use_explorer_fee_hints = req.use_explorer_fee_hints;
        meta::write_meta(&state.data_dir, &meta_from_state(state))?;
        Ok(())
    }

    /// Build a litview `/tx/{txid}` URL (no network).
    pub fn explorer_tx_url(&self, txid: &str) -> Result<String, WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        crate::explorer::tx_url(&state.explorer_base_url, txid)
    }

    /// Build a litview `/block/{hash}` URL (no network).
    pub fn explorer_block_url(&self, block_hash: &str) -> Result<String, WalletError> {
        self.ensure_unlocked()?;
        let guard = self.lock_state()?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        crate::explorer::block_url(&state.explorer_base_url, block_hash)
    }

    /// Fetch tx details from the explorer; mark wallet IO locally.
    pub fn fetch_tx_detail(&self, txid: &str) -> Result<crate::dto::TxEnrichment, WalletError> {
        self.ensure_unlocked()?;
        let (base, addresses) = {
            let guard = self.lock_state()?;
            let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
            (
                state.explorer_base_url.clone(),
                revealed_address_set(&state.wallet),
            )
        };
        crate::explorer::fetch_tx_detail(&base, txid, &addresses)
    }

    /// Spot LTC/USD from the explorer mempool price endpoint.
    pub fn fetch_spot_price(&self) -> Result<f64, WalletError> {
        self.ensure_unlocked()?;
        let base = {
            let guard = self.lock_state()?;
            let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
            state.explorer_base_url.clone()
        };
        crate::explorer::fetch_spot_price(&base)
    }

    /// Fee ladder suggestions from the explorer.
    pub fn fetch_fee_ladder(&self) -> Result<crate::dto::FeeLadder, WalletError> {
        self.ensure_unlocked()?;
        let base = {
            let guard = self.lock_state()?;
            let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
            if !state.use_explorer_fee_hints {
                return Err(WalletError::Meta(
                    "explorer fee hints are disabled in Settings".into(),
                ));
            }
            state.explorer_base_url.clone()
        };
        crate::explorer::fetch_fee_ladder(&base)
    }

    pub fn sync(&self) -> Result<SyncResult, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        self.abort_if_locked(&mut guard)?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;

        let electrum_started = std::time::Instant::now();
        let client = connect_electrum(state)?;
        client.populate_tx_cache(state.wallet.tx_graph().full_txs().map(|tx_node| tx_node.tx));

        let tx_count_before = state.wallet.transactions().count();
        let did_full_scan = state.needs_full_scan;
        let prev_tip = state.last_tip_height;

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
        let electrum_ms = electrum_started.elapsed().as_millis() as u64;

        let tip_height = state.wallet.latest_checkpoint().height();
        let tip_hash = state.wallet.latest_checkpoint().hash();

        if tip_height < prev_tip {
            if let Some(ref mut mweb) = state.mweb {
                mweb.disconnect_from(tip_height.saturating_add(1));
            }
        }
        state.last_tip_height = tip_height;

        if did_full_scan {
            state.needs_full_scan = false;
        }

        // Privacy-preserving second opinion: compare the served chain against
        // an independent server (headers only, never our scripts).
        let mut warnings: Vec<String> = Vec::new();
        let used_server = state
            .active_electrum_url
            .clone()
            .unwrap_or_else(|| state.electrum_url.clone());
        if let Some(alt) = electrum_candidates(state)
            .into_iter()
            .find(|url| *url != used_server)
        {
            let chain = state.wallet.local_chain();
            let local_hash_at = |height: u32| chain.get(height).map(|cp| cp.hash());
            match electrum::cross_check_tip(
                &alt,
                state.electrum_validate_domain,
                tip_height,
                &local_hash_at,
            ) {
                Ok(Some(warning)) => warnings.push(warning),
                Ok(None) => {}
                // Unreachable second server is not a finding; skip quietly.
                Err(_) => {}
            }
        }

        // Skip the long MWEB pass if the user locked during Electrum sync.
        self.abort_if_locked(&mut guard)?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;

        // MWEB sync (best-effort; peer failure → stale, not hard error).
        let mut mweb_ms = 0u64;
        if let Some(ref mut mweb) = state.mweb {
            let mweb_started = std::time::Instant::now();
            let peers = state.mweb_peers.clone();
            let network = state.network;
            // Hash on the current chain at the previous MWEB sync height, so the syncer
            // can detect a pure extension and skip the full UTXO re-download.
            let prev_tip_hash = mweb
                .sync_state
                .tip_height
                .and_then(|h| state.wallet.local_chain().get(h))
                .map(|cp| cp.hash());
            self.with_mweb_progress(|progress| {
                let _ = mweb.sync_at_tip(
                    tip_height,
                    tip_hash,
                    prev_tip_hash,
                    &peers,
                    network,
                    Some(progress),
                );
            });
            if state.needs_mweb_scan {
                state.needs_mweb_scan = false;
            }
            if let Some(warning) = mweb.cross_check_warning.clone() {
                warnings.push(warning);
            }
            let _ = mweb.persist(&state.data_dir);
            mweb_ms = mweb_started.elapsed().as_millis() as u64;
        }

        self.abort_if_locked(&mut guard)?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;

        // Repair peg-ins that credited MWEB without spending transparent UTXOs
        // (bug in earlier builds). Prefer Electrum raw tx; fall back to RPC.
        if state.mweb.is_some() {
            if let Some(repair_warning) = repair_missing_pegin_spends(state, &client)? {
                warnings.push(repair_warning);
            }
        }

        meta::write_meta(&state.data_dir, &meta_from_state(state))?;

        let tx_count_after = state.wallet.transactions().count();
        let new_txs = tx_count_after.saturating_sub(tx_count_before) as u32;
        let summary = build_summary(state)?;
        self.abort_if_locked(&mut guard)?;
        Ok(SyncResult {
            summary,
            new_txs,
            electrum_ms,
            mweb_ms,
            electrum_server: used_server,
            warnings,
        })
    }

    pub fn resync_mweb(&self) -> Result<(), WalletError> {
        self.resync_mweb_inner(None)
    }

    /// Switch the MWEB derivation scheme and rescan under it. Wipes local
    /// MWEB state (coins are re-discovered from the chain); transparent
    /// wallet data is untouched.
    pub fn set_mweb_scheme(&self, scheme: MwebScheme) -> Result<(), WalletError> {
        self.resync_mweb_inner(Some(scheme))
    }

    fn resync_mweb_inner(&self, new_scheme: Option<MwebScheme>) -> Result<(), WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        self.abort_if_locked(&mut guard)?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        mweb::wipe_mweb_files(&state.data_dir)?;
        // A manual resync usually means something looked wrong, so re-crawl
        // rather than reusing peers that may be part of the problem.
        crate::discovery::clear_cache();
        let stored = self
            .secrets
            .get_mnemonic()?
            .ok_or(WalletError::MissingMnemonic)?;
        let secret = MasterSecret::from_stored(&stored)?;
        if let Some(scheme) = new_scheme {
            state.mweb_scheme = scheme;
            let mut meta = meta_from_state(state);
            meta.needs_mweb_scan = true;
            meta::write_meta(&state.data_dir, &meta)?;
        }
        state.mweb = Some(MwebRuntime::open(
            &state.data_dir,
            &secret,
            state.network,
            state.mweb_scheme.to_master_scheme(),
            self.secrets.sealing_key(),
        )?);
        state.needs_mweb_scan = true;
        let tip_height = state.wallet.latest_checkpoint().height();
        let tip_hash = state.wallet.latest_checkpoint().hash();
        if let Some(ref mut mweb) = state.mweb {
            let peers = state.mweb_peers.clone();
            let network = state.network;
            // Fresh runtime after wipe: no previous sync state, so no extension hint.
            self.with_mweb_progress(|progress| {
                let _ = mweb.sync_at_tip(tip_height, tip_hash, None, &peers, network, Some(progress));
            });
            mweb.persist(&state.data_dir)?;
        }
        self.abort_if_locked(&mut guard)?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        state.needs_mweb_scan = false;
        Ok(())
    }

    /// Estimate a network fee rate (sat/vB) from Electrum.
    pub fn estimate_fee(&self) -> Result<FeeEstimate, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let client = connect_electrum(state)?;
        let (fee_rate_sat_vb, is_fallback) = electrum::estimate_fee_rate_sat_vb(&client)?;
        Ok(FeeEstimate {
            fee_rate_sat_vb,
            is_fallback,
        })
    }

    /// Build a send without broadcasting; returns absolute fee and recipient amount.
    pub fn preview_send(&self, req: SendRequest) -> Result<SendPreview, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let (fee_rate_sat_vb, psbt, recipient_script) = build_send_psbt(state, &req)?;
        let fee_sats = psbt
            .fee_amount()
            .ok_or_else(|| WalletError::BuildTx("unable to compute fee".into()))?
            .to_sat();
        let amount_sats = if req.drain {
            psbt.unsigned_tx
                .output
                .iter()
                .find(|o| o.script_pubkey == recipient_script)
                .map(|o| o.value.to_sat())
                .unwrap_or(0)
        } else {
            req.amount_sats
        };
        // Persist any change-address reveal staged by the builder.
        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        Ok(SendPreview {
            amount_sats,
            fee_sats,
            fee_rate_sat_vb,
        })
    }

    pub fn send(&self, req: SendRequest) -> Result<SendResult, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;

        let (_fee_rate_sat_vb, mut psbt, _recipient_script) = build_send_psbt(state, &req)?;

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

        let client = connect_electrum(state)?;
        client
            .transaction_broadcast(&tx)
            .map_err(|e| {
                WalletError::Electrum(crate::error::humanize_broadcast_error(&e.to_string()))
            })?;

        // Keep local balance honest until the next sync sees the broadcast.
        state
            .wallet
            .apply_unconfirmed_txs([(tx.clone(), crate::mweb_history::now_ts())]);

        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;

        Ok(SendResult {
            txid: tx.compute_txid().to_string(),
            fee_sats,
        })
    }

    pub fn preview_pegin(&self, req: PeginRequest) -> Result<PeginPreview, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let resolved = resolve_pegin_request(state, req)?;
        Ok(PeginPreview {
            amount_sats: resolved.amount_sats,
            private_credit_sats: resolved
                .amount_sats
                .saturating_sub(resolved.mweb_fee_sats),
            mweb_fee_sats: resolved.mweb_fee_sats,
            transparent_fee_sats: resolved.transparent_fee_sats,
            total_from_transparent_sats: resolved
                .amount_sats
                .saturating_add(resolved.transparent_fee_sats),
        })
    }

    pub fn pegin(&self, req: PeginRequest) -> Result<PeginResult, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let req = resolve_pegin_request(state, req)?;
        let rpc_url = state.litecoin_rpc_url.clone();
        let peers = state.mweb_peers.clone();
        let network = state.network;
        let mweb = state
            .mweb
            .as_mut()
            .ok_or_else(|| WalletError::Mweb("MWEB runtime not initialized".into()))?;
        let result = mweb::pegin(&mut state.wallet, mweb, rpc_url.as_deref(), &peers, req, network)?;
        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        mweb.persist(&state.data_dir)?;
        Ok(result)
    }

    pub fn preview_mweb_send(&self, req: MwebSendRequest) -> Result<MwebSendPreview, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let resolved = resolve_mweb_send_request(state, req)?;
        Ok(MwebSendPreview {
            amount_sats: resolved.amount_sats,
            fee_sats: resolved.fee_sats,
        })
    }

    pub fn mweb_send(&self, req: MwebSendRequest) -> Result<MwebBroadcastResult, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let req = resolve_mweb_send_request(state, req)?;
        let rpc_url = state.litecoin_rpc_url.clone();
        let peers = state.mweb_peers.clone();
        let mweb = state
            .mweb
            .as_mut()
            .ok_or_else(|| WalletError::Mweb("MWEB runtime not initialized".into()))?;
        let network = state.network;
        let result =
            mweb::mweb_send(&state.wallet, mweb, rpc_url.as_deref(), &peers, req, network)?;
        mweb.persist(&state.data_dir)?;
        Ok(result)
    }

    pub fn preview_pegout(&self, req: PegoutRequest) -> Result<PegoutPreview, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let (resolved, dust_sats) = resolve_pegout_request(state, req)?;
        Ok(PegoutPreview {
            amount_sats: resolved.amount_sats,
            fee_sats: resolved.fee_sats,
            dust_sats,
        })
    }

    pub fn pegout(&self, req: PegoutRequest) -> Result<MwebBroadcastResult, WalletError> {
        self.ensure_unlocked()?;
        let mut guard = self.lock_state()?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let (req, _) = resolve_pegout_request(state, req)?;
        let rpc_url = state.litecoin_rpc_url.clone();
        let peers = state.mweb_peers.clone();
        let mweb = state
            .mweb
            .as_mut()
            .ok_or_else(|| WalletError::Mweb("MWEB runtime not initialized".into()))?;
        let network = state.network;
        let result = mweb::pegout(&state.wallet, mweb, rpc_url.as_deref(), &peers, req, network)?;
        mweb.persist(&state.data_dir)?;
        Ok(result)
    }

    fn create_or_restore(
        &self,
        data_dir: &Path,
        secret: &MasterSecret,
        network: WalletNetwork,
        electrum_url: Option<String>,
        passphrase: &str,
        mweb_scheme: MwebScheme,
    ) -> Result<WalletSummary, WalletError> {
        if passphrase.trim().is_empty() {
            return Err(WalletError::SecretStore("passphrase must not be empty".into()));
        }
        if self.exists(data_dir) {
            if self.secrets.get_mnemonic()?.is_none() && !self.secrets.is_locked() {
                self.wipe(data_dir)?;
            } else if self.secrets.is_locked() || self.secrets.get_mnemonic()?.is_some() {
                return Err(WalletError::AlreadyExists);
            } else {
                self.wipe(data_dir)?;
            }
        }

        self.secrets
            .set_with_passphrase(passphrase, &secret.to_stored())?;

        std::fs::create_dir_all(data_dir)?;

        let create_db = || -> Result<(PersistedWallet<Connection>, Connection, WalletMeta), WalletError> {
            let db_path = meta::db_path(data_dir);
            let mut db =
                Connection::open(&db_path).map_err(|e| WalletError::Persist(e.to_string()))?;
            let params = create_params(secret, network)?;
            let mut wallet = PersistedWallet::create(&mut db, params)
                .map_err(|e| WalletError::Persist(e.to_string()))?;

            let _ = wallet.next_unused_address(KeychainKind::External);
            wallet
                .persist(&mut db)
                .map_err(|e| WalletError::Persist(e.to_string()))?;

            let mut meta = WalletMeta::new(network, electrum_url.clone());
            meta.mweb_scheme = mweb_scheme;
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

        let mweb = MwebRuntime::open(
            data_dir,
            secret,
            network,
            mweb_scheme.to_master_scheme(),
            self.secrets.sealing_key(),
        )
        .ok();

        let mut state = WalletState {
            last_tip_height: wallet.latest_checkpoint().height(),
            wallet,
            db,
            network,
            electrum_url: meta.electrum_url,
            electrum_validate_domain: meta.electrum_validate_domain,
            electrum_use_public_fallback: meta.electrum_use_public_fallback,
            auto_lock_minutes: meta.auto_lock_minutes,
            active_electrum_url: None,
            litecoin_rpc_url: meta.litecoin_rpc_url,
            mweb_peers: meta.mweb_peers,
            mweb_scheme,
            explorer_base_url: meta.explorer_base_url,
            show_fiat: meta.show_fiat,
            use_explorer_fee_hints: meta.use_explorer_fee_hints,
            data_dir: data_dir.to_path_buf(),
            needs_full_scan: meta.needs_full_scan,
            needs_mweb_scan: meta.needs_mweb_scan,
            mweb,
        };
        let summary = build_summary(&mut state)?;
        *self.lock_state()? = Some(state);
        Ok(summary)
    }

    fn ensure_unlocked(&self) -> Result<(), WalletError> {
        if self.secrets.is_locked() {
            Err(WalletError::Locked)
        } else {
            Ok(())
        }
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<WalletState>>, WalletError> {
        self.state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))
    }
}

/// Test-only wallet app backed by an arbitrary [`SecretStore`] (typically [`crate::secrets::MemoryStore`]).
pub struct MemoryBackedApp {
    state: Mutex<Option<WalletState>>,
    secrets: Arc<dyn SecretStore>,
}

impl MemoryBackedApp {
    pub fn exists(&self, data_dir: &Path) -> bool {
        meta::wallet_files_exist(data_dir)
    }

    pub fn wipe(&self, data_dir: &Path) -> Result<(), WalletError> {
        *self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))? = None;
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
        let secret = MasterSecret::parse(&mnemonic, None)?;
        let summary = self.create_or_restore(
            data_dir,
            &secret,
            req.network,
            req.electrum_url,
            MwebScheme::default(),
        )?;
        Ok(CreateWalletResponse { mnemonic, summary })
    }

    pub fn restore(
        &self,
        data_dir: &Path,
        req: RestoreWalletRequest,
    ) -> Result<WalletSummary, WalletError> {
        let secret = MasterSecret::parse(&req.mnemonic, req.aezeed_passphrase.as_deref())?;
        self.create_or_restore(
            data_dir,
            &secret,
            req.network,
            req.electrum_url,
            req.mweb_scheme,
        )
    }

    pub fn load(&self, data_dir: &Path) -> Result<WalletSummary, WalletError> {
        if !self.exists(data_dir) {
            return Err(WalletError::NotFound);
        }
        let meta = meta::read_meta(data_dir)?;
        let stored = self
            .secrets
            .get_mnemonic()?
            .ok_or(WalletError::MissingMnemonic)?;
        let secret = MasterSecret::from_stored(&stored)?;
        let mut db = Connection::open(meta::db_path(data_dir))
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        let params = load_params(&secret, meta.network)?;
        let wallet = PersistedWallet::load(&mut db, params)
            .map_err(|e| WalletError::Persist(e.to_string()))?
            .ok_or(WalletError::NotFound)?;
        let mut state = WalletState {
            last_tip_height: wallet.latest_checkpoint().height(),
            wallet,
            db,
            network: meta.network,
            electrum_url: meta.electrum_url,
            electrum_validate_domain: meta.electrum_validate_domain,
            electrum_use_public_fallback: meta.electrum_use_public_fallback,
            auto_lock_minutes: meta.auto_lock_minutes,
            active_electrum_url: None,
            litecoin_rpc_url: meta.litecoin_rpc_url,
            mweb_peers: meta.mweb_peers,
            mweb_scheme: meta.mweb_scheme,
            explorer_base_url: meta.explorer_base_url,
            show_fiat: meta.show_fiat,
            use_explorer_fee_hints: meta.use_explorer_fee_hints,
            data_dir: data_dir.to_path_buf(),
            needs_full_scan: meta.needs_full_scan,
            needs_mweb_scan: meta.needs_mweb_scan,
            mweb: None,
        };
        let summary = build_summary(&mut state)?;
        *self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))? = Some(state);
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
            .reveal_next_address(KeychainKind::External)
            .to_string();
        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        Ok(address)
    }

    pub fn address_reuse_hint(&self, address: &str) -> Result<AddressReuseHint, WalletError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        Ok(compute_address_reuse_hint(&state.wallet, address))
    }

    pub fn get_tx_labels(&self) -> Result<std::collections::HashMap<String, String>, WalletError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        Ok(labels::read_labels(&state.data_dir)?.labels)
    }

    pub fn set_tx_label(&self, req: SetTxLabelRequest) -> Result<(), WalletError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        labels::set_label(&state.data_dir, &req.txid, &req.label)
    }

    pub fn list_contacts(&self) -> Result<Vec<ContactRecord>, WalletError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        Ok(contacts::read_contacts(&state.data_dir)?.contacts)
    }

    pub fn upsert_contact(&self, req: UpsertContactRequest) -> Result<ContactRecord, WalletError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        contacts::upsert_contact(
            &state.data_dir,
            state.network,
            req.id.as_deref(),
            &req.name,
            &req.address,
            req.kind,
        )
    }

    pub fn delete_contact(&self, req: DeleteContactRequest) -> Result<(), WalletError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        contacts::delete_contact(&state.data_dir, &req.id)
    }

    pub fn list_unspent(&self) -> Result<Vec<UtxoRecord>, WalletError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_ref().ok_or(WalletError::NotLoaded)?;
        Ok(collect_unspent(state))
    }

    pub fn set_utxo_locked(&self, req: SetUtxoLockedRequest) -> Result<(), WalletError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let outpoint = parse_outpoint(&req.outpoint)?;
        if req.locked {
            state.wallet.lock_outpoint(outpoint);
        } else {
            state.wallet.unlock_outpoint(outpoint);
        }
        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        Ok(())
    }

    /// Test helper: mutate the in-memory wallet (for funding UTXOs in integration tests).
    pub fn with_wallet_mut<F, T>(&self, f: F) -> Result<T, WalletError>
    where
        F: FnOnce(&mut PersistedWallet<Connection>, &mut Connection) -> Result<T, WalletError>,
    {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        f(&mut state.wallet, &mut state.db)
    }

    /// Test helper: mark an external derivation index as used for reuse-detection tests.
    pub fn mark_external_used(&self, index: u32) -> Result<(), WalletError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        state.wallet.mark_used(KeychainKind::External, index);
        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        Ok(())
    }

    pub fn send(&self, req: SendRequest) -> Result<SendResult, WalletError> {
        // Minimal send for unit tests that don't hit the network: only build path errors.
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))?;
        let state = guard.as_mut().ok_or(WalletError::NotLoaded)?;
        let _ = build_send_psbt(state, &req)?;
        // Unit tests that reach broadcast should not; return a placeholder.
        Err(WalletError::Electrum(
            "memory-backed test app does not broadcast".into(),
        ))
    }

    fn create_or_restore(
        &self,
        data_dir: &Path,
        secret: &MasterSecret,
        network: WalletNetwork,
        electrum_url: Option<String>,
        mweb_scheme: MwebScheme,
    ) -> Result<WalletSummary, WalletError> {
        if self.exists(data_dir) {
            if self.secrets.get_mnemonic()?.is_none() {
                self.wipe(data_dir)?;
            } else {
                return Err(WalletError::AlreadyExists);
            }
        }
        let payload = secret.to_stored();
        self.secrets.set_mnemonic(&payload)?;
        let stored = self.secrets.get_mnemonic()?;
        if stored.as_deref() != Some(payload.as_str()) {
            let _ = self.secrets.delete_mnemonic();
            return Err(WalletError::SecretStore(
                "secret store did not persist mnemonic".into(),
            ));
        }
        std::fs::create_dir_all(data_dir)?;
        let db_path = meta::db_path(data_dir);
        let mut db =
            Connection::open(&db_path).map_err(|e| WalletError::Persist(e.to_string()))?;
        let params = create_params(secret, network)?;
        let mut wallet = PersistedWallet::create(&mut db, params)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        let _ = wallet.next_unused_address(KeychainKind::External);
        wallet
            .persist(&mut db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
        let mut meta = WalletMeta::new(network, electrum_url);
        meta.mweb_scheme = mweb_scheme;
        meta::write_meta(data_dir, &meta)?;
        let mut state = WalletState {
            last_tip_height: wallet.latest_checkpoint().height(),
            wallet,
            db,
            network,
            electrum_url: meta.electrum_url,
            electrum_validate_domain: meta.electrum_validate_domain,
            electrum_use_public_fallback: meta.electrum_use_public_fallback,
            auto_lock_minutes: meta.auto_lock_minutes,
            active_electrum_url: None,
            litecoin_rpc_url: meta.litecoin_rpc_url,
            mweb_peers: meta.mweb_peers,
            mweb_scheme,
            explorer_base_url: meta.explorer_base_url,
            show_fiat: meta.show_fiat,
            use_explorer_fee_hints: meta.use_explorer_fee_hints,
            data_dir: data_dir.to_path_buf(),
            needs_full_scan: meta.needs_full_scan,
            needs_mweb_scan: meta.needs_mweb_scan,
            mweb: None,
        };
        let summary = build_summary(&mut state)?;
        *self
            .state
            .lock()
            .map_err(|_| WalletError::Persist("wallet state lock poisoned".into()))? = Some(state);
        Ok(summary)
    }
}

/// Estimated vbytes for a typical peg-in (1–2 inputs + HogEx output + change).
const PEGIN_FEE_VB_ESTIMATE: u64 = 250;

fn auto_mweb_fee(explicit: u64) -> u64 {
    if explicit == 0 {
        DEFAULT_MWEB_FEE_SATS
    } else {
        explicit
    }
}

fn auto_pegin_transparent_fee(
    state: &mut WalletState,
    explicit: u64,
) -> Result<u64, WalletError> {
    if explicit > 0 {
        return Ok(explicit);
    }
    let client = connect_electrum(state)?;
    let (rate, _) = electrum::estimate_fee_rate_sat_vb(&client)?;
    Ok(rate.saturating_mul(PEGIN_FEE_VB_ESTIMATE).max(500))
}

fn resolve_pegin_request(
    state: &mut WalletState,
    req: PeginRequest,
) -> Result<PeginRequest, WalletError> {
    let mweb_fee_sats = auto_mweb_fee(req.mweb_fee_sats);
    let transparent_fee_sats = auto_pegin_transparent_fee(state, req.transparent_fee_sats)?;
    let spendable = state.wallet.balance().trusted_spendable().to_sat();
    let amount_sats = if req.drain {
        spendable.checked_sub(transparent_fee_sats).ok_or_else(|| {
            WalletError::BuildTx(format!(
                "not enough transparent funds to peg in after a ~{} litoshis miner fee",
                transparent_fee_sats
            ))
        })?
    } else {
        // The miner fee rides on top of the amount. Catch that here at preview
        // time, before BDK coin selection fails with a raw "BTC" error.
        let needed = req.amount_sats.saturating_add(transparent_fee_sats);
        if needed > spendable {
            return Err(WalletError::BuildTx(format!(
                "swapping {} litoshis needs {} litoshis with the ~{} litoshis miner fee, but only {} \
                 litoshis are spendable — lower the amount or use \"Move all public funds\"",
                req.amount_sats, needed, transparent_fee_sats, spendable
            )));
        }
        req.amount_sats
    };
    if amount_sats <= mweb_fee_sats {
        return Err(WalletError::BuildTx(format!(
            "peg-in amount {} litoshis must exceed the MWEB fee ({} litoshis) so a private coin remains",
            amount_sats, mweb_fee_sats
        )));
    }
    Ok(PeginRequest {
        amount_sats,
        mweb_fee_sats,
        transparent_fee_sats,
        drain: false,
    })
}

fn resolve_mweb_send_request(
    state: &WalletState,
    req: MwebSendRequest,
) -> Result<MwebSendRequest, WalletError> {
    let fee_sats = auto_mweb_fee(req.fee_sats);
    let mweb = state
        .mweb
        .as_ref()
        .ok_or_else(|| WalletError::Mweb("MWEB runtime not initialized".into()))?;
    let tip = state.wallet.latest_checkpoint().height();
    let spendable = mweb::spendable_mweb_sats(mweb, tip);
    let amount_sats = if req.drain {
        spendable.checked_sub(fee_sats).ok_or_else(|| {
            WalletError::BuildTx(format!(
                "not enough private funds to send after a {} litoshis MWEB fee",
                fee_sats
            ))
        })?
    } else {
        req.amount_sats
    };
    if amount_sats == 0 {
        return Err(WalletError::BuildTx(
            "private send amount must be greater than zero".into(),
        ));
    }
    if amount_sats.saturating_add(fee_sats) > spendable {
        return Err(WalletError::BuildTx(format!(
            "need {} litoshis spendable private (amount + fee) but only {} available",
            amount_sats.saturating_add(fee_sats),
            spendable
        )));
    }
    // Validate address early so preview fails before confirm.
    let net = state.network.to_bitcoin_network();
    let _ = Address::from_str(&req.address)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?
        .require_network(net)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;
    Ok(MwebSendRequest {
        address: req.address,
        amount_sats,
        fee_sats,
        drain: false,
    })
}

fn resolve_pegout_request(
    state: &WalletState,
    req: PegoutRequest,
) -> Result<(PegoutRequest, u64), WalletError> {
    let fee_sats = auto_mweb_fee(req.fee_sats);
    let mweb = state
        .mweb
        .as_ref()
        .ok_or_else(|| WalletError::Mweb("MWEB runtime not initialized".into()))?;
    let tip = state.wallet.latest_checkpoint().height();
    let spendable = mweb::spendable_mweb_sats(mweb, tip);
    let net = state.network.to_bitcoin_network();
    let dest = Address::from_str(&req.address)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?
        .require_network(net)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;
    let dust_relay = FeeRate::from_sat_per_vb(30)
        .ok_or_else(|| WalletError::BuildTx("internal dust fee rate".into()))?;
    let dust_sats = dest
        .script_pubkey()
        .minimal_non_dust_custom(dust_relay)
        .to_sat();

    let amount_sats = if req.drain {
        let amount = spendable.checked_sub(fee_sats).ok_or_else(|| {
            WalletError::BuildTx(format!(
                "not enough private funds to peg out after a {} litoshis MWEB fee",
                fee_sats
            ))
        })?;
        if amount < dust_sats {
            return Err(WalletError::BuildTx(format!(
                "peg-out all would create a {} litoshis output below the {} litoshis dust limit",
                amount, dust_sats
            )));
        }
        amount
    } else {
        req.amount_sats
    };
    if amount_sats < dust_sats {
        return Err(WalletError::BuildTx(format!(
            "peg-out amount {} litoshis is below the dust limit ({} litoshis for this address)",
            amount_sats, dust_sats
        )));
    }
    if amount_sats.saturating_add(fee_sats) > spendable {
        return Err(WalletError::BuildTx(format!(
            "need {} litoshis spendable private (amount + fee) but only {} available",
            amount_sats.saturating_add(fee_sats),
            spendable
        )));
    }
    Ok((
        PegoutRequest {
            address: req.address,
            amount_sats,
            fee_sats,
            drain: false,
        },
        dust_sats,
    ))
}

/// Build a send PSBT. Resolves fee rate from the request or Electrum.
/// Returns `(fee_rate_sat_vb, psbt, recipient_script)`.
fn build_send_psbt(
    state: &mut WalletState,
    req: &SendRequest,
) -> Result<(u64, Psbt, ScriptBuf), WalletError> {
    let network = state.network.to_bitcoin_network();
    let address = Address::from_str(&req.address)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?
        .require_network(network)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;

    let fee_rate_sat_vb = match req.fee_rate_sat_vb {
        Some(rate) if rate > 0 => rate,
        _ => {
            let client = connect_electrum(state)?;
            electrum::estimate_fee_rate_sat_vb(&client)?.0
        }
    };
    let fee_rate = FeeRate::from_sat_per_vb(fee_rate_sat_vb).ok_or_else(|| {
        WalletError::BuildTx(format!(
            "fee_rate_sat_vb must be non-zero (got {fee_rate_sat_vb})"
        ))
    })?;

    let dust_relay = FeeRate::from_sat_per_vb(30)
        .ok_or_else(|| WalletError::BuildTx("internal dust fee rate".into()))?;
    let recipient_script = address.script_pubkey();
    let min_non_dust = recipient_script.minimal_non_dust_custom(dust_relay);

    let selected = req
        .selected_outpoints
        .as_ref()
        .map(|v| {
            v.iter()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(parse_outpoint)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .filter(|v| !v.is_empty());

    if selected.is_some() && req.drain {
        return Err(WalletError::BuildTx(
            "manual coin selection cannot be used with Send all — turn off Send all first".into(),
        ));
    }

    if let Some(ref outpoints) = selected {
        for op in outpoints {
            if state.wallet.get_utxo(*op).is_none() {
                return Err(WalletError::BuildTx(format!(
                    "selected outpoint {op} is not an unspent output in this wallet"
                )));
            }
            if state.wallet.is_outpoint_locked(*op) {
                return Err(WalletError::BuildTx(format!(
                    "selected outpoint {op} is frozen — unfreeze it before spending"
                )));
            }
        }
    }

    let mut tx_builder = state.wallet.build_tx();
    if let Some(ref outpoints) = selected {
        tx_builder
            .add_utxos(outpoints)
            .map_err(|e| WalletError::BuildTx(e.to_string()))?;
        tx_builder.manually_selected_only();
    }

    if req.drain {
        tx_builder.drain_wallet();
        tx_builder.drain_to(recipient_script.clone());
    } else {
        let amount = Amount::from_sat(req.amount_sats);
        if amount < min_non_dust {
            return Err(WalletError::BuildTx(format!(
                "amount {} litoshis is below the network dust limit ({} litoshis for this address)",
                req.amount_sats,
                min_non_dust.to_sat()
            )));
        }
        tx_builder.add_recipient(recipient_script.clone(), amount);
    }
    tx_builder.fee_rate(fee_rate);
    let psbt = tx_builder.finish().map_err(|e| {
        let msg = e.to_string();
        if msg.to_lowercase().contains("dust") {
            WalletError::BuildTx(format!(
                "output below dust limit ({} litoshis for this address): {msg}",
                min_non_dust.to_sat()
            ))
        } else {
            WalletError::BuildTx(msg)
        }
    })?;
    Ok((fee_rate_sat_vb.max(MIN_FEE_RATE_SAT_VB), psbt, recipient_script))
}

fn parse_outpoint(raw: &str) -> Result<OutPoint, WalletError> {
    OutPoint::from_str(raw.trim()).map_err(|e| {
        WalletError::BuildTx(format!(
            "invalid outpoint '{raw}' (expected txid:vout): {e}"
        ))
    })
}

fn collect_unspent(state: &WalletState) -> Vec<UtxoRecord> {
    let tip = state.wallet.local_chain().tip().height();
    let mut rows: Vec<UtxoRecord> = state
        .wallet
        .list_unspent()
        .map(|utxo| {
            let confirmations = match utxo.chain_position {
                ChainPosition::Confirmed { anchor, .. } => {
                    tip.saturating_sub(anchor.block_id.height).saturating_add(1)
                }
                ChainPosition::Unconfirmed { .. } => 0,
            };
            let keychain = match utxo.keychain {
                KeychainKind::External => "external",
                KeychainKind::Internal => "internal",
            };
            UtxoRecord {
                outpoint: utxo.outpoint.to_string(),
                txid: utxo.outpoint.txid.to_string(),
                vout: utxo.outpoint.vout,
                amount_sats: utxo.txout.value.to_sat(),
                keychain: keychain.into(),
                confirmations,
                locked: state.wallet.is_outpoint_locked(utxo.outpoint),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.confirmations
            .cmp(&a.confirmations)
            .then_with(|| b.amount_sats.cmp(&a.amount_sats))
            .then_with(|| a.outpoint.cmp(&b.outpoint))
    });
    rows
}

/// Find history peg-ins missing from the transparent graph and apply their spends.
/// Returns a user-facing warning when repair is still needed but no source worked.
fn repair_missing_pegin_spends(
    state: &mut WalletState,
    client: &crate::electrum::ElectrumClient,
) -> Result<Option<String>, WalletError> {
    let Some(mweb) = state.mweb.as_ref() else {
        return Ok(None);
    };
    let missing: Vec<String> = mweb
        .history
        .entries
        .iter()
        .filter(|e| e.kind == TxKind::Pegin)
        .filter_map(|e| {
            let txid = parse_txid(&e.id)?;
            if state.wallet.get_tx(txid).is_some() {
                None
            } else {
                Some(e.id.clone())
            }
        })
        .collect();
    if missing.is_empty() {
        return Ok(None);
    }

    let mut repaired = 0u32;
    let mut still_missing = 0u32;
    let mut last_err: Option<String> = None;
    for id in &missing {
        match fetch_pegin_raw_tx(id, client, state) {
            Ok(bytes) => match deserialize::<bdk_wallet::bitcoin::Transaction>(&bytes) {
                Ok(tx) => {
                    mweb::apply_pegin_transparent_spend(&mut state.wallet, tx);
                    repaired += 1;
                }
                Err(e) => {
                    last_err = Some(format!("decode {id}: {e}"));
                    still_missing += 1;
                }
            },
            Err(e) => {
                last_err = Some(e);
                still_missing += 1;
            }
        }
    }
    if repaired > 0 {
        state
            .wallet
            .persist(&mut state.db)
            .map_err(|e| WalletError::Persist(e.to_string()))?;
    }
    if still_missing > 0 {
        if let Some(err) = &last_err {
            eprintln!("pegin repair: {err}");
        }
        let short = missing
            .first()
            .map(|id| {
                if id.len() > 16 {
                    format!("{}…{}", &id[..8], &id[id.len() - 8..])
                } else {
                    id.clone()
                }
            })
            .unwrap_or_default();
        Ok(Some(format!(
            "Could not repair {still_missing} peg-in spend(s) ({short}). \
             Transparent balance may be overstated. Start litecoind (RPC on {}) \
             or set Litecoin RPC in Settings, then sync again.",
            state.network.default_rpc_url()
        )))
    } else if repaired > 0 {
        Ok(Some(format!(
            "Repaired {repaired} peg-in spend(s) so transparent balance matches Private funds."
        )))
    } else {
        Ok(None)
    }
}

/// Fetch peg-in raw tx bytes. Electrum often omits MWEB peg-ins, so we also try
/// configured RPC, a stock local litecoind, other Electrum servers, and (mainnet)
/// a public explorer hex endpoint.
fn fetch_pegin_raw_tx(
    txid_hex: &str,
    client: &crate::electrum::ElectrumClient,
    state: &WalletState,
) -> Result<Vec<u8>, String> {
    use bdk_electrum::electrum_client::ElectrumApi;

    let mut errors: Vec<String> = Vec::new();
    let txid = parse_txid(txid_hex).ok_or_else(|| format!("invalid peg-in txid {txid_hex}"))?;

    match client.inner.transaction_get_raw(&txid) {
        Ok(raw) if !raw.is_empty() => return Ok(raw),
        Ok(_) => errors.push("sync Electrum returned empty tx".into()),
        Err(e) => errors.push(format!("sync Electrum: {e}")),
    }

    let mut rpc_urls: Vec<String> = Vec::new();
    if let Some(url) = &state.litecoin_rpc_url {
        rpc_urls.push(url.clone());
    }
    let local = state.network.default_rpc_url().to_string();
    if !rpc_urls.iter().any(|u| u == &local) {
        rpc_urls.push(local);
    }
    for url in &rpc_urls {
        match crate::rpc::get_raw_transaction_hex(url, txid_hex) {
            Ok(hex) => match hex::decode(hex.trim()) {
                Ok(raw) if !raw.is_empty() => return Ok(raw),
                Ok(_) => errors.push(format!("RPC {url}: empty hex")),
                Err(e) => errors.push(format!("RPC {url}: bad hex ({e})")),
            },
            Err(e) => errors.push(format!("RPC {url}: {e}")),
        }
    }

    // Other Electrum servers may still hold the raw bytes even when the sync
    // server refuses MWEB txs.
    for url in electrum_candidates(state) {
        if state.active_electrum_url.as_deref() == Some(url.as_str()) {
            continue;
        }
        match electrum::connect_with_timeout(&url, state.electrum_validate_domain, 10) {
            Ok(alt) => match alt.inner.transaction_get_raw(&txid) {
                Ok(raw) if !raw.is_empty() => return Ok(raw),
                Ok(_) => errors.push(format!("{url}: empty tx")),
                Err(e) => errors.push(format!("{url}: {e}")),
            },
            Err(e) => errors.push(format!("{url}: {e}")),
        }
    }

    if state.network == WalletNetwork::Mainnet {
        match fetch_tx_hex_litecoinspace(txid_hex) {
            Ok(raw) => return Ok(raw),
            Err(e) => errors.push(format!("litecoinspace: {e}")),
        }
    }

    Err(errors.join("; "))
}

fn fetch_tx_hex_litecoinspace(txid_hex: &str) -> Result<Vec<u8>, String> {
    let url = format!("https://litecoinspace.org/api/tx/{txid_hex}/hex");
    let body = ureq::get(&url)
        .set("Accept", "text/plain")
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())?;
    let hex = body.trim();
    if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("response was not hex".into());
    }
    hex::decode(hex).map_err(|e| e.to_string())
}

fn parse_txid(hex: &str) -> Option<Txid> {
    Txid::from_str(hex).ok()
}

/// Overlay MWEB activity onto the transparent history.
///
/// A peg-in's transparent tx shows up in the BDK graph after a sync; when its
/// txid matches a log entry we only tag the existing record instead of adding
/// a duplicate. All other entries (peg-outs, MWEB sends/receives) have no
/// transparent record and are appended as standalone records.
fn merge_mweb_history(records: &mut Vec<TxRecord>, mweb: &MwebRuntime, tip: u32) {
    let by_txid: std::collections::HashMap<String, usize> = records
        .iter()
        .enumerate()
        .map(|(i, r)| (r.txid.clone(), i))
        .collect();
    for entry in &mweb.history.entries {
        if let Some(&i) = by_txid.get(&entry.id) {
            records[i].kind = entry.kind;
            // The BDK row only sees the transparent side, so its fee misses the
            // MWEB fee (e.g. a peg-in costs miner + MWEB fee). The recorded
            // entry carries the full amount — prefer it.
            if entry.fee_sats.is_some() {
                records[i].fee_sats = entry.fee_sats;
            }
            continue;
        }
        // Prefer the height resolved and persisted at sync time (covers
        // peg-outs with no change coin, and survives an MWEB resync).
        // Fall back to the coins this tx created for us (received coins,
        // or the change coin of an outgoing tx).
        let mut height: Option<u32> = entry.confirmed_height;
        for id_hex in &entry.output_ids {
            let Ok(bytes) = hex::decode(id_hex) else { continue };
            let Ok(id) = <[u8; 32]>::try_from(bytes.as_slice()) else {
                continue;
            };
            let coin = mweb
                .store
                .db()
                .get(&id)
                .or_else(|| mweb.store.db().get_spent(&id));
            if let Some(h) = coin.and_then(|c| c.block_height) {
                height = Some(height.map_or(h, |cur| cur.min(h)));
            }
        }
        let confirmations = height
            .map(|h| tip.saturating_sub(h).saturating_add(1))
            .unwrap_or(0);
        records.push(TxRecord {
            txid: entry.id.clone(),
            net_sats: entry.net_sats,
            sent_sats: (-entry.net_sats).max(0) as u64,
            received_sats: entry.net_sats.max(0) as u64,
            fee_sats: entry.fee_sats,
            height,
            confirmations,
            timestamp: Some(entry.timestamp),
            kind: entry.kind,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mweb_history::MwebHistoryEntry;

    #[test]
    fn merge_uses_persisted_confirmed_height() {
        let dir = tempfile::tempdir().unwrap();
        let secret =
            MasterSecret::parse(&descriptors::generate_mnemonic().unwrap(), None).unwrap();
        let mut mweb = MwebRuntime::open(
            dir.path(),
            &secret,
            WalletNetwork::Testnet,
            MwebScheme::default().to_master_scheme(),
            None,
        )
        .unwrap();
        mweb.history.record(MwebHistoryEntry {
            id: "wtxid1".into(),
            kind: TxKind::Pegout,
            net_sats: -5_000,
            fee_sats: Some(100),
            timestamp: 1_700_000_000,
            output_ids: Vec::new(),
            input_ids: Vec::new(),
            confirmed_height: Some(90),
        });
        mweb.history.record(MwebHistoryEntry {
            id: "wtxid2".into(),
            kind: TxKind::Pegout,
            net_sats: -5_000,
            fee_sats: Some(100),
            timestamp: 1_700_000_000,
            output_ids: Vec::new(),
            input_ids: Vec::new(),
            confirmed_height: None,
        });
        let mut records = Vec::new();
        merge_mweb_history(&mut records, &mweb, 100);
        let confirmed = records.iter().find(|r| r.txid == "wtxid1").unwrap();
        assert_eq!(confirmed.height, Some(90));
        assert_eq!(confirmed.confirmations, 11);
        let pending = records.iter().find(|r| r.txid == "wtxid2").unwrap();
        assert_eq!(pending.height, None);
        assert_eq!(pending.confirmations, 0);
    }
}

/// Revealed external/internal addresses for local `is_wallet` tagging.
fn revealed_address_set(wallet: &PersistedWallet<Connection>) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    for keychain in [KeychainKind::External, KeychainKind::Internal] {
        if let Some(last) = wallet.derivation_index(keychain) {
            for i in 0..=last {
                set.insert(wallet.peek_address(keychain, i).address.to_string());
            }
        }
    }
    set
}

fn is_mweb_stealth_address(address: &str) -> bool {
    let lower = address.trim().to_ascii_lowercase();
    lower.starts_with("ltcmweb") || lower.starts_with("tmweb")
}

/// Used = revealed and not in `list_unused_addresses` for that keychain.
fn used_revealed_addresses(
    wallet: &PersistedWallet<Connection>,
) -> std::collections::HashSet<String> {
    let mut used = std::collections::HashSet::new();
    for keychain in [KeychainKind::External, KeychainKind::Internal] {
        let Some(last) = wallet.derivation_index(keychain) else {
            continue;
        };
        let unused: std::collections::HashSet<String> = wallet
            .list_unused_addresses(keychain)
            .map(|info| info.address.to_string())
            .collect();
        for i in 0..=last {
            let addr = wallet.peek_address(keychain, i).address.to_string();
            if !unused.contains(&addr) {
                used.insert(addr);
            }
        }
    }
    used
}

fn compute_address_reuse_hint(
    wallet: &PersistedWallet<Connection>,
    address: &str,
) -> AddressReuseHint {
    let address = address.trim();
    if address.is_empty() || is_mweb_stealth_address(address) {
        return AddressReuseHint { reused: false };
    }
    AddressReuseHint {
        reused: used_revealed_addresses(wallet).contains(address),
    }
}

fn build_summary(state: &mut WalletState) -> Result<WalletSummary, WalletError> {
    let balance = state.wallet.balance();
    let tip_height = state.wallet.local_chain().tip().height();
    // Prefer the last revealed address while it is still unused so a "New
    // address" click sticks across sync/reload. Once it has been used, reveal
    // the next index instead of falling back to an earlier unused gap address.
    let receive_address = current_receive_address(&mut state.wallet);
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

fn current_receive_address(wallet: &mut PersistedWallet<Connection>) -> String {
    if let Some(last_idx) = wallet.derivation_index(KeychainKind::External) {
        let last_still_unused = wallet
            .list_unused_addresses(KeychainKind::External)
            .any(|info| info.index == last_idx);
        if last_still_unused {
            return wallet
                .peek_address(KeychainKind::External, last_idx)
                .to_string();
        }
    }
    wallet
        .reveal_next_address(KeychainKind::External)
        .to_string()
}
