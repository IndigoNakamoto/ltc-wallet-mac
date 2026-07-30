//! Wallet-core: BDK boundary for the Litecoin Mac wallet.
//!
//! Public API exposes serde DTOs only. BDK types remain private.

mod app;
mod descriptors;
mod dto;
mod electrum;
mod error;
mod meta;
mod network;
mod secrets;

pub use app::WalletApp;
pub use dto::{
    CreateWalletRequest, CreateWalletResponse, RestoreWalletRequest, SendRequest, SendResult,
    SyncResult, WalletSummary,
};
pub use error::WalletError;
pub use network::WalletNetwork;
pub use secrets::{KeyringStore, MemoryStore, SecretStore};
