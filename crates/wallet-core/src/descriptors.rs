use bdk_wallet::bitcoin::bip32::Xpriv;
use bdk_wallet::bitcoin::Network;
use bdk_wallet::keys::bip39::{Language, Mnemonic, WordCount};
use bdk_wallet::keys::{GeneratableKey, GeneratedKey};
use bdk_wallet::miniscript::Segwitv0;
use bdk_wallet::template::Bip84;
use bdk_wallet::{CreateParams, KeychainKind, LoadParams, Wallet};

use crate::error::WalletError;
use crate::network::WalletNetwork;

/// Generate a new 12-word English mnemonic.
pub fn generate_mnemonic() -> Result<String, WalletError> {
    let generated: GeneratedKey<_, Segwitv0> =
        Mnemonic::generate((WordCount::Words12, Language::English))
            .map_err(|e| WalletError::InvalidMnemonic(format!("{e:?}")))?;
    Ok(generated.into_key().to_string())
}

/// Parse a mnemonic phrase.
pub fn parse_mnemonic(phrase: &str) -> Result<Mnemonic, WalletError> {
    Mnemonic::parse_in(Language::English, phrase.trim())
        .map_err(|e| WalletError::InvalidMnemonic(e.to_string()))
}

/// Derive the master `Xpriv` from a mnemonic (empty passphrase).
fn master_xprv(mnemonic: &Mnemonic, network: Network) -> Result<Xpriv, WalletError> {
    let seed = mnemonic.to_seed("");
    Xpriv::new_master(network, &seed).map_err(|e| WalletError::Descriptor(e.to_string()))
}

/// Build [`CreateParams`] for a BIP84 wallet from a mnemonic.
pub fn create_params(
    mnemonic: &Mnemonic,
    network: WalletNetwork,
) -> Result<CreateParams, WalletError> {
    let bdk_network = network.to_bitcoin_network();
    let xprv = master_xprv(mnemonic, bdk_network)?;
    Ok(Wallet::create(
        Bip84(xprv, KeychainKind::External),
        Bip84(xprv, KeychainKind::Internal),
    )
    .network(bdk_network))
}

/// Build [`LoadParams`] that check BIP84 descriptors and extract signing keys.
pub fn load_params(
    mnemonic: &Mnemonic,
    network: WalletNetwork,
) -> Result<LoadParams, WalletError> {
    let bdk_network = network.to_bitcoin_network();
    let xprv = master_xprv(mnemonic, bdk_network)?;
    Ok(Wallet::load()
        .descriptor(
            KeychainKind::External,
            Some(Bip84(xprv, KeychainKind::External)),
        )
        .descriptor(
            KeychainKind::Internal,
            Some(Bip84(xprv, KeychainKind::Internal)),
        )
        .extract_keys()
        .check_network(bdk_network))
}
