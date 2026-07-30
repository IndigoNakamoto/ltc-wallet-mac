use bdk_wallet::bitcoin::Network;
use serde::{Deserialize, Serialize};

/// User-facing network selection for the Litecoin wallet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WalletNetwork {
    Mainnet,
    Testnet,
}

impl WalletNetwork {
    /// Map to the litecoin crate `Network` (aliased as `bitcoin` in BDK).
    ///
    /// Litecoin mainnet = `Network::Bitcoin`, testnet = `Network::Testnet4`.
    pub fn to_bitcoin_network(self) -> Network {
        match self {
            Self::Mainnet => Network::Bitcoin,
            Self::Testnet => Network::Testnet4,
        }
    }

    /// Default Electrum-LTC endpoint for this network.
    pub fn default_electrum_url(self) -> &'static str {
        match self {
            Self::Mainnet => "ssl://electrum-ltc.bysh.me:50002",
            Self::Testnet => "ssl://electrum-ltc.bysh.me:51002",
        }
    }
}
