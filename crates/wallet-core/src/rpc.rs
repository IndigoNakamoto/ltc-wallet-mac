use serde_json::json;

use crate::error::WalletError;

/// Broadcast a raw transaction via litecoind `sendrawtransaction`.
/// Returns the node response string (txid or wtxid depending on node/tx type).
pub fn send_raw_transaction(rpc_url: &str, tx_hex: &str) -> Result<String, WalletError> {
    let body = json!({
        "jsonrpc": "1.0",
        "id": "ltc-wallet",
        "method": "sendrawtransaction",
        "params": [tx_hex],
    });
    let resp = ureq::post(rpc_url)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| WalletError::Rpc(e.to_string()))?;
    let value: serde_json::Value = resp
        .into_json()
        .map_err(|e| WalletError::Rpc(e.to_string()))?;
    if let Some(err) = value.get("error").filter(|e| !e.is_null()) {
        return Err(WalletError::Rpc(err.to_string()));
    }
    value
        .get("result")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| WalletError::Rpc("missing result from sendrawtransaction".into()))
}
