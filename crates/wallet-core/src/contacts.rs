//! Local address-book contacts (non-secret sidecar next to wallet meta).

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use bdk_wallet::bitcoin::Address;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::dto::{ContactKind, ContactRecord};
use crate::error::WalletError;
use crate::network::WalletNetwork;

pub const CONTACTS_FILE: &str = "contacts.json";
pub const MAX_CONTACT_NAME_CHARS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ContactsFile {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub contacts: Vec<ContactRecord>,
}

fn default_version() -> u32 {
    1
}

pub fn contacts_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CONTACTS_FILE)
}

pub fn normalize_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.chars().count() <= MAX_CONTACT_NAME_CHARS {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_CONTACT_NAME_CHARS).collect()
}

fn new_contact_id() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn is_mweb_stealth_address(address: &str) -> bool {
    let lower = address.trim().to_ascii_lowercase();
    lower.starts_with("ltcmweb") || lower.starts_with("tmweb")
}

fn validate_address(
    address: &str,
    kind: ContactKind,
    network: WalletNetwork,
) -> Result<String, WalletError> {
    let address = address.trim();
    if address.is_empty() {
        return Err(WalletError::InvalidAddress("address required".into()));
    }
    let bitcoin_network = network.to_bitcoin_network();
    match kind {
        ContactKind::Public => {
            if is_mweb_stealth_address(address) {
                return Err(WalletError::InvalidAddress(
                    "public contact cannot use a Private (MWEB) address".into(),
                ));
            }
            let parsed = Address::from_str(address)
                .map_err(|e| WalletError::InvalidAddress(e.to_string()))?
                .require_network(bitcoin_network)
                .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;
            Ok(parsed.to_string())
        }
        ContactKind::Private => {
            if !is_mweb_stealth_address(address) {
                return Err(WalletError::InvalidAddress(
                    "private contact requires an ltcmweb / tmweb stealth address".into(),
                ));
            }
            let parsed = Address::from_str(address)
                .map_err(|e| WalletError::InvalidAddress(e.to_string()))?
                .require_network(bitcoin_network)
                .map_err(|e| WalletError::InvalidAddress(e.to_string()))?;
            Ok(parsed.to_string())
        }
    }
}

pub fn read_contacts(data_dir: &Path) -> Result<ContactsFile, WalletError> {
    let path = contacts_path(data_dir);
    if !path.is_file() {
        return Ok(ContactsFile {
            version: 1,
            contacts: Vec::new(),
        });
    }
    let bytes = fs::read_to_string(&path).map_err(WalletError::Io)?;
    serde_json::from_str(&bytes).map_err(|e| WalletError::Meta(e.to_string()))
}

pub fn write_contacts(data_dir: &Path, file: &ContactsFile) -> Result<(), WalletError> {
    fs::create_dir_all(data_dir)?;
    let json = serde_json::to_string_pretty(file).map_err(|e| WalletError::Meta(e.to_string()))?;
    fs::write(contacts_path(data_dir), json)?;
    Ok(())
}

fn persist_or_remove(data_dir: &Path, file: &ContactsFile) -> Result<(), WalletError> {
    if file.contacts.is_empty() {
        let path = contacts_path(data_dir);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(WalletError::Io(e)),
        }
    } else {
        write_contacts(data_dir, file)
    }
}

/// Insert or replace a contact. Empty `id` allocates a new one.
pub fn upsert_contact(
    data_dir: &Path,
    network: WalletNetwork,
    id: Option<&str>,
    name: &str,
    address: &str,
    kind: ContactKind,
) -> Result<ContactRecord, WalletError> {
    let name = normalize_name(name);
    if name.is_empty() {
        return Err(WalletError::Meta("contact name required".into()));
    }
    let address = validate_address(address, kind, network)?;
    let mut file = read_contacts(data_dir)?;
    let id = id.map(str::trim).filter(|s| !s.is_empty());
    let contact = ContactRecord {
        id: id.map(|s| s.to_string()).unwrap_or_else(new_contact_id),
        name,
        address,
        kind,
    };
    if let Some(existing) = file.contacts.iter_mut().find(|c| c.id == contact.id) {
        *existing = contact.clone();
    } else {
        file.contacts.push(contact.clone());
    }
    file.contacts
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    persist_or_remove(data_dir, &file)?;
    Ok(contact)
}

pub fn delete_contact(data_dir: &Path, id: &str) -> Result<(), WalletError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(WalletError::Meta("contact id required".into()));
    }
    let mut file = read_contacts(data_dir)?;
    let before = file.contacts.len();
    file.contacts.retain(|c| c.id != id);
    if file.contacts.len() == before {
        return Err(WalletError::Meta("contact not found".into()));
    }
    persist_or_remove(data_dir, &file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn name_truncates() {
        let long: String = "n".repeat(80);
        assert_eq!(normalize_name(&long).chars().count(), MAX_CONTACT_NAME_CHARS);
    }

    #[test]
    fn public_reject_mweb_prefix() {
        let dir = tempdir().unwrap();
        let err = upsert_contact(
            dir.path(),
            WalletNetwork::Mainnet,
            None,
            "Alice",
            "ltcmweb1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq2l6h2p",
            ContactKind::Public,
        )
        .unwrap_err();
        assert!(err.to_string().contains("Private"));
    }
}
