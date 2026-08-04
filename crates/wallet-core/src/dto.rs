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

/// Request to restore a wallet from an existing seed: a BIP39 mnemonic, an
/// aezeed mnemonic (Nexus), or a root extended private key (xprv/zprv/Ltpv).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreWalletRequest {
    /// Seed input; the kind is auto-detected. Named `mnemonic` for backward
    /// compatibility with existing callers.
    pub mnemonic: String,
    pub network: WalletNetwork,
    #[serde(default)]
    pub electrum_url: Option<String>,
    /// MWEB key-derivation scheme to restore under.
    #[serde(default)]
    pub mweb_scheme: MwebScheme,
    /// aezeed cipher-seed passphrase, when the seed is aezeed and one was set.
    #[serde(default)]
    pub aezeed_passphrase: Option<String>,
}

/// Which BIP32 layout derives the MWEB scan/spend keys.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MwebScheme {
    /// Litecoin Core 0.21: `m/0'/100'/{0,1}'`.
    #[default]
    LitecoinCore,
    /// LIP-0004 text: `m/1/0/{100',101'}`.
    Lip0004,
    /// mwebd / Nexus (BIP43 purpose 1000): `m/1000'/2'/0'/{0,1}'`.
    Mwebd,
}

impl MwebScheme {
    pub(crate) fn to_master_scheme(self) -> bdk_mweb::keys::MasterKeyScheme {
        match self {
            Self::LitecoinCore => bdk_mweb::keys::MasterKeyScheme::LitecoinCore,
            Self::Lip0004 => bdk_mweb::keys::MasterKeyScheme::Lip0004,
            Self::Mwebd => bdk_mweb::keys::MasterKeyScheme::Mwebd,
        }
    }
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
    /// Wall-clock time in the Electrum phase.
    pub electrum_ms: u64,
    /// Wall-clock time in the MWEB phase; 0 when MWEB is not active.
    pub mweb_ms: u64,
    /// Electrum server that served this sync.
    #[serde(default)]
    pub electrum_server: String,
    /// Cross-check / trust warnings the user should see (empty when all clear).
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Request to send litecoin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendRequest {
    pub address: String,
    /// Ignored when [`Self::drain`] is true.
    #[serde(default)]
    pub amount_sats: u64,
    /// Fee rate in sat/vB. When omitted or zero, the wallet estimates from Electrum.
    #[serde(default)]
    pub fee_rate_sat_vb: Option<u64>,
    /// When true, drain all spendable funds to `address` (send max).
    #[serde(default)]
    pub drain: bool,
    /// When non-empty, spend only these transparent outpoints (`txid:vout`).
    /// Incompatible with [`Self::drain`].
    #[serde(default)]
    pub selected_outpoints: Option<Vec<String>>,
}

/// One spendable transparent UTXO for coin control.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UtxoRecord {
    /// `txid:vout`
    pub outpoint: String,
    pub txid: String,
    pub vout: u32,
    pub amount_sats: u64,
    /// `external` (receive) or `internal` (change).
    pub keychain: String,
    pub confirmations: u32,
    /// Frozen coins are skipped by automatic coin selection.
    pub locked: bool,
}

/// Freeze or unfreeze a transparent UTXO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetUtxoLockedRequest {
    /// `txid:vout`
    pub outpoint: String,
    pub locked: bool,
}

/// Result of a broadcast send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResult {
    pub txid: String,
    pub fee_sats: u64,
}

/// Dry-run of a send: absolute fee and recipient amount before broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendPreview {
    /// Amount that will arrive at the recipient (for drain: total − fee).
    pub amount_sats: u64,
    pub fee_sats: u64,
    pub fee_rate_sat_vb: u64,
}

/// Network fee-rate estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    pub fee_rate_sat_vb: u64,
    /// True when Electrum had no estimate and the floor rate was used.
    pub is_fallback: bool,
}

/// What a history record represents, so the UI can label it.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TxKind {
    /// Plain transparent transaction.
    #[default]
    Transparent,
    Pegin,
    Pegout,
    MwebSend,
    MwebReceive,
}

/// A wallet-relevant transaction for history UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxRecord {
    /// Transparent txid; wtxid or output id for MWEB-only records.
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
    /// Kind of activity (transparent, peg-in, peg-out, MWEB send/receive).
    #[serde(default)]
    pub kind: TxKind,
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

fn default_true() -> bool {
    true
}

fn default_auto_lock_minutes() -> u32 {
    15
}

fn default_explorer_base_url() -> String {
    crate::explorer::DEFAULT_EXPLORER_BASE_URL.to_string()
}

/// Electrum / peer / explorer settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletSettings {
    pub electrum_url: String,
    /// Verify TLS certificates on ssl:// Electrum servers (default true).
    #[serde(default = "default_true")]
    pub electrum_validate_domain: bool,
    /// Fall back to built-in public Electrum servers when the configured one
    /// is down (default true). Disable to keep addresses off public servers.
    #[serde(default = "default_true")]
    pub electrum_use_public_fallback: bool,
    /// Lock the wallet after this many idle minutes (0 = never).
    #[serde(default = "default_auto_lock_minutes")]
    pub auto_lock_minutes: u32,
    /// Server the current session last connected to (read-only, may be a fallback).
    #[serde(default)]
    pub electrum_active_url: Option<String>,
    #[serde(default)]
    pub litecoin_rpc_url: Option<String>,
    #[serde(default)]
    pub mweb_peers: Vec<String>,
    /// Active MWEB key-derivation scheme (changing it requires an MWEB resync).
    #[serde(default)]
    pub mweb_scheme: MwebScheme,
    /// Block explorer base URL (litview / self-hosted LRK).
    #[serde(default = "default_explorer_base_url")]
    pub explorer_base_url: String,
    /// Show LTC/USD under the balance hero (fetched from the explorer API).
    #[serde(default = "default_true")]
    pub show_fiat: bool,
    /// Show explorer fee-rate suggestion chips on the send form.
    #[serde(default = "default_true")]
    pub use_explorer_fee_hints: bool,
}

/// Request to update wallet settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    pub electrum_url: String,
    /// Verify TLS certificates on ssl:// Electrum servers (default true).
    #[serde(default = "default_true")]
    pub electrum_validate_domain: bool,
    /// Fall back to built-in public Electrum servers when the configured one
    /// is down (default true).
    #[serde(default = "default_true")]
    pub electrum_use_public_fallback: bool,
    /// Lock the wallet after this many idle minutes (0 = never).
    #[serde(default = "default_auto_lock_minutes")]
    pub auto_lock_minutes: u32,
    #[serde(default)]
    pub litecoin_rpc_url: Option<String>,
    #[serde(default)]
    pub mweb_peers: Vec<String>,
    #[serde(default = "default_explorer_base_url")]
    pub explorer_base_url: String,
    #[serde(default = "default_true")]
    pub show_fiat: bool,
    #[serde(default = "default_true")]
    pub use_explorer_fee_hints: bool,
}

/// Whether a destination matches a previously used transparent wallet address.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddressReuseHint {
    /// True when the address is a revealed BIP84 receive/change address that has been used.
    pub reused: bool,
}

/// Request to set or clear a local transaction label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTxLabelRequest {
    pub txid: String,
    /// Empty string clears the label.
    pub label: String,
}

/// Address-book contact kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContactKind {
    Public,
    Private,
}

/// Local address-book entry (non-secret).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContactRecord {
    pub id: String,
    pub name: String,
    pub address: String,
    pub kind: ContactKind,
}

/// Create or update a contact. Empty / omitted `id` creates a new entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertContactRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub address: String,
    pub kind: ContactKind,
}

/// Delete a contact by id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteContactRequest {
    pub id: String,
}

/// One input or output from explorer tx enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxIo {
    pub address: String,
    pub value_sats: u64,
    /// True when this address is one of the wallet's revealed SPKs (local match).
    pub is_wallet: bool,
}

/// Confirmation status from the explorer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxStatus {
    pub confirmed: bool,
    pub block_height: Option<u32>,
    pub block_hash: Option<String>,
    pub block_time: Option<u64>,
}

/// Full transaction view from litview (`/api/tx/{txid}`), with local wallet tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxEnrichment {
    pub txid: String,
    pub fee_sats: Option<u64>,
    pub size: Option<u32>,
    pub weight: Option<u32>,
    pub status: TxStatus,
    pub inputs: Vec<TxIo>,
    pub outputs: Vec<TxIo>,
}

/// Suggested fee rates from litview (`/api/v1/fees/recommended`), sat/vB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeLadder {
    pub fastest_sat_vb: u64,
    pub half_hour_sat_vb: u64,
    pub hour_sat_vb: u64,
    pub economy_sat_vb: Option<u64>,
    pub minimum_sat_vb: Option<u64>,
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

/// Default MWEB kernel fee (0.0005 LTC). Used when the request leaves fee as 0.
pub const DEFAULT_MWEB_FEE_SATS: u64 = 50_000;

/// Request to peg transparent LTC into MWEB.
///
/// `amount_sats` is the peg-in output value (HogEx). The private coin credited is
/// `amount_sats - mweb_fee_sats`. Transparent inputs also pay `transparent_fee_sats`.
/// Fee fields of `0` mean “pick automatically”.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeginRequest {
    /// Ignored when [`Self::drain`] is true.
    #[serde(default)]
    pub amount_sats: u64,
    /// MWEB kernel fee; `0` = [`DEFAULT_MWEB_FEE_SATS`].
    #[serde(default)]
    pub mweb_fee_sats: u64,
    /// Transparent miner fee; `0` = estimate from Electrum.
    #[serde(default)]
    pub transparent_fee_sats: u64,
    /// Peg in all trusted-spendable transparent funds (minus transparent fee).
    #[serde(default)]
    pub drain: bool,
}

/// Dry-run of a peg-in before broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeginPreview {
    /// Peg-in output value (leaves transparent).
    pub amount_sats: u64,
    /// Private coin that will be credited after the MWEB fee.
    pub private_credit_sats: u64,
    pub mweb_fee_sats: u64,
    pub transparent_fee_sats: u64,
    /// Total transparent spend: peg-in amount + transparent fee.
    pub total_from_transparent_sats: u64,
}

/// Result of a peg-in broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeginResult {
    pub txid: String,
    pub fee_sats: u64,
    pub maturity_blocks: u32,
}

/// Request to send MWEB → MWEB. `fee_sats` of `0` means auto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MwebSendRequest {
    pub address: String,
    /// Ignored when [`Self::drain`] is true.
    #[serde(default)]
    pub amount_sats: u64,
    #[serde(default)]
    pub fee_sats: u64,
    /// Send all spendable private funds minus the kernel fee.
    #[serde(default)]
    pub drain: bool,
}

/// Dry-run of a private send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MwebSendPreview {
    pub amount_sats: u64,
    pub fee_sats: u64,
}

/// Request to peg MWEB out to a transparent address. `fee_sats` of `0` means auto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PegoutRequest {
    pub address: String,
    /// Ignored when [`Self::drain`] is true.
    #[serde(default)]
    pub amount_sats: u64,
    #[serde(default)]
    pub fee_sats: u64,
    /// Peg out all spendable private funds minus the kernel fee.
    #[serde(default)]
    pub drain: bool,
}

/// Dry-run of a peg-out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PegoutPreview {
    pub amount_sats: u64,
    pub fee_sats: u64,
    /// Minimum non-dust for the destination script (litoshis).
    pub dust_sats: u64,
}

/// Result of an MWEB-only broadcast (identified by wtxid).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MwebBroadcastResult {
    pub wtxid: String,
    pub fee_sats: u64,
}
