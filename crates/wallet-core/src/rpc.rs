use base64::Engine;
use serde_json::json;

use crate::error::WalletError;

/// Broadcast a raw transaction via litecoind `sendrawtransaction`.
/// Returns the node response string (txid or wtxid depending on node/tx type).
///
/// Authentication: credentials embedded in the URL (`http://user:pass@host:port`)
/// take precedence. Without them, falls back to the litecoind `.cookie` file in
/// the default data directory, so a bare `http://127.0.0.1:9332` works with a
/// stock local node even though the cookie rotates on every node restart.
pub fn send_raw_transaction(rpc_url: &str, tx_hex: &str) -> Result<String, WalletError> {
    let rpc_url = &normalize_rpc_url(rpc_url);
    let body = json!({
        "jsonrpc": "1.0",
        "id": "ltc-wallet",
        "method": "sendrawtransaction",
        "params": [tx_hex],
    });
    let mut request = ureq::post(rpc_url).set("Content-Type", "application/json");
    if !url_has_credentials(rpc_url) {
        if let Some(cookie) = read_default_cookie() {
            let encoded = base64::engine::general_purpose::STANDARD.encode(cookie);
            request = request.set("Authorization", &format!("Basic {encoded}"));
        }
    }
    let value: serde_json::Value = match request.send_json(body) {
        Ok(resp) => resp.into_json().map_err(|e| WalletError::Rpc(e.to_string()))?,
        // litecoind reports JSON-RPC failures (e.g. rejected txs) as HTTP 500
        // with the real reason in the body; surface that instead of the status.
        Err(ureq::Error::Status(code, resp)) => match resp.into_json::<serde_json::Value>() {
            Ok(v) => v,
            Err(_) => {
                let hint = if code == 401 {
                    " (authentication failed — set rpcuser/rpcpassword in litecoin.conf or use http://user:pass@host:port)"
                } else {
                    ""
                };
                return Err(WalletError::Rpc(format!("http status {code}{hint}")));
            }
        },
        // ureq errors embed the request URL, which may carry user:pass userinfo.
        Err(e) => return Err(WalletError::Rpc(redact_userinfo(&e.to_string()))),
    };
    if let Some(err) = value.get("error").filter(|e| !e.is_null()) {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .map(|m| m.to_string())
            .unwrap_or_else(|| err.to_string());
        return Err(WalletError::Rpc(crate::error::humanize_broadcast_error(&msg)));
    }
    value
        .get("result")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| WalletError::Rpc("missing result from sendrawtransaction".into()))
}

/// Prepend `http://` when the URL has no scheme (e.g. a bare `127.0.0.1:9332`),
/// which ureq would otherwise reject as a relative URL.
pub fn normalize_rpc_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

/// Strip `user:pass@` userinfo from any URL embedded in `msg` so RPC
/// credentials never reach logs or UI error text.
fn redact_userinfo(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    while let Some(idx) = rest.find("://") {
        let (head, tail) = rest.split_at(idx + 3);
        out.push_str(head);
        let authority_end = tail
            .find(['/', '?', '#', ' ', ')', '"'])
            .unwrap_or(tail.len());
        let authority = &tail[..authority_end];
        if let Some(at) = authority.rfind('@') {
            out.push_str("[redacted]@");
            out.push_str(&authority[at + 1..]);
        } else {
            out.push_str(authority);
        }
        rest = &tail[authority_end..];
    }
    out.push_str(rest);
    out
}

/// True when the URL authority contains userinfo (`scheme://user:pass@host`).
fn url_has_credentials(url: &str) -> bool {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    authority.contains('@')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_adds_scheme_when_missing() {
        assert_eq!(normalize_rpc_url("127.0.0.1:9332"), "http://127.0.0.1:9332");
        assert_eq!(normalize_rpc_url(" 127.0.0.1:9332 "), "http://127.0.0.1:9332");
        assert_eq!(normalize_rpc_url("http://127.0.0.1:9332"), "http://127.0.0.1:9332");
        assert_eq!(
            normalize_rpc_url("https://user:pass@host:9332"),
            "https://user:pass@host:9332"
        );
        assert_eq!(normalize_rpc_url(""), "");
    }

    #[test]
    fn redacts_userinfo_in_error_text() {
        assert_eq!(
            redact_userinfo("http://user:secret@127.0.0.1:9332/: Connection Failed"),
            "http://[redacted]@127.0.0.1:9332/: Connection Failed"
        );
        assert_eq!(
            redact_userinfo("error at https://host:9332/path"),
            "error at https://host:9332/path"
        );
        assert_eq!(redact_userinfo("no url here"), "no url here");
    }

    #[test]
    fn detects_url_credentials() {
        assert!(url_has_credentials("http://user:pass@127.0.0.1:9332"));
        assert!(!url_has_credentials("http://127.0.0.1:9332"));
        assert!(!url_has_credentials("http://127.0.0.1:9332/path?a=b@c"));
    }
}

/// Read `__cookie__:<token>` from the default litecoind mainnet data directory
/// (macOS Application Support, then Linux `~/.litecoin`).
fn read_default_cookie() -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let home = std::path::PathBuf::from(home);
    let candidates = [
        home.join("Library/Application Support/Litecoin/.cookie"),
        home.join(".litecoin/.cookie"),
    ];
    candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
