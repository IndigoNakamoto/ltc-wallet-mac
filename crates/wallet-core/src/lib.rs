//! Wallet-core: BDK boundary for the Litecoin Mac wallet.
//!
//! Public API exposes serde DTOs only. BDK types remain private.

mod app;
mod descriptors;
mod dto;
mod electrum;
mod error;
mod meta;
mod mweb;
mod mweb_history;
mod network;
mod rpc;
mod secrets;

pub use app::{MemoryBackedApp, WalletApp};
pub use dto::{
    CombinedSummary, CreateWalletRequest, CreateWalletResponse, MigrateEncryptRequest,
    MwebBroadcastResult, MwebSendRequest, MwebSyncProgress, PeginRequest, PeginResult,
    PegoutRequest, RestoreWalletRequest, SendRequest, SendResult, SyncResult, TxKind, TxRecord,
    UnlockRequest, UpdateSettingsRequest, WalletSettings, WalletSummary,
};
pub use error::WalletError;
pub use network::WalletNetwork;
pub use secrets::{
    EncryptedFileSecretStore, FileSecretStore, MemoryStore, SecretStore, UnlockableSecretStore,
};

/// Filename for the legacy plaintext mnemonic store.
pub const MNEMONIC_FILE: &str = "wallet.mnemonic";
/// Filename for the encrypted mnemonic store.
pub const MNEMONIC_ENC_FILE: &str = "wallet.mnemonic.enc";
