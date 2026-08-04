//! First-party litview / LRK explorer helpers and HTTP client.
//!
//! Deep-link URL builders are pure. Fetches use `ureq` with short timeouts and
//! never upload wallet address lists — callers mark `is_wallet` locally.

use std::collections::HashSet;
use std::time::Duration;

use serde::Deserialize;

use crate::dto::{FeeLadder, TxEnrichment, TxIo, TxStatus};
use crate::error::WalletError;

/// Default hosted LRK instance.
pub const DEFAULT_EXPLORER_BASE_URL: &str = "https://litview.space";

const HTTP_TIMEOUT: Duration = Duration::from_secs(12);

fn default_explorer_base() -> String {
    DEFAULT_EXPLORER_BASE_URL.to_string()
}

/// Normalize and validate an explorer base URL (`https://host` or `http://host`).
pub fn normalize_base_url(raw: &str) -> Result<String, WalletError> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(default_explorer_base());
    }
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(WalletError::Meta(
            "explorer URL must start with https:// or http://".into(),
        ));
    }
    let rest = if lower.starts_with("https://") {
        &trimmed["https://".len()..]
    } else {
        &trimmed["http://".len()..]
    };
    if rest.is_empty() || rest.contains(' ') || rest.contains('\n') {
        return Err(WalletError::Meta("explorer URL is missing a host".into()));
    }
    Ok(trimmed.to_string())
}

/// True when `id` looks like a 32-byte hex transaction id (transparent chain).
pub fn is_chain_txid(id: &str) -> bool {
    let id = id.trim();
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `{base}/tx/{txid}`
pub fn tx_url(base: &str, txid: &str) -> Result<String, WalletError> {
    let base = normalize_base_url(base)?;
    let txid = txid.trim();
    if !is_chain_txid(txid) {
        return Err(WalletError::Meta(
            "not a transparent transaction id — cannot open in explorer".into(),
        ));
    }
    Ok(format!("{base}/tx/{txid}"))
}

/// `{base}/block/{hash}` when hash looks like a block hash.
pub fn block_url(base: &str, block_hash: &str) -> Result<String, WalletError> {
    let base = normalize_base_url(base)?;
    let hash = block_hash.trim();
    if !is_chain_txid(hash) {
        return Err(WalletError::Meta("invalid block hash".into()));
    }
    Ok(format!("{base}/block/{hash}"))
}

/// Reject non-http(s) URLs before handing them to the OS opener.
pub fn validate_open_url(url: &str) -> Result<(), WalletError> {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        Ok(())
    } else {
        Err(WalletError::Meta(
            "only http(s) explorer URLs can be opened".into(),
        ))
    }
}

fn get_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, WalletError> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(HTTP_TIMEOUT)
        .timeout_read(HTTP_TIMEOUT)
        .build();
    let resp = agent.get(url).call().map_err(|e| {
        WalletError::Explorer(format!("request failed: {}", crate::rpc::redact_userinfo(&e.to_string())))
    })?;
    resp.into_json::<T>()
        .map_err(|e| WalletError::Explorer(format!("invalid JSON: {e}")))
}

fn get_text(url: &str) -> Result<String, WalletError> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(HTTP_TIMEOUT)
        .timeout_read(HTTP_TIMEOUT)
        .build();
    let resp = agent.get(url).call().map_err(|e| {
        WalletError::Explorer(format!("request failed: {}", crate::rpc::redact_userinfo(&e.to_string())))
    })?;
    resp.into_string()
        .map_err(|e| WalletError::Explorer(format!("read failed: {e}")))
}

/// `GET {base}/api/mempool/price` — plain number body.
pub fn fetch_spot_price(base: &str) -> Result<f64, WalletError> {
    let base = normalize_base_url(base)?;
    let url = format!("{base}/api/mempool/price");
    let body = get_text(&url)?;
    body.trim()
        .parse::<f64>()
        .map_err(|e| WalletError::Explorer(format!("invalid price: {e}")))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeeLadderRaw {
    fastest_fee: f64,
    half_hour_fee: f64,
    hour_fee: f64,
    #[serde(default)]
    economy_fee: Option<f64>,
    #[serde(default)]
    minimum_fee: Option<f64>,
}

fn ceil_sat_vb(v: f64) -> u64 {
    if !v.is_finite() || v <= 0.0 {
        1
    } else {
        v.ceil() as u64
    }
}

/// `GET {base}/api/v1/fees/recommended`
pub fn fetch_fee_ladder(base: &str) -> Result<FeeLadder, WalletError> {
    let base = normalize_base_url(base)?;
    let url = format!("{base}/api/v1/fees/recommended");
    let raw: FeeLadderRaw = get_json(&url)?;
    Ok(FeeLadder {
        fastest_sat_vb: ceil_sat_vb(raw.fastest_fee),
        half_hour_sat_vb: ceil_sat_vb(raw.half_hour_fee),
        hour_sat_vb: ceil_sat_vb(raw.hour_fee),
        economy_sat_vb: raw.economy_fee.map(ceil_sat_vb),
        minimum_sat_vb: raw.minimum_fee.map(ceil_sat_vb),
    })
}

#[derive(Debug, Deserialize)]
struct ApiTx {
    txid: String,
    #[serde(default)]
    fee: Option<u64>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    weight: Option<u32>,
    #[serde(default)]
    status: Option<ApiTxStatus>,
    #[serde(default)]
    vin: Vec<ApiVin>,
    #[serde(default)]
    vout: Vec<ApiVout>,
}

#[derive(Debug, Deserialize)]
struct ApiTxStatus {
    #[serde(default)]
    confirmed: bool,
    #[serde(default)]
    block_height: Option<u32>,
    #[serde(default)]
    block_hash: Option<String>,
    #[serde(default)]
    block_time: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ApiVin {
    #[serde(default)]
    prevout: Option<ApiPrevout>,
}

#[derive(Debug, Deserialize)]
struct ApiPrevout {
    #[serde(default)]
    scriptpubkey_address: Option<String>,
    #[serde(default)]
    value: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ApiVout {
    #[serde(default)]
    scriptpubkey_address: Option<String>,
    #[serde(default)]
    value: Option<u64>,
}

/// `GET {base}/api/tx/{txid}`, then mark IO belonging to `wallet_addresses`.
pub fn fetch_tx_detail(
    base: &str,
    txid: &str,
    wallet_addresses: &HashSet<String>,
) -> Result<TxEnrichment, WalletError> {
    let base = normalize_base_url(base)?;
    let txid = txid.trim();
    if !is_chain_txid(txid) {
        return Err(WalletError::Meta(
            "not a transparent transaction id — cannot enrich from explorer".into(),
        ));
    }
    let url = format!("{base}/api/tx/{txid}");
    let raw: ApiTx = get_json(&url)?;
    let status = raw.status.unwrap_or(ApiTxStatus {
        confirmed: false,
        block_height: None,
        block_hash: None,
        block_time: None,
    });
    let map_io = |address: Option<String>, value: Option<u64>| -> TxIo {
        let address = address.unwrap_or_default();
        let is_wallet = !address.is_empty() && wallet_addresses.contains(&address);
        TxIo {
            address,
            value_sats: value.unwrap_or(0),
            is_wallet,
        }
    };
    let inputs = raw
        .vin
        .into_iter()
        .map(|vin| {
            let prev = vin.prevout.unwrap_or(ApiPrevout {
                scriptpubkey_address: None,
                value: None,
            });
            map_io(prev.scriptpubkey_address, prev.value)
        })
        .collect();
    let outputs = raw
        .vout
        .into_iter()
        .map(|vout| map_io(vout.scriptpubkey_address, vout.value))
        .collect();
    Ok(TxEnrichment {
        txid: raw.txid,
        fee_sats: raw.fee,
        size: raw.size,
        weight: raw.weight,
        status: TxStatus {
            confirmed: status.confirmed,
            block_height: status.block_height,
            block_hash: status.block_hash,
            block_time: status.block_time,
        },
        inputs,
        outputs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_default_and_trailing_slash() {
        assert_eq!(
            normalize_base_url("").unwrap(),
            DEFAULT_EXPLORER_BASE_URL
        );
        assert_eq!(
            normalize_base_url("https://litview.space/").unwrap(),
            "https://litview.space"
        );
    }

    #[test]
    fn rejects_non_http_base() {
        assert!(normalize_base_url("ftp://x").is_err());
        assert!(normalize_base_url("litview.space").is_err());
    }

    #[test]
    fn builds_tx_url() {
        let txid = "cf24383eacf8c34c01a0114df0d58e438a5d4fb685570f43c2cef84522361bd0";
        assert_eq!(
            tx_url("https://litview.space", txid).unwrap(),
            format!("https://litview.space/tx/{txid}")
        );
        assert!(tx_url("https://litview.space", "not-a-txid").is_err());
    }

    #[test]
    fn chain_txid_hex() {
        assert!(is_chain_txid(
            "cf24383eacf8c34c01a0114df0d58e438a5d4fb685570f43c2cef84522361bd0"
        ));
        assert!(!is_chain_txid("abc"));
        assert!(!is_chain_txid(&"g".repeat(64)));
    }
}
