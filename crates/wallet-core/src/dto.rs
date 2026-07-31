use serde::{Deserialize, Serialize};

use crate::network::WalletNetwork;

/// Request to create a new wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWalletRequest {
    pub network: WalletNetwork,
    /// Optional Electrum URL; defaults from the selected network when omitted.
    #[serde(default)]
    pub electrum_url: Option<String>,
}

/// Response from wallet creation. The mnemonic is returned once for backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWalletResponse {
    pub mnemonic: String,
    pub summary: WalletSummary,
}

/// Request to restore a wallet from an existing mnemonic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreWalletRequest {
    pub mnemonic: String,
    pub network: WalletNetwork,
    #[serde(default)]
    pub electrum_url: Option<String>,
}

/// Snapshot of wallet balances and tip (amounts in litoshis).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletSummary {
    pub network: WalletNetwork,
    pub confirmed_sats: u64,
    pub trusted_pending_sats: u64,
    pub untrusted_pending_sats: u64,
    pub immature_sats: u64,
    pub total_sats: u64,
    pub tip_height: u32,
    pub receive_address: String,
}

/// Result of a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub summary: WalletSummary,
    pub new_txs: u32,
}

/// Request to send litecoin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendRequest {
    pub address: String,
    /// Ignored when [`Self::drain`] is true.
    #[serde(default)]
    pub amount_sats: u64,
    pub fee_rate_sat_vb: u64,
    /// When true, drain all spendable funds to `address` (send max).
    #[serde(default)]
    pub drain: bool,
}

/// Result of a broadcast send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResult {
    pub txid: String,
    pub fee_sats: u64,
}

/// A wallet-relevant transaction for history UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxRecord {
    pub txid: String,
    /// Net change for the wallet (received − sent); negative for outgoing.
    pub net_sats: i64,
    pub sent_sats: u64,
    pub received_sats: u64,
    /// Fee when computable (outgoing); `None` for incoming txs with foreign inputs.
    pub fee_sats: Option<u64>,
    /// Confirmation height when confirmed.
    pub height: Option<u32>,
    /// Confirmations relative to tip; `0` when unconfirmed.
    pub confirmations: u32,
    /// Confirmation timestamp (unix seconds) when known.
    pub timestamp: Option<u64>,
}

/// Request to unlock an encrypted wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockRequest {
    pub passphrase: String,
}

/// Request to migrate a plaintext mnemonic to an encrypted store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrateEncryptRequest {
    pub passphrase: String,
}

/// Electrum / peer settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletSettings {
    pub electrum_url: String,
    #[serde(default)]
    pub litecoin_rpc_url: Option<String>,
    #[serde(default)]
    pub mweb_peers: Vec<String>,
}

/// Request to update wallet settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    pub electrum_url: String,
    #[serde(default)]
    pub litecoin_rpc_url: Option<String>,
    #[serde(default)]
    pub mweb_peers: Vec<String>,
}

/// Combined transparent + MWEB balances (v0.2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CombinedSummary {
    pub transparent: WalletSummary,
    pub mweb_confirmed_sats: u64,
    pub mweb_unconfirmed_sats: u64,
    pub mweb_immature_sats: u64,
    pub mweb_total_sats: u64,
    pub mweb_receive_address: Option<String>,
    /// Tip height of last successful MWEB sync; `None` if never synced.
    pub mweb_synced_height: Option<u32>,
    pub mweb_stale: bool,
    pub mweb_status: String,
}

/// Progress of an in-flight MWEB UTXO download (poll while a sync runs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MwebSyncProgress {
    /// True while an MWEB sync pass is running.
    pub active: bool,
    /// UTXO leaves fetched so far in the current pass.
    pub fetched: u64,
    /// Total UTXO leaves the current pass will download (0 until known).
    pub total: u64,
}

/// Request to peg transparent LTC into MWEB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeginRequest {
    pub amount_sats: u64,
    #[serde(default = "default_mweb_fee")]
    pub mweb_fee_sats: u64,
    #[serde(default = "default_transparent_fee")]
    pub transparent_fee_sats: u64,
}

fn default_mweb_fee() -> u64 {
    50_000
}

fn default_transparent_fee() -> u64 {
    1_000
}

/// Result of a peg-in broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeginResult {
    pub txid: String,
    pub fee_sats: u64,
    pub maturity_blocks: u32,
}

/// Request to send MWEB → MWEB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MwebSendRequest {
    pub address: String,
    pub amount_sats: u64,
    #[serde(default = "default_mweb_fee")]
    pub fee_sats: u64,
}

/// Request to peg MWEB out to a transparent address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PegoutRequest {
    pub address: String,
    pub amount_sats: u64,
    #[serde(default = "default_mweb_fee")]
    pub fee_sats: u64,
}

/// Result of an MWEB-only broadcast (identified by wtxid).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MwebBroadcastResult {
    pub wtxid: String,
    pub fee_sats: u64,
}
