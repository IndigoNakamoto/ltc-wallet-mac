use thiserror::Error;

/// Errors from the wallet-core public surface.
#[derive(Debug, Error)]
pub enum WalletError {
    #[error("wallet already exists at data directory")]
    AlreadyExists,

    #[error("wallet not found at data directory")]
    NotFound,

    #[error("wallet is not loaded")]
    NotLoaded,

    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("invalid address: {0}")]
    InvalidAddress(String),

    #[error("descriptor error: {0}")]
    Descriptor(String),

    #[error("persistence error: {0}")]
    Persist(String),

    #[error("secret store error: {0}")]
    SecretStore(String),

    #[error("metadata error: {0}")]
    Meta(String),

    #[error("electrum error: {0}")]
    Electrum(String),

    #[error("failed to build transaction: {0}")]
    BuildTx(String),

    #[error("failed to sign transaction: {0}")]
    Sign(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
