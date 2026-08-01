//! MWEB orchestration beside the transparent wallet.

use std::fs;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::str::FromStr;

use std::sync::Arc;

use bdk_mweb::keys::{MasterKeyScheme, MasterKeys};
use bdk_mweb::lip0006::MwebUtxoSource;
use bdk_mweb::lip0006_tcp::{BroadcastAck, TcpMwebPeer};
use bdk_mweb::mweb_sync::{
    leafset_has_leaf, FixedHeaderProvider, MwebSyncer, PeerPool, ReadyNotifier, SyncProgress,
    SyncState,
};
use bdk_mweb::p2p::MwebLeafset;
use bdk_mweb::pmmr::verify_leafset;
use bdk_mweb::MwebCoinDatabase;
use bdk_mweb::tx_builder::CHANGE_ADDRESS_INDEX;
use bdk_mweb::{AddressBook, ChangeSet, MWEB_PEGIN_MATURITY};
use bdk_wallet::chain::Merge;
use bdk_wallet::bitcoin::consensus::encode::serialize_hex;
use bdk_wallet::bitcoin::key::Secp256k1;
use bdk_wallet::bitcoin::{Address, Amount, NetworkKind};
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

/// MWEB address-book size. Ownership lookup during scanning is a single map
/// probe regardless of book size (see `bdk_mweb::scan`), so this is sized for
/// recovery: coins received at any index below it are found on restore. The
/// old `DEFAULT_GAP_LIMIT` of 20 silently missed coins at higher indices.
pub const MWEB_GAP_LIMIT: u32 = 1000;

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
    /// Warning from the last sync's second-peer leafset cross-check, if any.
    pub cross_check_warning: Option<String>,
    /// Full aggregate coin changeset mirrored on disk (sealed persistence).
    agg: ChangeSet,
    /// Key for encrypting MWEB files at rest; `None` falls back to the legacy
    /// plaintext formats (tests / plaintext-era wallets awaiting migration).
    sealing_key: Option<[u8; 32]>,
    secp: Secp256k1<bdk_wallet::bitcoin::secp256k1::All>,
}

impl MwebRuntime {
    pub fn open(
        data_dir: &Path,
        secret: &crate::seed::MasterSecret,
        network: WalletNetwork,
        scheme: MasterKeyScheme,
        sealing_key: Option<[u8; 32]>,
    ) -> Result<Self, WalletError> {
        let secp = Secp256k1::new();
        let bdk_net = network.to_bitcoin_network();
        let master = secret.master_xprv(bdk_net)?;
        let keys = MasterKeys::from_xprv(&master, scheme, &secp)
            .map_err(|e| WalletError::Mweb(e.to_string()))?;
        let book = AddressBook::from_keys(&keys, MWEB_GAP_LIMIT, &secp)
            .map_err(|e| WalletError::Mweb(e.to_string()))?;

        let (store, agg) = load_store(data_dir, sealing_key.as_ref())?;
        let sync_state = load_sync_state(data_dir, sealing_key.as_ref())?;
        let receive_index = load_receive_index(data_dir, sealing_key.as_ref())?;
        let history = load_history(data_dir, sealing_key.as_ref())?;

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
            cross_check_warning: None,
            agg,
            sealing_key,
            secp,
        })
    }

    pub fn persist(&mut self, data_dir: &Path) -> Result<(), WalletError> {
        if let Some(key) = self.sealing_key {
            self.agg.merge(self.store.take_staged());
            let coins = bdk_mweb::seal_changeset(&key, &self.agg)
                .map_err(|e| WalletError::Mweb(e.to_string()))?;
            write_sealed(&meta::mweb_coins_enc_path(data_dir), coins)?;
            let sync_json = serde_json::to_vec(&self.sync_state)
                .map_err(|e| WalletError::Mweb(e.to_string()))?;
            write_sealed(&meta::mweb_sync_enc_path(data_dir), seal_bytes(&key, &sync_json)?)?;
            write_sealed(
                &meta::mweb_index_enc_path(data_dir),
                seal_bytes(&key, self.receive_index.to_string().as_bytes())?,
            )?;
            let history_json = serde_json::to_vec(&self.history)
                .map_err(|e| WalletError::Mweb(e.to_string()))?;
            write_sealed(
                &meta::mweb_history_enc_path(data_dir),
                seal_bytes(&key, &history_json)?,
            )?;
            // Everything is sealed on disk now; drop plaintext-era leftovers.
            remove_legacy_plaintext_files(data_dir);
        } else {
            persist_store(data_dir, &mut self.store)?;
            save_sync_state(data_dir, &self.sync_state)?;
            save_receive_index(data_dir, self.receive_index)?;
            self.history.save(&meta::mweb_history_path(data_dir))?;
        }
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
        let configured = resolve_peers(peers)?;
        let mut configured_err = None;
        if !configured.is_empty() {
            match self.sync_pass(
                configured.clone(),
                tip_height,
                tip_hash,
                prev_tip_hash,
                network,
                progress.clone(),
            ) {
                Ok(()) => {
                    let mut candidates = configured;
                    if candidates.len() < 2 {
                        candidates.extend(crate::discovery::discover_mweb_peers(network));
                    }
                    self.cross_check_leafset(tip_hash, candidates, network);
                    return Ok(());
                }
                Err(e) => configured_err = Some(e),
            }
        }
        // Their own node is absent or down, so fall back to public MWEB peers
        // from the DNS seeds rather than leaving MWEB dark.
        let discovered = crate::discovery::discover_mweb_peers(network);
        if discovered.is_empty() {
            self.stale = true;
            self.last_status = match configured_err {
                Some(e) => format!("MWEB peer unreachable: {e}"),
                None => "no MWEB peers configured, and none found via DNS seeds".into(),
            };
            return Ok(());
        }
        match self.sync_pass(
            discovered.clone(),
            tip_height,
            tip_hash,
            prev_tip_hash,
            network,
            progress,
        ) {
            Ok(()) => {
                self.last_status.push_str(" via public peer");
                self.cross_check_leafset(tip_hash, discovered, network);
                Ok(())
            }
            Err(e) => {
                self.stale = true;
                self.last_status = format!("MWEB peer unreachable: {e}");
                Ok(())
            }
        }
    }

    /// Ask up to two peers for the MWEB header at `tip_hash` and verify our
    /// freshly synced leafset against each header's `leafset_root`.
    ///
    /// A single LIP-0006 peer can understate the balance by serving an
    /// internally consistent but stale/forged header + leafset; agreement from
    /// a second peer means omission now requires collusion. Only headers are
    /// requested, so the peers learn nothing about the wallet.
    fn cross_check_leafset(
        &mut self,
        tip_hash: bdk_wallet::bitcoin::BlockHash,
        addrs: Vec<std::net::SocketAddr>,
        network: WalletNetwork,
    ) {
        self.cross_check_warning = None;
        if self.sync_state.leafset.is_empty() {
            return;
        }
        let mut distinct: Vec<std::net::SocketAddr> = Vec::new();
        for addr in addrs {
            if !distinct.contains(&addr) {
                distinct.push(addr);
            }
        }
        let leafset = MwebLeafset {
            block_hash: tip_hash,
            leafset: self.sync_state.leafset.clone(),
        };
        let bdk_net = network.to_bitcoin_network();
        let mut confirmed = 0u32;
        for addr in distinct {
            if confirmed >= 2 {
                break;
            }
            let Ok(mut peer) = TcpMwebPeer::connect(addr, bdk_net) else {
                continue;
            };
            let Ok(hdr) = peer.get_header(tip_hash) else {
                continue;
            };
            match verify_leafset(
                &leafset,
                &hdr.mweb_header.leafset_root,
                hdr.mweb_header.output_mmr_size,
            ) {
                Ok(()) => confirmed += 1,
                Err(_) => {
                    self.cross_check_warning = Some(format!(
                        "WARNING: MWEB peer {addr} reports a different UTXO set than the peer \
                         used for this sync — your private balance may be incomplete. Run \
                         'Resync MWEB' or verify against your own node"
                    ));
                    return;
                }
            }
        }
        match confirmed {
            0 => self
                .last_status
                .push_str(" · cross-check unavailable (no second peer reachable)"),
            1 => self.last_status.push_str(" · leafset confirmed by 1 peer"),
            n => self
                .last_status
                .push_str(&format!(" · leafset confirmed by {n} peers")),
        }
    }

    /// One differential sync attempt against a fixed set of peers.
    #[allow(clippy::too_many_arguments)]
    fn sync_pass(
        &mut self,
        addrs: Vec<std::net::SocketAddr>,
        tip_height: u32,
        tip_hash: bdk_wallet::bitcoin::BlockHash,
        prev_tip_hash: Option<bdk_wallet::bitcoin::BlockHash>,
        network: WalletNetwork,
        progress: Option<Arc<SyncProgress>>,
    ) -> Result<(), String> {
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
                update_outgoing_confirmations(
                    &mut self.history,
                    self.store.db(),
                    &self.sync_state.leafset,
                    tip_height,
                );
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn disconnect_from(&mut self, height: u32) {
        self.store.disconnect_from(height);
        // Reorged-out entries must be re-resolved on the new chain.
        for entry in &mut self.history.entries {
            if entry.confirmed_height.is_some_and(|h| h >= height) {
                entry.confirmed_height = None;
            }
        }
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
                input_ids: Vec::new(),
                confirmed_height: None,
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

/// Resolve `confirmed_height` for outgoing MWEB entries (peg-outs and MWEB
/// sends) after a successful sync at `tip`.
///
/// Signals, in order of preference:
/// 1. A coin the tx created for us (change) carries a `block_height` — exact.
/// 2. Every spent input's leaf is gone from the network leafset (spends only
///    clear at inclusion), or the coin vanished from the store entirely after
///    a wipe/resync — confirmed at or before `tip`, recorded as `tip`.
/// 3. Legacy entries with nothing to track (recorded before input tracking
///    existed, no change): resolved at `tip` rather than pending forever; the
///    inputs already left the balance at broadcast.
pub(crate) fn update_outgoing_confirmations(
    history: &mut MwebHistory,
    db: &MwebCoinDatabase,
    leafset: &[u8],
    tip: u32,
) {
    for entry in &mut history.entries {
        if entry.confirmed_height.is_some()
            || !matches!(entry.kind, TxKind::Pegout | TxKind::MwebSend)
        {
            continue;
        }
        let mut height: Option<u32> = None;
        for id_hex in &entry.output_ids {
            let Some(id) = decode_output_id(id_hex) else {
                continue;
            };
            let coin = db.get(&id).or_else(|| db.get_spent(&id));
            if let Some(h) = coin.and_then(|c| c.block_height) {
                height = Some(height.map_or(h, |cur| cur.min(h)));
            }
        }
        if height.is_some() {
            entry.confirmed_height = height;
            continue;
        }
        if entry.input_ids.is_empty() {
            if entry.output_ids.is_empty() {
                entry.confirmed_height = Some(tip);
            }
            continue;
        }
        if leafset.is_empty() {
            continue;
        }
        let all_inputs_gone = entry.input_ids.iter().all(|id_hex| {
            let Some(id) = decode_output_id(id_hex) else {
                return false;
            };
            match db.get(&id).or_else(|| db.get_spent(&id)) {
                // Absent after a store wipe: a confirmed-spent coin is never
                // re-found by sync, while an unconfirmed spend's coin would be.
                None => true,
                Some(coin) => coin
                    .leaf_index
                    .is_some_and(|leaf| !leafset_has_leaf(leafset, leaf)),
            }
        });
        if all_inputs_gone {
            entry.confirmed_height = Some(tip);
        }
    }
}

fn decode_output_id(id_hex: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(id_hex).ok()?;
    <[u8; 32]>::try_from(bytes.as_slice()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bdk_mweb::MwebCoin;

    const TIP: u32 = 200;

    fn coin(id: u8, height: Option<u32>, leaf_index: Option<u64>) -> MwebCoin {
        MwebCoin {
            output_id: [id; 32],
            commitment: [0; 33],
            amount: 1_000,
            address_index: 0,
            blind: [0; 32],
            shared_secret: [0; 32],
            spend_key: Some([1; 32]),
            block_height: height,
            is_pegin: false,
            leaf_index,
        }
    }

    fn id_hex(id: u8) -> String {
        hex::encode([id; 32])
    }

    fn entry(kind: TxKind, outputs: &[u8], inputs: &[u8]) -> MwebHistoryEntry {
        MwebHistoryEntry {
            id: "tx".into(),
            kind,
            net_sats: -1_000,
            fee_sats: Some(100),
            timestamp: 1_700_000_000,
            output_ids: outputs.iter().map(|&b| id_hex(b)).collect(),
            input_ids: inputs.iter().map(|&b| id_hex(b)).collect(),
            confirmed_height: None,
        }
    }

    /// MSB-first leafset with the given leaves set (never empty).
    fn leafset(leaves: &[u64]) -> Vec<u8> {
        let max = leaves.iter().copied().max().unwrap_or(0);
        let mut out = vec![0u8; (max / 8 + 1) as usize];
        for &l in leaves {
            out[(l / 8) as usize] |= 1 << (7 - (l % 8) as u8);
        }
        out
    }

    fn run(
        entries: Vec<MwebHistoryEntry>,
        db: &MwebCoinDatabase,
        leaves: &[u64],
    ) -> Vec<Option<u32>> {
        let mut history = MwebHistory::default();
        for e in entries {
            history.record(e);
        }
        update_outgoing_confirmations(&mut history, db, &leafset(leaves), TIP);
        history.entries.iter().map(|e| e.confirmed_height).collect()
    }

    #[test]
    fn change_coin_height_gives_exact_confirmation() {
        let mut db = MwebCoinDatabase::default();
        db.insert(coin(1, Some(150), Some(7)));
        // Inputs still in the leafset must not matter once change is dated.
        db.insert(coin(2, Some(100), Some(3)));
        db.mark_spent(&[2; 32]);
        let got = run(vec![entry(TxKind::Pegout, &[1], &[2])], &db, &[3, 7]);
        assert_eq!(got, vec![Some(150)]);
    }

    #[test]
    fn exact_pegout_confirms_when_input_leaves_leave_leafset() {
        let mut db = MwebCoinDatabase::default();
        db.insert(coin(2, Some(100), Some(3)));
        db.mark_spent(&[2; 32]);
        let got = run(vec![entry(TxKind::Pegout, &[], &[2])], &db, &[9]);
        assert_eq!(got, vec![Some(TIP)]);
    }

    #[test]
    fn exact_pegout_stays_pending_while_input_leaf_present() {
        let mut db = MwebCoinDatabase::default();
        db.insert(coin(2, Some(100), Some(3)));
        db.mark_spent(&[2; 32]);
        let got = run(vec![entry(TxKind::MwebSend, &[], &[2])], &db, &[3]);
        assert_eq!(got, vec![None]);
    }

    #[test]
    fn input_missing_from_store_counts_as_confirmed_spent() {
        // After an MWEB wipe/resync a confirmed-spent coin is never re-found.
        let db = MwebCoinDatabase::default();
        let got = run(vec![entry(TxKind::Pegout, &[], &[2])], &db, &[9]);
        assert_eq!(got, vec![Some(TIP)]);
    }

    #[test]
    fn input_without_leaf_index_is_unverifiable() {
        let mut db = MwebCoinDatabase::default();
        db.insert(coin(2, None, None));
        db.mark_spent(&[2; 32]);
        let got = run(vec![entry(TxKind::Pegout, &[], &[2])], &db, &[9]);
        assert_eq!(got, vec![None]);
    }

    #[test]
    fn undated_change_with_present_inputs_stays_pending() {
        let mut db = MwebCoinDatabase::default();
        db.insert(coin(1, None, None));
        db.insert(coin(2, Some(100), Some(3)));
        db.mark_spent(&[2; 32]);
        let got = run(vec![entry(TxKind::Pegout, &[1], &[2])], &db, &[3]);
        assert_eq!(got, vec![None]);
    }

    #[test]
    fn legacy_trackless_entry_resolves_at_tip() {
        let db = MwebCoinDatabase::default();
        let got = run(vec![entry(TxKind::Pegout, &[], &[])], &db, &[0]);
        assert_eq!(got, vec![Some(TIP)]);
    }

    #[test]
    fn non_outgoing_kinds_are_untouched() {
        let db = MwebCoinDatabase::default();
        let got = run(
            vec![
                entry(TxKind::Pegin, &[], &[]),
                entry(TxKind::MwebReceive, &[], &[]),
            ],
            &db,
            &[0],
        );
        assert_eq!(got, vec![None, None]);
    }

    #[test]
    fn already_resolved_entry_is_not_rewritten() {
        let mut db = MwebCoinDatabase::default();
        db.insert(coin(1, Some(150), Some(7)));
        let mut e = entry(TxKind::Pegout, &[1], &[]);
        e.confirmed_height = Some(50);
        let got = run(vec![e], &db, &[7]);
        assert_eq!(got, vec![Some(50)]);
    }

    #[test]
    fn sealed_persistence_round_trips_and_migrates() {
        let dir = tempfile::tempdir().unwrap();
        let secret = crate::seed::MasterSecret::parse(
            &crate::descriptors::generate_mnemonic().unwrap(),
            None,
        )
        .unwrap();
        let key = [9u8; 32];
        let scheme = crate::dto::MwebScheme::default().to_master_scheme();

        // Start plaintext (legacy wallet), write a coin + history entry.
        let mut legacy =
            MwebRuntime::open(dir.path(), &secret, WalletNetwork::Testnet, scheme, None).unwrap();
        legacy.store.db_mut().insert(coin(1, Some(100), Some(4)));
        legacy.receive_index = 7;
        legacy.history.record(entry(TxKind::MwebSend, &[1], &[]));
        legacy.persist(dir.path()).unwrap();
        assert!(meta::mweb_db_path(dir.path()).is_file());

        // Reopen with a sealing key: legacy sqlite is read, next persist seals
        // everything and removes the plaintext files.
        let mut sealed =
            MwebRuntime::open(dir.path(), &secret, WalletNetwork::Testnet, scheme, Some(key))
                .unwrap();
        assert_eq!(sealed.store.db().unspent_count(), 1);
        assert_eq!(sealed.receive_index, 7);
        assert_eq!(sealed.history.entries.len(), 1);
        sealed.persist(dir.path()).unwrap();
        assert!(meta::mweb_coins_enc_path(dir.path()).is_file());
        assert!(!meta::mweb_db_path(dir.path()).is_file());
        assert!(!meta::mweb_history_path(dir.path()).is_file());

        // Sealed reload sees the same state; the sealed blob is not plaintext.
        let reloaded =
            MwebRuntime::open(dir.path(), &secret, WalletNetwork::Testnet, scheme, Some(key))
                .unwrap();
        assert_eq!(reloaded.store.db().unspent_count(), 1);
        assert_eq!(reloaded.receive_index, 7);
        assert_eq!(reloaded.history.entries.len(), 1);
        let blob = std::fs::read(meta::mweb_history_enc_path(dir.path())).unwrap();
        assert!(!blob.windows(2).any(|w| w == b"tx"));

        // Wrong key must fail loudly, not fall back to empty state.
        let wrong = MwebRuntime::open(
            dir.path(),
            &secret,
            WalletNetwork::Testnet,
            scheme,
            Some([0u8; 32]),
        );
        assert!(wrong.is_err());
    }

    #[test]
    fn empty_leafset_defers_input_based_resolution() {
        let mut history = MwebHistory::default();
        history.record(entry(TxKind::Pegout, &[], &[2]));
        let db = MwebCoinDatabase::default();
        update_outgoing_confirmations(&mut history, &db, &[], TIP);
        assert_eq!(history.entries[0].confirmed_height, None);
    }
}

/// Remove MWEB store/sync/index files for a from-scratch resync.
///
/// Deliberately keeps the history log (plain and sealed): entries stay visible
/// across a resync, and `known_outputs` stops re-found coins from duplicating
/// receive entries.
pub fn wipe_mweb_files(data_dir: &Path) -> Result<(), WalletError> {
    for path in [
        meta::mweb_db_path(data_dir),
        meta::mweb_sync_path(data_dir),
        meta::mweb_index_path(data_dir),
        meta::mweb_coins_enc_path(data_dir),
        meta::mweb_sync_enc_path(data_dir),
        meta::mweb_index_enc_path(data_dir),
    ] {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(WalletError::Io(e)),
        }
    }
    Ok(())
}

/// AEAD-seal `plain` under the wallet sealing key.
fn seal_bytes(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>, WalletError> {
    bdk_mweb::seal(key, plain).map_err(|e| WalletError::Mweb(e.to_string()))
}

/// Open a sealed file; `Ok(None)` when the file does not exist.
fn read_sealed(path: &Path, key: &[u8; 32]) -> Result<Option<Vec<u8>>, WalletError> {
    match fs::read(path) {
        Ok(blob) => bdk_mweb::open(key, &blob)
            .map(Some)
            .map_err(|e| WalletError::Mweb(format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(WalletError::Io(e)),
    }
}

fn write_sealed(path: &Path, blob: Vec<u8>) -> Result<(), WalletError> {
    crate::secrets::write_bytes(path, &blob)
        .map_err(|e| WalletError::Mweb(format!("{}: {e}", path.display())))
}

/// Delete plaintext-era MWEB files once their sealed replacements are written.
fn remove_legacy_plaintext_files(data_dir: &Path) {
    let db = meta::mweb_db_path(data_dir);
    for path in [
        db.clone(),
        std::path::PathBuf::from(format!("{}-wal", db.display())),
        std::path::PathBuf::from(format!("{}-shm", db.display())),
        meta::mweb_sync_path(data_dir),
        meta::mweb_index_path(data_dir),
        meta::mweb_history_path(data_dir),
    ] {
        let _ = fs::remove_file(&path);
    }
}

/// Load the coin store and its aggregate changeset. Preference order: sealed
/// file (when a key is available), legacy sqlite (migrated to sealed on the
/// next persist), empty.
fn load_store(
    data_dir: &Path,
    sealing_key: Option<&[u8; 32]>,
) -> Result<(MwebStore, ChangeSet), WalletError> {
    if let Some(key) = sealing_key {
        let path = meta::mweb_coins_enc_path(data_dir);
        if path.is_file() {
            let blob = fs::read(&path)?;
            let cs = bdk_mweb::open_changeset(key, &blob)
                .map_err(|e| WalletError::Mweb(format!("sealed MWEB store: {e}")))?;
            return Ok((MwebStore::from_changeset(cs.clone()), cs));
        }
    }
    let path = meta::mweb_db_path(data_dir);
    if !path.is_file() {
        return Ok((MwebStore::new(), ChangeSet::default()));
    }
    let mut conn = Connection::open(&path).map_err(|e| WalletError::Mweb(e.to_string()))?;
    let tx = conn
        .transaction()
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
    ChangeSet::init_sqlite_tables(&tx).map_err(|e| WalletError::Mweb(e.to_string()))?;
    let cs = ChangeSet::from_sqlite(&tx).map_err(|e| WalletError::Mweb(e.to_string()))?;
    tx.commit().map_err(|e| WalletError::Mweb(e.to_string()))?;
    Ok((MwebStore::from_changeset(cs.clone()), cs))
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

fn load_sync_state(
    data_dir: &Path,
    sealing_key: Option<&[u8; 32]>,
) -> Result<SyncState, WalletError> {
    if let Some(key) = sealing_key {
        if let Some(plain) = read_sealed(&meta::mweb_sync_enc_path(data_dir), key)? {
            return serde_json::from_slice(&plain).map_err(|e| WalletError::Mweb(e.to_string()));
        }
    }
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

fn load_receive_index(
    data_dir: &Path,
    sealing_key: Option<&[u8; 32]>,
) -> Result<u32, WalletError> {
    if let Some(key) = sealing_key {
        if let Some(plain) = read_sealed(&meta::mweb_index_enc_path(data_dir), key)? {
            let s = String::from_utf8_lossy(&plain);
            return Ok(s.trim().parse().unwrap_or(0));
        }
    }
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

fn load_history(
    data_dir: &Path,
    sealing_key: Option<&[u8; 32]>,
) -> Result<MwebHistory, WalletError> {
    if let Some(key) = sealing_key {
        if let Some(plain) = read_sealed(&meta::mweb_history_enc_path(data_dir), key)? {
            return serde_json::from_slice(&plain).map_err(|e| WalletError::Mweb(e.to_string()));
        }
    }
    MwebHistory::load(&meta::mweb_history_path(data_dir))
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
    let tx = extract_prepared_mweb_pegin(&prepared.psbt)
        .map_err(|e| WalletError::Mweb(e.to_string()))?;
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
        input_ids: Vec::new(),
        confirmed_height: None,
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
/// P2P goes first: a peer relays the tx through the Dandelion++ stem phase,
/// while litecoind RPC submits it as our own and gives up that origin privacy.
/// RPC is the fallback when no peer accepts it.
fn broadcast_mweb_tx(
    tx: &bdk_wallet::bitcoin::Transaction,
    rpc_url: Option<&str>,
    peers: &[String],
    network: WalletNetwork,
) -> Result<(), WalletError> {
    let p2p_err = match broadcast_via_p2p(tx, resolve_peers(peers)?, network) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };
    // Still prefer a stranger's node over our own RPC: relaying through a peer
    // we did not author the tx on is the whole point of the P2P path.
    let discovered = crate::discovery::discover_mweb_peers(network);
    if !discovered.is_empty() {
        match broadcast_via_p2p(tx, discovered, network) {
            Ok(()) => return Ok(()),
            Err(e) => eprintln!("MWEB broadcast: discovered peers also failed ({e})"),
        }
    }
    let Some(url) = rpc_url else {
        return Err(WalletError::Mweb(format!(
            "could not reach any MWEB peer to broadcast this transaction ({p2p_err}) — \
             check your connection, or set a Litecoin RPC URL in Settings"
        )));
    };
    match rpc::send_raw_transaction(url, &serialize_hex(tx)) {
        Ok(_) => {
            eprintln!("MWEB broadcast: fell back to litecoind RPC ({p2p_err})");
            Ok(())
        }
        Err(rpc_err) => Err(WalletError::Mweb(format!(
            "could not broadcast over P2P ({p2p_err}) or litecoind RPC ({rpc_err})"
        ))),
    }
}

/// Offer the tx to each peer in turn until one takes it.
fn broadcast_via_p2p(
    tx: &bdk_wallet::bitcoin::Transaction,
    addrs: Vec<std::net::SocketAddr>,
    network: WalletNetwork,
) -> Result<(), WalletError> {
    if addrs.is_empty() {
        return Err(WalletError::Mweb("no MWEB P2P peers reachable".into()));
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
    Err(WalletError::Mweb(format!(
        "P2P broadcast failed: {last_err}"
    )))
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
        input_ids: spent_ids.iter().map(hex::encode).collect(),
        confirmed_height: None,
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
        input_ids: spent_ids.iter().map(hex::encode).collect(),
        confirmed_height: None,
    });
    Ok(MwebBroadcastResult {
        wtxid,
        fee_sats: req.fee_sats,
    })
}
