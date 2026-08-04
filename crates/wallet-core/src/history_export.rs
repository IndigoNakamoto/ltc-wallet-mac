//! Non-secret history export helpers (CSV / JSON).

use std::collections::HashMap;

use serde::Serialize;

use crate::dto::{TxKind, TxRecord};
use crate::error::WalletError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryExportFormat {
    Csv,
    Json,
}

impl HistoryExportFormat {
    pub fn parse(raw: &str) -> Result<Self, WalletError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            other => Err(WalletError::Meta(format!(
                "unsupported export format '{other}' (use csv or json)"
            ))),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }

    pub fn default_filename(self) -> &'static str {
        match self {
            Self::Csv => "ltc-wallet-history.csv",
            Self::Json => "ltc-wallet-history.json",
        }
    }
}

fn kind_slug(kind: TxKind) -> &'static str {
    match kind {
        TxKind::Transparent => "transparent",
        TxKind::Pegin => "pegin",
        TxKind::Pegout => "pegout",
        TxKind::MwebSend => "mweb-send",
        TxKind::MwebReceive => "mweb-receive",
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r')
    {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

/// Build a CSV document of non-secret history fields.
pub fn to_csv(txs: &[TxRecord], labels: &HashMap<String, String>) -> String {
    let mut out = String::from(
        "txid,kind,net_sats,sent_sats,received_sats,fee_sats,height,confirmations,timestamp,note\n",
    );
    for tx in txs {
        let note = labels.get(&tx.txid).map(String::as_str).unwrap_or("");
        let fee = tx
            .fee_sats
            .map(|v| v.to_string())
            .unwrap_or_default();
        let height = tx.height.map(|v| v.to_string()).unwrap_or_default();
        let timestamp = tx.timestamp.map(|v| v.to_string()).unwrap_or_default();
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&tx.txid),
            kind_slug(tx.kind),
            tx.net_sats,
            tx.sent_sats,
            tx.received_sats,
            fee,
            height,
            tx.confirmations,
            timestamp,
            csv_escape(note),
        ));
    }
    out
}

#[derive(Serialize)]
struct ExportRow<'a> {
    txid: &'a str,
    kind: &'static str,
    net_sats: i64,
    sent_sats: u64,
    received_sats: u64,
    fee_sats: Option<u64>,
    height: Option<u32>,
    confirmations: u32,
    timestamp: Option<u64>,
    note: &'a str,
}

/// Build a pretty-printed JSON array of non-secret history fields.
pub fn to_json(txs: &[TxRecord], labels: &HashMap<String, String>) -> Result<String, WalletError> {
    let rows: Vec<ExportRow<'_>> = txs
        .iter()
        .map(|tx| ExportRow {
            txid: &tx.txid,
            kind: kind_slug(tx.kind),
            net_sats: tx.net_sats,
            sent_sats: tx.sent_sats,
            received_sats: tx.received_sats,
            fee_sats: tx.fee_sats,
            height: tx.height,
            confirmations: tx.confirmations,
            timestamp: tx.timestamp,
            note: labels.get(&tx.txid).map(String::as_str).unwrap_or(""),
        })
        .collect();
    serde_json::to_string_pretty(&rows).map_err(|e| WalletError::Meta(e.to_string()))
}

pub fn render(
    format: HistoryExportFormat,
    txs: &[TxRecord],
    labels: &HashMap<String, String>,
) -> Result<String, WalletError> {
    match format {
        HistoryExportFormat::Csv => Ok(to_csv(txs, labels)),
        HistoryExportFormat::Json => to_json(txs, labels),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::TxKind;

    fn sample_tx() -> TxRecord {
        TxRecord {
            txid: "abcd1234".into(),
            net_sats: -50_000,
            sent_sats: 50_000,
            received_sats: 0,
            fee_sats: Some(250),
            height: Some(100),
            confirmations: 3,
            timestamp: Some(1_700_000_000),
            kind: TxKind::Transparent,
        }
    }

    #[test]
    fn csv_includes_header_note_and_escapes() {
        let tx = TxRecord {
            txid: "id,with,comma".into(),
            ..sample_tx()
        };
        let mut labels = HashMap::new();
        labels.insert("id,with,comma".into(), "hello \"world\"".into());
        let csv = to_csv(&[tx], &labels);
        assert!(csv.starts_with(
            "txid,kind,net_sats,sent_sats,received_sats,fee_sats,height,confirmations,timestamp,note\n"
        ));
        assert!(csv.contains("\"id,with,comma\""));
        assert!(csv.contains("transparent"));
        assert!(csv.contains("\"hello \"\"world\"\"\""));
    }

    #[test]
    fn json_round_trips_fields() {
        let tx = sample_tx();
        let mut labels = HashMap::new();
        labels.insert("abcd1234".into(), "coffee".into());
        let json = to_json(&[tx], &labels).unwrap();
        assert!(json.contains("\"txid\": \"abcd1234\""));
        assert!(json.contains("\"note\": \"coffee\""));
        assert!(json.contains("\"kind\": \"transparent\""));
    }
}
