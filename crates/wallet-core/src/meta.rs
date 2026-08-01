use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::WalletError;
use crate::network::WalletNetwork;
use crate::{MNEMONIC_ENC_FILE, MNEMONIC_FILE};

pub const WALLET_DB_FILE: &str = "wallet.sqlite";
pub const WALLET_META_FILE: &str = "wallet_meta.json";
pub const MWEB_DB_FILE: &str = "mweb.sqlite";
pub const MWEB_SYNC_FILE: &str = "mweb_sync.json";
pub const MWEB_INDEX_FILE: &str = "mweb_receive_index.txt";
pub const MWEB_HISTORY_FILE: &str = "mweb_history.json";
// Encrypted-at-rest replacements for the plaintext MWEB files above, sealed
// under the secret store's sealing key (see crate::sealed).
pub const MWEB_COINS_ENC_FILE: &str = "mweb_coins.enc";
pub const MWEB_SYNC_ENC_FILE: &str = "mweb_sync.enc";
pub const MWEB_INDEX_ENC_FILE: &str = "mweb_receive_index.enc";
pub const MWEB_HISTORY_ENC_FILE: &str = "mweb_history.enc";
/// Persist counter bound into every sealed MWEB blob. Not a secret: it reveals
/// only how many times the wallet has written, and its value is already visible
/// in the cleartext header of each envelope.
pub const MWEB_SEAL_COUNTER_FILE: &str = "mweb_seal_counter.txt";

fn default_true() -> bool {
    true
}

fn default_auto_lock_minutes() -> u32 {
    15
}

fn default_mweb_peers() -> Vec<String> {
    vec!["127.0.0.1:9333".into()]
}

/// Lightweight metadata stored beside the BDK sqlite DB (never secrets).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletMeta {
    pub network: WalletNetwork,
    pub electrum_url: String,
    /// Verify TLS certificates (CA chain + hostname) on ssl:// Electrum servers.
    /// Defaults to true; disabling allows self-signed community servers but
    /// removes man-in-the-middle protection.
    #[serde(default = "default_true")]
    pub electrum_validate_domain: bool,
    /// Fall back to the built-in public Electrum servers when the configured
    /// server is unreachable. Users running their own server for privacy can
    /// disable this so their addresses are never sent to public servers.
    #[serde(default = "default_true")]
    pub electrum_use_public_fallback: bool,
    /// Lock the wallet after this many minutes without user activity (0 = never).
    #[serde(default = "default_auto_lock_minutes")]
    pub auto_lock_minutes: u32,
    /// When true, the next sync runs a BIP84 full_scan; cleared after success.
    #[serde(default = "default_true")]
    pub needs_full_scan: bool,
    /// When true, MWEB needs a fresh scan after restore.
    #[serde(default = "default_true")]
    pub needs_mweb_scan: bool,
    /// Optional litecoind RPC for pure MWEB broadcast.
    #[serde(default)]
    pub litecoin_rpc_url: Option<String>,
    /// LIP-0006 P2P peers (`host:port`).
    #[serde(default = "default_mweb_peers")]
    pub mweb_peers: Vec<String>,
    /// MWEB key-derivation scheme; wallets from before this field existed
    /// default to Litecoin Core's layout.
    #[serde(default)]
    pub mweb_scheme: crate::dto::MwebScheme,
}

impl WalletMeta {
    pub fn new(network: WalletNetwork, electrum_url: Option<String>) -> Self {
        Self {
            network,
            electrum_url: electrum_url
                .unwrap_or_else(|| network.default_electrum_url().to_string()),
            electrum_validate_domain: true,
            electrum_use_public_fallback: true,
            auto_lock_minutes: default_auto_lock_minutes(),
            needs_full_scan: true,
            needs_mweb_scan: true,
            litecoin_rpc_url: None,
            mweb_peers: default_mweb_peers(),
            mweb_scheme: crate::dto::MwebScheme::default(),
        }
    }
}

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join(WALLET_DB_FILE)
}

pub fn meta_path(data_dir: &Path) -> PathBuf {
    data_dir.join(WALLET_META_FILE)
}

pub fn mweb_db_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MWEB_DB_FILE)
}

pub fn mweb_sync_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MWEB_SYNC_FILE)
}

pub fn mweb_index_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MWEB_INDEX_FILE)
}

pub fn mweb_history_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MWEB_HISTORY_FILE)
}

pub fn mweb_coins_enc_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MWEB_COINS_ENC_FILE)
}

pub fn mweb_sync_enc_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MWEB_SYNC_ENC_FILE)
}

pub fn mweb_index_enc_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MWEB_INDEX_ENC_FILE)
}

pub fn mweb_history_enc_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MWEB_HISTORY_ENC_FILE)
}

pub fn mweb_seal_counter_path(data_dir: &Path) -> PathBuf {
    data_dir.join(MWEB_SEAL_COUNTER_FILE)
}

pub fn write_meta(data_dir: &Path, meta: &WalletMeta) -> Result<(), WalletError> {
    fs::create_dir_all(data_dir)?;
    let json = serde_json::to_string_pretty(meta).map_err(|e| WalletError::Meta(e.to_string()))?;
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

pub fn validate_electrum_url(url: &str) -> Result<(), WalletError> {
    let url = url.trim();
    let ok = (url.starts_with("ssl://") || url.starts_with("tcp://"))
        && url
            .rfind(':')
            .map(|i| url[i + 1..].parse::<u16>().is_ok())
            .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(WalletError::Meta(
            "electrum URL must be ssl://host:port or tcp://host:port".into(),
        ))
    }
}

/// Remove wallet DB/meta/secret/MWEB files (and sqlite sidecars). Ignores missing paths.
pub fn remove_wallet_files(data_dir: &Path) -> Result<(), WalletError> {
    let db = db_path(data_dir);
    let mweb_db = mweb_db_path(data_dir);
    for path in [
        db.clone(),
        PathBuf::from(format!("{}-wal", db.display())),
        PathBuf::from(format!("{}-shm", db.display())),
        mweb_db.clone(),
        PathBuf::from(format!("{}-wal", mweb_db.display())),
        PathBuf::from(format!("{}-shm", mweb_db.display())),
        meta_path(data_dir),
        mweb_sync_path(data_dir),
        mweb_index_path(data_dir),
        mweb_history_path(data_dir),
        mweb_coins_enc_path(data_dir),
        mweb_sync_enc_path(data_dir),
        mweb_index_enc_path(data_dir),
        mweb_history_enc_path(data_dir),
        data_dir.join(MNEMONIC_FILE),
        data_dir.join(MNEMONIC_ENC_FILE),
    ] {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(WalletError::Io(e)),
        }
    }
    Ok(())
}
