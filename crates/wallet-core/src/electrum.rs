use bdk_electrum::electrum_client::{Client, ConfigBuilder};
use bdk_electrum::BdkElectrumClient;

use crate::error::WalletError;

pub const STOP_GAP: usize = 50;
pub const BATCH_SIZE: usize = 5;

/// Connect to an Electrum-LTC server. Public servers often use self-signed certs.
pub fn connect(url: &str) -> Result<BdkElectrumClient<Client>, WalletError> {
    let config = ConfigBuilder::new().validate_domain(false).build();
    let client = Client::from_config(url, config)
        .map_err(|e| WalletError::Electrum(e.to_string()))?;
    Ok(BdkElectrumClient::new(client))
}
