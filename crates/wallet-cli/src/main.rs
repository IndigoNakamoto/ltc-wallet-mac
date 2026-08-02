use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use wallet_core::{
    CreateWalletRequest, RestoreWalletRequest, SendRequest, UnlockRequest, WalletApp, WalletNetwork,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliNetwork {
    Mainnet,
    Testnet,
}

impl From<CliNetwork> for WalletNetwork {
    fn from(value: CliNetwork) -> Self {
        match value {
            CliNetwork::Mainnet => WalletNetwork::Mainnet,
            CliNetwork::Testnet => WalletNetwork::Testnet,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum CliMwebScheme {
    /// Litecoin Core: m/0'/100'/{0,1}'
    #[default]
    LitecoinCore,
    /// LIP-0004: m/1/0/{100',101'}
    Lip0004,
    /// mwebd / Nexus: m/1000'/2'/0'/{0,1}'
    Mwebd,
}

impl From<CliMwebScheme> for wallet_core::MwebScheme {
    fn from(value: CliMwebScheme) -> Self {
        match value {
            CliMwebScheme::LitecoinCore => Self::LitecoinCore,
            CliMwebScheme::Lip0004 => Self::Lip0004,
            CliMwebScheme::Mwebd => Self::Mwebd,
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "wallet-cli",
    about = "Litecoin wallet-core smoke CLI (Electrum mainnet by default)"
)]
struct Cli {
    /// Wallet data directory (sqlite + meta + encrypted mnemonic).
    #[arg(long, global = true, default_value = ".wallet-data")]
    data_dir: PathBuf,

    /// Passphrase for the encrypted mnemonic (prompted if omitted).
    #[arg(long, global = true)]
    passphrase: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create a new BIP84 wallet; prints mnemonic once.
    Create {
        #[arg(long, value_enum, default_value_t = CliNetwork::Mainnet)]
        network: CliNetwork,
        #[arg(long)]
        electrum: Option<String>,
    },
    /// Restore a wallet from a seed (BIP39/aezeed words or a root xprv/zprv).
    Restore {
        #[arg(long)]
        mnemonic: String,
        #[arg(long, value_enum, default_value_t = CliNetwork::Mainnet)]
        network: CliNetwork,
        #[arg(long)]
        electrum: Option<String>,
        /// MWEB key-derivation scheme (use mwebd for Nexus seeds).
        #[arg(long, value_enum, default_value_t = CliMwebScheme::LitecoinCore)]
        mweb_scheme: CliMwebScheme,
        /// aezeed cipher-seed passphrase, if one was set.
        #[arg(long)]
        aezeed_passphrase: Option<String>,
    },
    /// Print wallet summary JSON.
    Summary,
    /// Reveal and print a new receive address.
    Address,
    /// Sync against Electrum (full_scan first, then incremental).
    Sync,
    /// Build, sign, and broadcast a transaction.
    Send {
        #[arg(long)]
        address: String,
        #[arg(long)]
        amount_sats: Option<u64>,
        #[arg(long, default_value_t = 1)]
        fee_rate: u64,
        #[arg(long)]
        drain: bool,
    },
    /// List recent transactions.
    History,
    /// Print the addresses a seed would derive (BIP84 + MWEB under every
    /// scheme) without creating a wallet. For cross-wallet parity checks.
    Derive {
        /// Seed input: BIP39 words, aezeed words, or a root xprv/zprv/Ltpv.
        /// Read from stdin when omitted (avoids shell history).
        #[arg(long)]
        input: Option<String>,
        #[arg(long, value_enum, default_value_t = CliNetwork::Mainnet)]
        network: CliNetwork,
        /// Number of addresses to derive per chain.
        #[arg(long, default_value_t = 5)]
        count: u32,
        /// aezeed cipher-seed passphrase, if one was set (Nexus advanced setting).
        #[arg(long)]
        aezeed_passphrase: Option<String>,
    },
}

fn read_passphrase(explicit: &Option<String>) -> Result<String> {
    if let Some(p) = explicit {
        return Ok(p.clone());
    }
    if let Ok(p) = std::env::var("WALLET_PASSPHRASE") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    let p = rpassword::prompt_password("Passphrase: ").context("read passphrase")?;
    if p.is_empty() {
        bail!("passphrase must not be empty");
    }
    Ok(p)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = cli.data_dir.clone();
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("create {}", data_dir.display()))?;
    let app = WalletApp::new(&data_dir);
    let passphrase_opt = cli.passphrase.clone();

    match cli.command {
        Command::Create { network, electrum } => {
            let passphrase = read_passphrase(&passphrase_opt)?;
            let resp = app
                .create(
                    &data_dir,
                    CreateWalletRequest {
                        network: network.into(),
                        electrum_url: electrum,
                    },
                    &passphrase,
                )
                .context("create wallet")?;
            eprintln!("mnemonic (backup once): {}", resp.mnemonic);
            println!("{}", serde_json::to_string_pretty(&resp.summary)?);
        }
        Command::Restore {
            mnemonic,
            network,
            electrum,
            mweb_scheme,
            aezeed_passphrase,
        } => {
            let passphrase = read_passphrase(&passphrase_opt)?;
            let summary = app
                .restore(
                    &data_dir,
                    RestoreWalletRequest {
                        mnemonic,
                        network: network.into(),
                        electrum_url: electrum,
                        mweb_scheme: mweb_scheme.into(),
                        aezeed_passphrase,
                    },
                    &passphrase,
                )
                .context("restore wallet")?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::Summary => {
            ensure_loaded(&app, &data_dir, &passphrase_opt)?;
            let summary = app.summary().context("summary")?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::Address => {
            ensure_loaded(&app, &data_dir, &passphrase_opt)?;
            let address = app.receive_address().context("receive address")?;
            println!("{address}");
        }
        Command::Sync => {
            ensure_loaded(&app, &data_dir, &passphrase_opt)?;
            let result = app.sync().context("sync")?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::Send {
            address,
            amount_sats,
            fee_rate,
            drain,
        } => {
            ensure_loaded(&app, &data_dir, &passphrase_opt)?;
            if !drain && amount_sats.is_none() {
                bail!("--amount-sats required unless --drain");
            }
            let result = app
                .send(SendRequest {
                    address,
                    amount_sats: amount_sats.unwrap_or(0),
                    fee_rate_sat_vb: fee_rate,
                    drain,
                })
                .context("send")?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::History => {
            ensure_loaded(&app, &data_dir, &passphrase_opt)?;
            let txs = app.transactions().context("history")?;
            println!("{}", serde_json::to_string_pretty(&txs)?);
        }
        Command::Derive {
            input,
            network,
            count,
            aezeed_passphrase,
        } => {
            let input = match input {
                Some(i) => i,
                None => {
                    eprintln!("Enter seed (words or extended key), then press Enter:");
                    let mut line = String::new();
                    std::io::stdin()
                        .read_line(&mut line)
                        .context("read seed from stdin")?;
                    line
                }
            };
            let preview = wallet_core::derive_preview(
                &input,
                aezeed_passphrase.as_deref(),
                network.into(),
                count,
            )
            .context("derive preview")?;
            println!("{}", serde_json::to_string_pretty(&preview)?);
        }
    }

    Ok(())
}

fn ensure_loaded(
    app: &WalletApp,
    data_dir: &Path,
    passphrase_opt: &Option<String>,
) -> Result<()> {
    if !app.exists(data_dir) {
        bail!("no wallet at {}", data_dir.display());
    }
    if app.needs_migration() {
        let passphrase = read_passphrase(passphrase_opt)?;
        app.migrate_encrypt(wallet_core::MigrateEncryptRequest { passphrase })
            .context("migrate encrypt")?;
    } else if app.is_locked() {
        let passphrase = read_passphrase(passphrase_opt)?;
        app.unlock(UnlockRequest { passphrase })
            .context("unlock")?;
    }
    app.load(data_dir).context("load wallet")?;
    Ok(())
}
