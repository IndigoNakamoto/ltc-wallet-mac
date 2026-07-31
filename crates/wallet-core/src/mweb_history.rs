//! App-level MWEB activity log.
//!
//! The MWEB coin store is a pure UTXO set with no transaction records, so
//! peg-ins, peg-outs, MWEB sends, and MWEB receives are recorded here at
//! broadcast / discovery time and merged into the transparent history.
//!
//! The log survives an MWEB resync (it lives outside the wiped store files);
//! `known_outputs` prevents re-found coins from producing duplicate entries.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::dto::TxKind;
use crate::error::WalletError;

/// One MWEB-side wallet event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MwebHistoryEntry {
    /// Transparent txid for peg-ins, wtxid for MWEB-only txs, output id (hex)
    /// for receives discovered during sync.
    pub id: String,
    pub kind: TxKind,
    /// Net change for the wallet in litoshis; negative for outgoing.
    pub net_sats: i64,
    pub fee_sats: Option<u64>,
    /// Unix seconds when the entry was recorded (broadcast or discovery time).
    pub timestamp: u64,
    /// Hex output ids of our coins created by this tx (used to derive the
    /// confirmation height from the coin store).
    #[serde(default)]
    pub output_ids: Vec<String>,
}

/// Persisted MWEB activity log.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MwebHistory {
    #[serde(default)]
    pub entries: Vec<MwebHistoryEntry>,
    /// Output ids (hex) already attributed to an entry.
    #[serde(default)]
    pub known_outputs: BTreeSet<String>,
}

impl MwebHistory {
    pub fn load(path: &Path) -> Result<Self, WalletError> {
        match fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).map_err(|e| WalletError::Mweb(e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(WalletError::Io(e)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), WalletError> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| WalletError::Mweb(e.to_string()))?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Append an entry and mark its output ids as attributed.
    pub fn record(&mut self, entry: MwebHistoryEntry) {
        self.known_outputs.extend(entry.output_ids.iter().cloned());
        self.entries.push(entry);
    }

    pub fn is_known(&self, output_id_hex: &str) -> bool {
        self.known_outputs.contains(output_id_hex)
    }
}

/// Current unix time in seconds (0 if the clock is before the epoch).
pub fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, outputs: &[&str]) -> MwebHistoryEntry {
        MwebHistoryEntry {
            id: id.into(),
            kind: TxKind::Pegin,
            net_sats: -1_050_000,
            fee_sats: Some(51_000),
            timestamp: 1_700_000_000,
            output_ids: outputs.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn record_marks_outputs_known() {
        let mut h = MwebHistory::default();
        h.record(entry("txid1", &["aa", "bb"]));
        assert!(h.is_known("aa"));
        assert!(h.is_known("bb"));
        assert!(!h.is_known("cc"));
        assert_eq!(h.entries.len(), 1);
    }

    #[test]
    fn save_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mweb_history.json");
        let mut h = MwebHistory::default();
        h.record(entry("txid1", &["aa"]));
        h.save(&path).unwrap();
        let loaded = MwebHistory::load(&path).unwrap();
        assert_eq!(loaded.entries, h.entries);
        assert!(loaded.is_known("aa"));
    }

    #[test]
    fn load_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let h = MwebHistory::load(&dir.path().join("nope.json")).unwrap();
        assert!(h.entries.is_empty());
        assert!(h.known_outputs.is_empty());
    }
}
