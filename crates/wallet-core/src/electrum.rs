use bdk_electrum::electrum_client::{Client, ConfigBuilder, ElectrumApi, Param};
use bdk_electrum::BdkElectrumClient;

use crate::error::WalletError;

pub const STOP_GAP: usize = 50;
pub const BATCH_SIZE: usize = 5;

/// Concrete client type used across the crate.
pub type ElectrumClient = BdkElectrumClient<Client>;

/// Connect to an Electrum-LTC server.
///
/// `validate_domain` controls TLS certificate validation for `ssl://` URLs:
/// when true the server must present a CA-signed certificate matching its
/// hostname (protects against man-in-the-middle attacks); when false any
/// certificate is accepted, which many community Electrum-LTC servers with
/// self-signed certificates require.
pub fn connect(url: &str, validate_domain: bool) -> Result<BdkElectrumClient<Client>, WalletError> {
    connect_with_timeout(url, validate_domain, 30)
}

/// [`connect`] with an explicit timeout (seconds). Cross-checks use a short
/// timeout so a dead second server cannot stall the sync it is auditing.
pub fn connect_with_timeout(
    url: &str,
    validate_domain: bool,
    timeout_secs: u8,
) -> Result<BdkElectrumClient<Client>, WalletError> {
    let config = ConfigBuilder::new()
        .validate_domain(validate_domain)
        .timeout(Some(timeout_secs))
        .build();
    let client = Client::from_config(url, config).map_err(|e| {
        let mut msg = format!("failed to connect to {url} (timed out or unreachable): {e}");
        if validate_domain && url.starts_with("ssl://") {
            msg.push_str(
                "; if this server uses a self-signed certificate, disable TLS certificate \
                 validation in Settings (reduces security) or pick a CA-certified server",
            );
        }
        WalletError::Electrum(msg)
    })?;
    Ok(BdkElectrumClient::new(client))
}

/// Try each candidate URL in order; return the first server that connects *and*
/// answers a ping, along with the URL that worked.
///
/// Public Electrum-LTC servers disappear regularly, so callers should pass the
/// user-configured URL followed by [`crate::WalletNetwork::default_electrum_urls`].
pub fn connect_first(
    urls: &[String],
    validate_domain: bool,
) -> Result<(BdkElectrumClient<Client>, String), WalletError> {
    let mut errors: Vec<String> = Vec::new();
    for url in urls {
        match connect(url, validate_domain) {
            Ok(client) => match handshake(&client) {
                Ok(()) => return Ok((client, url.clone())),
                Err(e) => errors.push(format!("{url}: connected but unresponsive ({e})")),
            },
            Err(e) => errors.push(format!("{url}: {e}")),
        }
    }
    Err(WalletError::Electrum(format!(
        "no Electrum server reachable — {}",
        errors.join("; ")
    )))
}

/// How many blocks two servers may differ at the tip before it is suspicious
/// (normal propagation delay, not disagreement).
const TIP_LAG_TOLERANCE: u32 = 2;

/// Cross-check the chain served by `url` against our local chain after a sync.
///
/// This is deliberately privacy-preserving: only block headers are requested,
/// never our scripts, so the second server learns nothing about the wallet.
/// It catches a sync server that is on a different chain or withholding
/// blocks; it cannot catch omission of individual transactions.
///
/// Returns `Ok(None)` when consistent, `Ok(Some(warning))` on disagreement,
/// and `Err` when the second server was unreachable (callers should skip,
/// not alarm).
pub fn cross_check_tip(
    url: &str,
    validate_domain: bool,
    local_tip_height: u32,
    local_hash_at: &dyn Fn(u32) -> Option<bdk_wallet::bitcoin::BlockHash>,
) -> Result<Option<String>, WalletError> {
    let client = connect_with_timeout(url, validate_domain, 10)?;
    handshake(&client).map_err(|e| WalletError::Electrum(e.to_string()))?;
    let sub = client
        .inner
        .block_headers_subscribe()
        .map_err(|e| WalletError::Electrum(e.to_string()))?;
    let their_tip = sub.height as u32;
    if their_tip > local_tip_height.saturating_add(TIP_LAG_TOLERANCE) {
        return Ok(Some(format!(
            "cross-check: independent server {url} is at height {their_tip}, {} blocks ahead of \
             the server used for this sync — that server may be lagging or withholding blocks",
            their_tip - local_tip_height
        )));
    }
    let common = local_tip_height.min(their_tip);
    let Some(local_hash) = local_hash_at(common) else {
        return Ok(None);
    };
    let their_header = client
        .inner
        .block_header(common as usize)
        .map_err(|e| WalletError::Electrum(e.to_string()))?;
    if their_header.block_hash() != local_hash {
        return Ok(Some(format!(
            "WARNING: Electrum servers disagree at height {common}: {url} reports a different \
             block than the server used for this sync. One of them may be dishonest — verify \
             your balance against a block explorer or your own node before transacting"
        )));
    }
    Ok(None)
}

/// Confirm the server actually answers requests. Introduce ourselves with
/// `server.version` first: some ElectrumX deployments refuse every other call
/// until the client identifies itself. Servers that don't care about
/// identification are covered by the plain ping fallback.
fn handshake(client: &ElectrumClient) -> Result<(), bdk_electrum::electrum_client::Error> {
    let version = client.inner.raw_call(
        "server.version",
        vec![Param::String("ltc-wallet".into()), Param::String("1.4".into())],
    );
    match version {
        Ok(_) => Ok(()),
        Err(_) => client.inner.ping(),
    }
}
