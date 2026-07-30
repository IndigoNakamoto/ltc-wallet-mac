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

    #[error("descriptor error: {0}")]
    Descriptor(String),

    #[error("persistence error: {0}")]
    Persist(String),

    #[error("secret store error: {0}")]
    SecretStore(String),

    #[error("metadata error: {0}")]
    Meta(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}
