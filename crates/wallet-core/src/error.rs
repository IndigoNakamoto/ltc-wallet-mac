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

    #[error("wallet is locked; unlock with passphrase first")]
    Locked,

    #[error("incorrect passphrase")]
    IncorrectPassphrase,

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

    /// Wallet DB exists but the mnemonic secret is missing (orphaned data).
    #[error(
        "mnemonic missing from secret store; reset wallet data and restore from your backup phrase"
    )]
    MissingMnemonic,

    #[error("metadata error: {0}")]
    Meta(String),

    #[error("electrum error: {0}")]
    Electrum(String),

    #[error("failed to build transaction: {0}")]
    BuildTx(String),

    #[error("failed to sign transaction: {0}")]
    Sign(String),

    #[error("mweb error: {0}")]
    Mweb(String),

    #[error("litecoin rpc error: {0}")]
    Rpc(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Best-effort translation of a raw broadcast rejection (from an Electrum
/// server, litecoind RPC, or a P2P peer) into a message a person can act on.
///
/// Unknown rejections pass through cleaned but unchanged, so no information is
/// lost; known ones get a plain-language explanation with the server's own
/// words kept as a suffix.
pub(crate) fn humanize_broadcast_error(raw: &str) -> String {
    let cleaned = clean_server_message(raw);
    let lower = cleaned.to_lowercase();

    let friendly = if lower.contains("decode failed") || lower.contains("deserialization") {
        Some(
            "the server could not read this transaction. Transactions involving MWEB \
             (peg-in, peg-out, private send) carry extra data that Electrum servers and \
             older nodes do not understand — configure a Litecoin RPC URL or MWEB P2P \
             peers in Settings and try again",
        )
    } else if lower.contains("insufficient fee")
        || lower.contains("min relay fee")
        || lower.contains("mempool min fee")
        || lower.contains("fee not met")
    {
        Some("the network rejected this transaction because its fee is too low — increase the fee and try again")
    } else if lower.contains("dust") {
        Some("one of the amounts is too small for the network to accept (below the dust limit) — send a larger amount")
    } else if lower.contains("missingorspent")
        || lower.contains("missing inputs")
        || lower.contains("bad-txns-inputs")
    {
        Some(
            "the coins this transaction spends are unknown to the network — they may \
             already be spent or not yet confirmed. Sync the wallet and try again",
        )
    } else if lower.contains("txn-mempool-conflict") {
        Some(
            "this transaction conflicts with another unconfirmed transaction spending \
             the same coins — wait for that one to confirm, then sync and try again",
        )
    } else if lower.contains("already in block chain")
        || lower.contains("already known")
        || lower.contains("txn-already")
    {
        Some("this transaction was already broadcast — sync the wallet to see it")
    } else if lower.contains("script-verify") || lower.contains("non-final") {
        Some("the network rejected this transaction as invalid (signature or timelock check failed)")
    } else {
        None
    };

    match friendly {
        Some(f) => format!("{f} (server said: {cleaned})"),
        None => cleaned,
    }
}

/// Strip machine noise from a server rejection: unwrap the `message` field of
/// an embedded JSON-RPC error object, drop raw transaction hex dumps, and
/// collapse whitespace.
fn clean_server_message(raw: &str) -> String {
    let mut msg = raw.to_string();
    if let Some(start) = raw.find('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw[start..]) {
            if let Some(m) = v.get("message").and_then(|m| m.as_str()) {
                msg = m.to_string();
            }
        }
    }
    // ElectrumX appends the full raw tx as "\n[02000000...]"; useless to a person.
    if let Some(pos) = msg.find("\n[") {
        msg.truncate(pos);
    }
    msg.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanizes_electrumx_decode_failure() {
        let raw = r#"Electrum server error: {"code":1,"message":"the transaction was rejected by network rules.\n\nTX decode failed\n[020000000009018efa]"}"#;
        let msg = humanize_broadcast_error(raw);
        assert!(msg.contains("could not read this transaction"), "{msg}");
        assert!(msg.contains("MWEB"), "{msg}");
        assert!(!msg.contains("020000000009"), "raw hex must be stripped: {msg}");
    }

    #[test]
    fn humanizes_low_fee() {
        let msg = humanize_broadcast_error("min relay fee not met, 100 < 1000");
        assert!(msg.contains("fee is too low"), "{msg}");
        assert!(msg.contains("min relay fee not met"), "{msg}");
    }

    #[test]
    fn unknown_errors_pass_through_cleaned() {
        let msg = humanize_broadcast_error("something   odd\nhappened");
        assert_eq!(msg, "something odd happened");
    }
}
