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
    pub amount_sats: u64,
    pub fee_rate_sat_vb: u64,
}

/// Result of a broadcast send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResult {
    pub txid: String,
    pub fee_sats: u64,
}
