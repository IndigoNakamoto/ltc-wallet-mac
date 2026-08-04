//! Wallet-core: BDK boundary for the Litecoin Mac wallet.
//!
//! Public API exposes serde DTOs only. BDK types remain private.

mod aezeed;
mod app;
mod descriptors;
mod discovery;
mod dto;
mod electrum;
mod error;
pub mod explorer;
mod meta;
mod mweb;
mod mweb_history;
mod network;
mod rpc;
mod secrets;
mod seed;

pub use app::{MemoryBackedApp, WalletApp};
pub use seed::{derive_preview, DerivePreview, MasterSecret, MwebSchemePreview};
pub use dto::{
    CombinedSummary, CreateWalletRequest, CreateWalletResponse, FeeEstimate, FeeLadder,
    MigrateEncryptRequest, MwebBroadcastResult, MwebScheme, MwebSendPreview, MwebSendRequest,
    MwebSyncProgress, PeginPreview, PeginRequest, PeginResult, PegoutPreview, PegoutRequest,
    RestoreWalletRequest, SendPreview, SendRequest, SendResult, SyncResult, TxEnrichment, TxIo,
    TxKind, TxRecord, TxStatus, UnlockRequest, UpdateSettingsRequest, WalletSettings,
    WalletSummary, DEFAULT_MWEB_FEE_SATS,
};
pub use explorer::{DEFAULT_EXPLORER_BASE_URL, is_chain_txid};
pub use error::WalletError;
pub use network::WalletNetwork;
pub use secrets::{
    EncryptedFileSecretStore, FileSecretStore, MemoryStore, SecretStore, UnlockableSecretStore,
};

/// Filename for the legacy plaintext mnemonic store.
pub const MNEMONIC_FILE: &str = "wallet.mnemonic";
/// Filename for the encrypted mnemonic store.
pub const MNEMONIC_ENC_FILE: &str = "wallet.mnemonic.enc";
