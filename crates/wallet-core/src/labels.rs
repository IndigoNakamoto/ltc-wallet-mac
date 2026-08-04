//! Local transaction labels (non-secret sidecar next to wallet meta).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::WalletError;

pub const TX_LABELS_FILE: &str = "tx_labels.json";
pub const MAX_TX_LABEL_CHARS: usize = 140;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TxLabelsFile {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

fn default_version() -> u32 {
    1
}

pub fn labels_path(data_dir: &Path) -> PathBuf {
    data_dir.join(TX_LABELS_FILE)
}

/// Truncate to [`MAX_TX_LABEL_CHARS`] on Unicode scalar values (not bytes).
pub fn normalize_label(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= MAX_TX_LABEL_CHARS {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_TX_LABEL_CHARS).collect()
}

pub fn read_labels(data_dir: &Path) -> Result<TxLabelsFile, WalletError> {
    let path = labels_path(data_dir);
    if !path.is_file() {
        return Ok(TxLabelsFile {
            version: 1,
            labels: HashMap::new(),
        });
    }
    let bytes = fs::read_to_string(&path).map_err(WalletError::Io)?;
    serde_json::from_str(&bytes).map_err(|e| WalletError::Meta(e.to_string()))
}

pub fn write_labels(data_dir: &Path, file: &TxLabelsFile) -> Result<(), WalletError> {
    fs::create_dir_all(data_dir)?;
    let json = serde_json::to_string_pretty(file).map_err(|e| WalletError::Meta(e.to_string()))?;
    fs::write(labels_path(data_dir), json)?;
    Ok(())
}

/// Set or clear a label. Empty `label` deletes the entry.
pub fn set_label(data_dir: &Path, txid: &str, label: &str) -> Result<(), WalletError> {
    let txid = txid.trim();
    if txid.is_empty() {
        return Err(WalletError::Meta("txid required for label".into()));
    }
    let mut file = read_labels(data_dir)?;
    let note = normalize_label(label);
    if note.is_empty() {
        file.labels.remove(txid);
    } else {
        file.labels.insert(txid.to_string(), note);
    }
    if file.labels.is_empty() {
        let path = labels_path(data_dir);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(WalletError::Io(e)),
        }
    } else {
        write_labels(data_dir, &file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip_and_truncate() {
        let dir = tempdir().unwrap();
        set_label(dir.path(), "abc", "  hello  ").unwrap();
        let file = read_labels(dir.path()).unwrap();
        assert_eq!(file.labels.get("abc").map(String::as_str), Some("hello"));

        let long: String = "x".repeat(200);
        set_label(dir.path(), "abc", &long).unwrap();
        let file = read_labels(dir.path()).unwrap();
        assert_eq!(file.labels["abc"].chars().count(), MAX_TX_LABEL_CHARS);

        set_label(dir.path(), "abc", "").unwrap();
        assert!(!labels_path(dir.path()).is_file());
    }
}
