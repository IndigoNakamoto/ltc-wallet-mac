use bdk_electrum::electrum_client::{Client, ConfigBuilder, ElectrumApi};
use bdk_electrum::BdkElectrumClient;

use crate::error::WalletError;

pub const STOP_GAP: usize = 50;
pub const BATCH_SIZE: usize = 5;

/// Concrete client type used across the crate.
pub type ElectrumClient = BdkElectrumClient<Client>;

/// Connect to an Electrum-LTC server. Public servers often use self-signed certs.
pub fn connect(url: &str) -> Result<BdkElectrumClient<Client>, WalletError> {
    // Timeout is seconds (u8); keeps flaky servers from hanging the UI forever.
    let config = ConfigBuilder::new()
        .validate_domain(false)
        .timeout(Some(30))
        .build();
    let client = Client::from_config(url, config).map_err(|e| {
        WalletError::Electrum(format!(
            "failed to connect to {url} (timed out or unreachable): {e}"
        ))
    })?;
    Ok(BdkElectrumClient::new(client))
}

/// Try each candidate URL in order; return the first server that connects *and*
/// answers a ping, along with the URL that worked.
///
/// Public Electrum-LTC servers disappear regularly, so callers should pass the
/// user-configured URL followed by [`crate::WalletNetwork::default_electrum_urls`].
pub fn connect_first(urls: &[String]) -> Result<(BdkElectrumClient<Client>, String), WalletError> {
    let mut errors: Vec<String> = Vec::new();
    for url in urls {
        match connect(url) {
            Ok(client) => match client.inner.ping() {
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
