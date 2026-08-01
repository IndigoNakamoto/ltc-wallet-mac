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
    // Timeout is seconds (u8); keeps flaky servers from hanging the UI forever.
    let config = ConfigBuilder::new()
        .validate_domain(validate_domain)
        .timeout(Some(30))
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
