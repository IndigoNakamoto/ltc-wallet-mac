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

    /// Default Electrum-LTC endpoints in fallback order (first = default for new wallets).
    ///
    /// Public community servers come and go; connection code should try these in order
    /// rather than depending on any single one.
    pub fn default_electrum_urls(self) -> &'static [&'static str] {
        match self {
            Self::Mainnet => &[
                "ssl://backup.electrum-ltc.org:443",
                "ssl://electrum.ltc.xurious.com:50002",
                "ssl://electrum-ltc.bysh.me:50002",
            ],
            Self::Testnet => &[
                "ssl://electrum.ltc.xurious.com:51002",
                "ssl://electrum-ltc.bysh.me:51002",
            ],
        }
    }

    /// Default Electrum-LTC endpoint for this network.
    pub fn default_electrum_url(self) -> &'static str {
        self.default_electrum_urls()[0]
    }

    /// Port litecoind listens on for P2P, where LIP-0006 MWEB data is served.
    pub fn p2p_port(self) -> u16 {
        match self {
            Self::Mainnet => 9333,
            Self::Testnet => 19335,
        }
    }

    /// Litecoin Core's DNS seeds, used to find public MWEB-serving peers for
    /// users who do not run their own node (see `crate::discovery`).
    pub fn dns_seeds(self) -> &'static [&'static str] {
        match self {
            Self::Mainnet => &[
                "seed-a.litecoin.loshan.co.uk",
                "dnsseed.thrasher.io",
                "dnsseed.litecointools.com",
                "dnsseed.litecoinpool.org",
                "dnsseed.koin-project.com",
            ],
            Self::Testnet => &[
                "testnet-seed.litecointools.com",
                "seed-b.litecoin.loshan.co.uk",
                "dnsseed-testnet.thrasher.io",
            ],
        }
    }
}
