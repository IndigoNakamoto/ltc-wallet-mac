use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::WalletError;
use crate::network::WalletNetwork;

pub const WALLET_DB_FILE: &str = "wallet.sqlite";
pub const WALLET_META_FILE: &str = "wallet_meta.json";

fn default_true() -> bool {
    true
}

/// Lightweight metadata stored beside the BDK sqlite DB (never secrets).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletMeta {
    pub network: WalletNetwork,
    pub electrum_url: String,
    /// When true, the next sync runs a BIP84 full_scan; cleared after success.
    #[serde(default = "default_true")]
    pub needs_full_scan: bool,
}

impl WalletMeta {
    pub fn new(network: WalletNetwork, electrum_url: Option<String>) -> Self {
        Self {
            network,
            electrum_url: electrum_url
                .unwrap_or_else(|| network.default_electrum_url().to_string()),
            needs_full_scan: true,
        }
    }
}

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join(WALLET_DB_FILE)
}

pub fn meta_path(data_dir: &Path) -> PathBuf {
    data_dir.join(WALLET_META_FILE)
}

pub fn write_meta(data_dir: &Path, meta: &WalletMeta) -> Result<(), WalletError> {
    fs::create_dir_all(data_dir)?;
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| WalletError::Meta(e.to_string()))?;
    fs::write(meta_path(data_dir), json)?;
    Ok(())
}

pub fn read_meta(data_dir: &Path) -> Result<WalletMeta, WalletError> {
    let bytes = fs::read_to_string(meta_path(data_dir)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            WalletError::NotFound
        } else {
            WalletError::Io(e)
        }
    })?;
    serde_json::from_str(&bytes).map_err(|e| WalletError::Meta(e.to_string()))
}

pub fn wallet_files_exist(data_dir: &Path) -> bool {
    db_path(data_dir).is_file()
}
