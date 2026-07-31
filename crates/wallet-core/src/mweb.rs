//! MWEB orchestration beside the transparent wallet.

use std::fs;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::str::FromStr;

use std::sync::Arc;

use bdk_mweb::keys::{MasterKeyScheme, MasterKeys};
use bdk_mweb::lip0006_tcp::{BroadcastAck, TcpMwebPeer};
use bdk_mweb::mweb_sync::{
    FixedHeaderProvider, MwebSyncer, PeerPool, ReadyNotifier, SyncProgress, SyncState,
};
use bdk_mweb::tx_builder::CHANGE_ADDRESS_INDEX;
use bdk_mweb::{AddressBook, DEFAULT_GAP_LIMIT, MWEB_PEGIN_MATURITY};
use bdk_wallet::bitcoin::consensus::encode::serialize_hex;
use bdk_wallet::bitcoin::key::Secp256k1;
use bdk_wallet::bitcoin::{Address, Amount, NetworkKind};
use bdk_wallet::keys::bip39::Mnemonic;
use bdk_wallet::rusqlite::Connection;
use bdk_wallet::{
    extract_prepared_mweb_pegin, MwebStore, PersistedWallet, SignOptions,
};

use crate::dto::{
    CombinedSummary, MwebBroadcastResult, MwebSendRequest, PeginRequest, PeginResult, PegoutRequest,
    TxKind, WalletSummary,
};
use crate::error::WalletError;
use crate::meta;
use crate::mweb_history::{now_ts, MwebHistory, MwebHistoryEntry};
use crate::network::WalletNetwork;
use crate::rpc;

pub struct MwebRuntime {
    pub store: MwebStore,
    pub keys: MasterKeys,
    pub book: AddressBook,
    pub sync_state: SyncState,
    pub receive_index: u32,
    pub last_synced_height: Option<u32>,
    pub last_status: String,
    pub stale: bool,
    pub history: MwebHistory,
    secp: Secp256k1<bdk_wallet::bitcoin::secp256k1::All>,
}

impl MwebRuntime {
    pub fn open(data_dir: &Path, mnemonic: &Mnemonic, network: WalletNetwork) -> Result<Self, WalletError> {
        let secp = Secp256k1::new();
        let bdk_net = network.to_bitcoin_network();
        let seed = mnemonic.to_seed("");
        let keys = MasterKeys::from_seed(&seed, bdk_net, MasterKeyScheme::LitecoinCore, &secp)
            .map_err(|e| WalletError::Mweb(e.to_string()))?;
        let book = AddressBook::from_keys(&keys, DEFAULT_GAP_LIMIT, &secp)
            .map_err(|e| WalletError::Mweb(e.to_string()))?;

        let store = load_store(data_dir)?;
        let sync_state = load_sync_state(data_dir)?;
        let receive_index = load_receive_index(data_dir)?;
        let history = MwebHistory::load(&meta::mweb_history_path(data_dir))?;

        Ok(Self {
            store,
            keys,
            book,
            sync_state,
            receive_index,
            last_synced_height: None,
            last_status: "MWEB not synced yet".into(),
            stale: true,
            history,
            secp,
        })
    }

    pub fn persist(&mut self, data_dir: &Path) -> Result<(), WalletError> {
        persist_store(data_dir, &mut self.store)?;
        save_sync_state(data_dir, &self.sync_state)?;
        save_receive_index(data_dir, self.receive_index)?;
        self.history.save(&meta::mweb_history_path(data_dir))?;
        Ok(())
    }

    pub fn receive_address(&mut self, network: WalletNetwork) -> Result<String, WalletError> {
        let kind = match network {
            WalletNetwork::Mainnet => NetworkKind::Main,
            WalletNetwork::Testnet => NetworkKind::Test,
        };
        let addr = self
            .keys
            .address(self.receive_index, kind, &self.secp)
            .map_err(|e| WalletError::Mweb(e.to_string()))?;
        Ok(addr.to_string())
    }

    pub fn advance_receive_index(&mut self) {
        self.receive_index = self.receive_index.saturating_add(1);
    }

    /// Tip-only LIP sync. On peer failure, mark stale and keep transparent usable.
    ///
    /// `prev_tip_hash` is the block hash on the *current* chain at the height of the
    /// previous MWEB sync. Providing it lets the syncer prove the previous tip is still
    /// on-chain (pure extension), keeping the leafset snapshot so only new leaves are
    /// downloaded instead of the full UTXO set on every block.
    pub fn sync_at_tip(
        &mut self,
        tip_height: u32,
        tip_hash: bdk_wallet::bitcoin::BlockHash,
        prev_tip_hash: Option<bdk_wallet::bitcoin::BlockHash>,
        peers: &[String],
        network: WalletNetwork,
        progress: Option<Arc<SyncProgress>>,
    ) -> Result<(), WalletError> {
        if peers.is_empty() {
            self.stale = true;
            self.last_status = "no MWEB peers configured".into();
            return Ok(());
        }
        let addrs = resolve_peers(peers)?;
        if addrs.is_empty() {
            self.stale = true;
            self.last_status = format!("could not resolve MWEB peers: {peers:?}");
            return Ok(());
        }
        let mut pool = PeerPool::new(addrs);
        let syncer = MwebSyncer {
            progress,
            ..MwebSyncer::tip_only()
        };
        let bdk_net = network.to_bitcoin_network();
        let mut hashes = std::collections::BTreeMap::new();
        if let (Some(prev_height), Some(prev_hash)) = (self.sync_state.tip_height, prev_tip_hash) {
            hashes.insert(prev_height, prev_hash);
        }
        hashes.insert(tip_height, tip_hash);
        let mut headers = FixedHeaderProvider {
            tip_hash,
            tip_height,
            hashes,
        };
        headers.set_tip(tip_hash, tip_height);
        let mut notifier = ReadyNotifier { tip_height };
        match pool.with_failover(bdk_net, |peer| {
            self.store.sync_differential(
                &syncer,
                &headers,
                &mut notifier,
                peer,
                &mut self.sync_state,
                &self.keys,
                &self.book,
                &self.secp,
            )
        }) {
            Ok(result) => {
                self.last_synced_height = Some(tip_height);
                self.stale = false;
                self.last_status = format!(
                    "MWEB synced at {tip_height} (+{} downloaded, {} found)",
                    result.downloaded,
                    result.found.len()
                );
                self.absorb_received_coins();
                Ok(())
            }
            Err(e) => {
                self.stale = true;
                self.last_status = format!("MWEB peer unreachable: {e}");
                Ok(())
            }
        }
    }

    pub fn disconnect_from(&mut self, height: u32) {
        self.store.disconnect_from(height);
    }

    /// Add history entries for coins in the store that no entry accounts for
    /// yet (external MWEB receives, or peg-ins found on restore). Coins created
    /// by our own peg-ins / sends are pre-registered and skipped.
    fn absorb_received_coins(&mut self) {
        for coin in self.store.db().unspent_vec() {
            let id_hex = hex::encode(coin.output_id);
            if self.history.is_known(&id_hex) {
                continue;
            }
            self.history.record(MwebHistoryEntry {
                id: id_hex.clone(),
                kind: if coin.is_pegin {
                    TxKind::Pegin
                } else {
                    TxKind::MwebReceive
                },
                net_sats: coin.amount as i64,
                fee_sats: None,
                timestamp: now_ts(),
                output_ids: vec![id_hex],
            });
        }
    }

    pub fn combined_summary(
        &mut self,
        wallet: &PersistedWallet<Connection>,
        transparent: WalletSummary,
    ) -> Result<CombinedSummary, WalletError> {
        let bal = wallet.balance_combined_store(&self.store);
        let tip = wallet.latest_checkpoint().height();
        let immature = self
            .store
            .db()
            .unspent_vec()
            .into_iter()
            .filter(|c| {
                c.is_pegin
                    && c.block_height
                        .map(|h| tip.saturating_sub(h) < MWEB_PEGIN_MATURITY)
                        .unwrap_or(true)
            })
            .map(|c| c.amount)
            .sum::<u64>();
        let mweb_addr = self.receive_address(transparent.network).ok();
        Ok(CombinedSummary {
            transparent,
            mweb_confirmed_sats: bal.mweb_confirmed.to_sat().saturating_sub(immature),
            mweb_unconfirmed_sats: bal.mweb_untrusted_pending.to_sat(),
            mweb_immature_sats: immature,
            mweb_total_sats: bal.mweb_total().to_sat(),
            mweb_receive_address: mweb_addr,
            mweb_synced_height: self.last_synced_height,
            mweb_stale: self.stale,
            mweb_status: self.last_status.clone(),
        })
    }
}

/// Remove MWEB store/sync/index files for a from-scratch resync.
///
/// Deliberately keeps the history log: entries stay visible across a resync,
/// and `known_outputs` stops re-found coins from duplicating receive entries.
pub fn wipe_mweb_files(data_dir: &Path) -> Result<(), WalletError> {
    for path in [
        meta::mweb_db_path(data_dir),
        meta::mweb_sync_path(data_dir),
        meta::mweb_index_path(data_dir),
    ] {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(WalletError::Io(e)),
        }
    }
    Ok(())
}

fn load_store(data_dir: &Path) -> Result<MwebStore, WalletError> {
    let path = meta::mweb_db_path(data_dir);
    let mut conn = Connection::open(&path).map_err(|e| WalletError::Mweb(e.to_string()))?;
    let tx = conn
        .transaction()
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
    let store = MwebStore::load_sqlite(&tx).map_err(|e| WalletError::Mweb(e.to_string()))?;
    tx.commit().map_err(|e| WalletError::Mweb(e.to_string()))?;
    Ok(store)
}

fn persist_store(data_dir: &Path, store: &mut MwebStore) -> Result<(), WalletError> {
    let path = meta::mweb_db_path(data_dir);
    let mut conn = Connection::open(&path).map_err(|e| WalletError::Mweb(e.to_string()))?;
    let tx = conn
        .transaction()
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
    store
        .persist_sqlite(&tx)
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
    tx.commit().map_err(|e| WalletError::Mweb(e.to_string()))?;
    Ok(())
}

fn load_sync_state(data_dir: &Path) -> Result<SyncState, WalletError> {
    let path = meta::mweb_sync_path(data_dir);
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| WalletError::Mweb(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SyncState::default()),
        Err(e) => Err(WalletError::Io(e)),
    }
}

fn save_sync_state(data_dir: &Path, state: &SyncState) -> Result<(), WalletError> {
    let json = serde_json::to_string_pretty(state).map_err(|e| WalletError::Mweb(e.to_string()))?;
    fs::write(meta::mweb_sync_path(data_dir), json)?;
    Ok(())
}

fn load_receive_index(data_dir: &Path) -> Result<u32, WalletError> {
    let path = meta::mweb_index_path(data_dir);
    match fs::read_to_string(&path) {
        Ok(s) => Ok(s.trim().parse().unwrap_or(0)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(WalletError::Io(e)),
    }
}

fn save_receive_index(data_dir: &Path, index: u32) -> Result<(), WalletError> {
    fs::write(meta::mweb_index_path(data_dir), format!("{index}\n"))?;
    Ok(())
}

fn resolve_peers(peers: &[String]) -> Result<Vec<std::net::SocketAddr>, WalletError> {
    let mut out = Vec::new();
    for p in peers {
        match p.to_socket_addrs() {
            Ok(iter) => out.extend(iter),
            Err(e) => {
                // Keep going; sync_at_tip will report if none resolve.
                let _ = e;
            }
        }
    }
    Ok(out)
}

pub fn pegin(
    wallet: &mut PersistedWallet<Connection>,
    runtime: &mut MwebRuntime,
    rpc_url: Option<&str>,
    peers: &[String],
    req: PeginRequest,
    network: WalletNetwork,
) -> Result<PeginResult, WalletError> {
    let mut prepared = wallet
        .prepare_mweb_pegin(
            &runtime.keys,
            runtime.receive_index,
            Amount::from_sat(req.amount_sats),
            Amount::from_sat(req.mweb_fee_sats),
            Amount::from_sat(req.transparent_fee_sats),
            &runtime.secp,
        )
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
    let signed = wallet
        .sign(&mut prepared.psbt, SignOptions::default())
        .map_err(|e| WalletError::Sign(e.to_string()))?;
    if !signed {
        return Err(WalletError::Sign("peg-in not fully signed".into()));
    }
    let tx =
        extract_prepared_mweb_pegin(&prepared.psbt).map_err(|e| WalletError::Mweb(e.to_string()))?;
    // A peg-in tx carries MWEB extension data that Electrum servers reject
    // ("TX decode failed"), so it must take the RPC/P2P path like other MWEB txs.
    broadcast_mweb_tx(&tx, rpc_url, peers, network)?;
    let tip = wallet.latest_checkpoint().height();
    let mut output_ids = Vec::new();
    for mut coin in prepared.outputs {
        coin.is_pegin = true;
        coin.block_height = Some(tip);
        output_ids.push(hex::encode(coin.output_id));
        runtime.store.db_mut().insert(coin);
    }
    runtime.advance_receive_index();
    let txid = tx.compute_txid().to_string();
    let fee_sats = req.transparent_fee_sats.saturating_add(req.mweb_fee_sats);
    // Net matches what BDK will compute for the transparent tx once it syncs:
    // the peg-in output (amount + MWEB fee) plus the transparent fee leave the
    // transparent wallet. The entry is deduped against that record by txid.
    runtime.history.record(MwebHistoryEntry {
        id: txid.clone(),
        kind: TxKind::Pegin,
        net_sats: -((req.amount_sats.saturating_add(fee_sats)) as i64),
        fee_sats: Some(fee_sats),
        timestamp: now_ts(),
        output_ids,
    });
    Ok(PeginResult {
        txid,
        fee_sats,
        maturity_blocks: MWEB_PEGIN_MATURITY,
    })
}

/// Broadcast a transaction that carries MWEB data (peg-in, peg-out, MWEB send).
/// Electrum servers cannot relay these, so they never take the Electrum path.
///
/// Prefers litecoind RPC when configured (authoritative error messages), and
/// falls back to sending the tx over the LIP-0006 P2P connection otherwise.
fn broadcast_mweb_tx(
    tx: &bdk_wallet::bitcoin::Transaction,
    rpc_url: Option<&str>,
    peers: &[String],
    network: WalletNetwork,
) -> Result<(), WalletError> {
    if let Some(url) = rpc_url {
        let hex = serialize_hex(tx);
        let _ = rpc::send_raw_transaction(url, &hex)?;
        return Ok(());
    }
    let addrs = resolve_peers(peers)?;
    if addrs.is_empty() {
        return Err(WalletError::Mweb(
            "cannot broadcast MWEB transaction: configure a Litecoin RPC URL or reachable MWEB P2P peers in Settings".into(),
        ));
    }
    let bdk_net = network.to_bitcoin_network();
    let mut last_err = String::from("no MWEB peers reachable");
    for addr in addrs {
        match TcpMwebPeer::connect(addr, bdk_net) {
            Ok(mut peer) => match peer.broadcast_tx(tx) {
                Ok(BroadcastAck::Confirmed) => {
                    eprintln!("MWEB broadcast: {addr} accepted the transaction");
                    return Ok(());
                }
                Ok(BroadcastAck::Sent) => {
                    eprintln!(
                        "MWEB broadcast: sent to {addr}; acceptance not confirmed before deadline"
                    );
                    return Ok(());
                }
                Err(e) => {
                    last_err = format!(
                        "{addr}: {}",
                        crate::error::humanize_broadcast_error(&e.to_string())
                    )
                }
            },
            Err(e) => last_err = format!("{addr}: {e}"),
        }
    }
    Err(WalletError::Mweb(format!("P2P broadcast failed: {last_err}")))
}

pub fn mweb_send(
    wallet: &PersistedWallet<Connection>,
    runtime: &mut MwebRuntime,
    rpc_url: Option<&str>,
    peers: &[String],
    req: MwebSendRequest,
    network: WalletNetwork,
) -> Result<MwebBroadcastResult, WalletError> {
    let net = network.to_bitcoin_network();
    let dest = Address::from_str(&req.address)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?
        .require_network(net)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;
    let mut funded = wallet
        .fund_mweb_send(
            runtime.store.db(),
            &runtime.keys,
            dest,
            Amount::from_sat(req.amount_sats),
            Amount::from_sat(req.fee_sats),
            CHANGE_ADDRESS_INDEX,
            &runtime.secp,
        )
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
    let spent_ids: Vec<_> = funded.spent_coins.iter().map(|c| c.output_id).collect();
    let (tx, change) = wallet
        .sign_and_extract_funded_mweb(&mut funded, &runtime.keys, &runtime.secp)
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
    broadcast_mweb_tx(&tx, rpc_url, peers, network)?;
    let wtxid = tx.compute_wtxid().to_string();
    for id in &spent_ids {
        let _ = runtime.store.db_mut().mark_spent(id);
    }
    let mut output_ids = Vec::new();
    if let Some(mut change) = change {
        change.block_height = None;
        output_ids.push(hex::encode(change.output_id));
        runtime.store.db_mut().insert(change);
    }
    runtime.history.record(MwebHistoryEntry {
        id: wtxid.clone(),
        kind: TxKind::MwebSend,
        net_sats: -((req.amount_sats.saturating_add(req.fee_sats)) as i64),
        fee_sats: Some(req.fee_sats),
        timestamp: now_ts(),
        output_ids,
    });
    Ok(MwebBroadcastResult {
        wtxid,
        fee_sats: req.fee_sats,
    })
}

pub fn pegout(
    wallet: &PersistedWallet<Connection>,
    runtime: &mut MwebRuntime,
    rpc_url: Option<&str>,
    peers: &[String],
    req: PegoutRequest,
    network: WalletNetwork,
) -> Result<MwebBroadcastResult, WalletError> {
    let net = network.to_bitcoin_network();
    let dest = Address::from_str(&req.address)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?
        .require_network(net)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;

    let dust_relay = bdk_wallet::bitcoin::FeeRate::from_sat_per_vb(30)
        .ok_or_else(|| WalletError::BuildTx("internal dust fee rate".into()))?;
    let min_non_dust = dest.script_pubkey().minimal_non_dust_custom(dust_relay);
    if Amount::from_sat(req.amount_sats) < min_non_dust {
        return Err(WalletError::BuildTx(format!(
            "peg-out amount {} litoshis is below dust limit ({})",
            req.amount_sats,
            min_non_dust.to_sat()
        )));
    }

    let mut funded = wallet
        .fund_mweb_pegout(
            runtime.store.db(),
            &runtime.keys,
            dest.script_pubkey(),
            Amount::from_sat(req.amount_sats),
            Amount::from_sat(req.fee_sats),
            CHANGE_ADDRESS_INDEX,
            &runtime.secp,
        )
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
    let spent_ids: Vec<_> = funded.spent_coins.iter().map(|c| c.output_id).collect();
    let (tx, change) = wallet
        .sign_and_extract_funded_mweb(&mut funded, &runtime.keys, &runtime.secp)
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
    broadcast_mweb_tx(&tx, rpc_url, peers, network)?;
    let wtxid = tx.compute_wtxid().to_string();
    for id in &spent_ids {
        let _ = runtime.store.db_mut().mark_spent(id);
    }
    let mut output_ids = Vec::new();
    if let Some(mut change) = change {
        change.block_height = None;
        output_ids.push(hex::encode(change.output_id));
        runtime.store.db_mut().insert(change);
    }
    runtime.history.record(MwebHistoryEntry {
        id: wtxid.clone(),
        kind: TxKind::Pegout,
        net_sats: -((req.amount_sats.saturating_add(req.fee_sats)) as i64),
        fee_sats: Some(req.fee_sats),
        timestamp: now_ts(),
        output_ids,
    });
    Ok(MwebBroadcastResult {
        wtxid,
        fee_sats: req.fee_sats,
    })
}
