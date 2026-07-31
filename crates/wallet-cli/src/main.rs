use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use wallet_core::{
    CreateWalletRequest, RestoreWalletRequest, SendRequest, WalletApp, WalletNetwork,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliNetwork {
    Mainnet,
    Testnet,
}

impl std::fmt::Display for CliNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mainnet => write!(f, "mainnet"),
            Self::Testnet => write!(f, "testnet"),
        }
    }
}

impl From<CliNetwork> for WalletNetwork {
    fn from(value: CliNetwork) -> Self {
        match value {
            CliNetwork::Mainnet => WalletNetwork::Mainnet,
            CliNetwork::Testnet => WalletNetwork::Testnet,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "wallet-cli",
    about = "Litecoin wallet-core smoke CLI (Electrum testnet)"
)]
struct Cli {
    /// Wallet data directory (sqlite + meta).
    #[arg(long, global = true, default_value = ".wallet-data")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create a new BIP84 wallet; prints mnemonic once.
    Create {
        #[arg(long, value_enum, default_value_t = CliNetwork::Testnet)]
        network: CliNetwork,
        #[arg(long)]
        electrum: Option<String>,
    },
    /// Restore a wallet from a mnemonic.
    Restore {
        #[arg(long)]
        mnemonic: String,
        #[arg(long, value_enum, default_value_t = CliNetwork::Testnet)]
        network: CliNetwork,
        #[arg(long)]
        electrum: Option<String>,
    },
    /// Print wallet summary JSON.
    Summary,
    /// Print the current unused receive address.
    Address,
    /// Sync against Electrum (full_scan first, then incremental).
    Sync,
    /// Build, sign, and broadcast a transaction.
    Send {
        #[arg(long)]
        address: String,
        #[arg(long)]
        amount_sats: u64,
        #[arg(long, default_value_t = 1)]
        fee_rate: u64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = &cli.data_dir;
    std::fs::create_dir_all(data_dir).with_context(|| format!("create {}", data_dir.display()))?;
    let app = WalletApp::new(data_dir);

    match cli.command {
        Command::Create { network, electrum } => {
            let resp = app
                .create(
                    data_dir,
                    CreateWalletRequest {
                        network: network.into(),
                        electrum_url: electrum,
                    },
                )
                .context("create wallet")?;
            eprintln!("mnemonic (backup once): {}", resp.mnemonic);
            println!("{}", serde_json::to_string_pretty(&resp.summary)?);
        }
        Command::Restore {
            mnemonic,
            network,
            electrum,
        } => {
            let summary = app
                .restore(
                    data_dir,
                    RestoreWalletRequest {
                        mnemonic,
                        network: network.into(),
                        electrum_url: electrum,
                    },
                )
                .context("restore wallet")?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::Summary => {
            ensure_loaded(&app, data_dir)?;
            let summary = app.summary().context("summary")?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::Address => {
            ensure_loaded(&app, data_dir)?;
            let address = app.receive_address().context("receive address")?;
            println!("{address}");
        }
        Command::Sync => {
            ensure_loaded(&app, data_dir)?;
            let result = app.sync().context("sync")?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::Send {
            address,
            amount_sats,
            fee_rate,
        } => {
            ensure_loaded(&app, data_dir)?;
            let result = app
                .send(SendRequest {
                    address,
                    amount_sats,
                    fee_rate_sat_vb: fee_rate,
                })
                .context("send")?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }

    Ok(())
}

fn ensure_loaded(app: &WalletApp, data_dir: &Path) -> Result<()> {
    if !app.exists(data_dir) {
        bail!("no wallet at {}", data_dir.display());
    }
    app.load(data_dir).context("load wallet")?;
    Ok(())
}
