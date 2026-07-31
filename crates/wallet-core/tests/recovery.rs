//! Seed-recovery tests: aezeed (Nexus) and extended-key imports, MWEB
//! derivation schemes, and frozen derivation vectors.

use std::sync::Arc;

use tempfile::tempdir;
use wallet_core::{
    derive_preview, MemoryBackedApp, MemoryStore, MwebScheme, RestoreWalletRequest, SecretStore,
    WalletApp, WalletNetwork,
};

fn with_secrets(secrets: Arc<dyn SecretStore>) -> MemoryBackedApp {
    WalletApp::with_secrets(secrets)
}

fn restore_req(input: &str, scheme: MwebScheme) -> RestoreWalletRequest {
    RestoreWalletRequest {
        mnemonic: input.into(),
        network: WalletNetwork::Testnet,
        electrum_url: None,
        mweb_scheme: scheme,
        aezeed_passphrase: None,
    }
}

/// Standard BIP39 test mnemonic (public knowledge, zero-value).
const BIP39_TEST: &str = "abandon abandon abandon abandon abandon abandon abandon abandon \
     abandon abandon abandon about";

/// aezeed known-answer vector: entropy 81b637d8…4cfd, salt "salt1", birthday 0,
/// enciphered with lnd's production scrypt parameters (see src/aezeed.rs).
const AEZEED_WORDS: &str = "above judge emerge veteran reform crunch system all snap please \
     shoulder vault hurt city quarter cover enlist swear success suggest drink wagon enrich body";

/// BIP32 test vector 1 root key, re-encoded with SLIP-132 zprv version bytes.
const ROOT_ZPRV: &str = "zprvAWgYBBk7JR8GjzqSzmunMCS7dAbwpYTCs1YUMDXqduMA5JFHZ3iX5s2UkAR6vBdcCYYa1S5o1fVLrKsrnpCQ4WpUd6aVUWP1bS2Yy5DoaKv";
/// Its m/0' child (depth 1) as zprv — must be rejected for MWEB recovery.
const CHILD_ZPRV: &str = "zprvAYwxAu3aPgFFtbyACXnREEYDwzAJCcwDYcyNKJuxc6YSSaYTyHFcD1NtXRmSKu1ZubSGNjQfQ2LDa5uaSQUaratzgyFdiU5uJSfQQEgCdm3";

/// Frozen derivation vectors for the standard test mnemonic on mainnet.
///
/// The litecoin-core MWEB addresses are cross-validated against ltcd/Core in
/// bdk_mweb's key tests; the mwebd addresses implement the derivation path
/// documented in mwebd's README (`m/1000'/2'/0'/{0,1}'`) and freeze current
/// behavior until byte-for-byte confirmation against a live Nexus wallet.
#[test]
fn frozen_derivation_vectors_all_schemes() {
    let p = derive_preview(BIP39_TEST, None, WalletNetwork::Mainnet, 2).unwrap();
    assert_eq!(p.kind, "BIP39 mnemonic");
    assert_eq!(p.master_fingerprint, "73c5da0a");
    assert_eq!(p.depth, 0);
    assert_eq!(
        p.bip84_external,
        vec![
            "ltc1qjmxnz78nmc8nq77wuxh25n2es7rzm5c2rkk4wh",
            "ltc1qwlezpr3890hcp6vva9twqh27mr6edadreqvhnn",
        ]
    );
    assert_eq!(
        p.bip84_internal[0],
        "ltc1qyeljcy9v88jg8sqvnqh0m5q390xruc5r98q9yy"
    );

    let by_scheme = |name: &str| p.mweb.iter().find(|s| s.scheme == name).unwrap();

    let core = by_scheme("litecoin-core");
    assert_eq!(core.scan_path, "m/0'/100'/0'");
    assert_eq!(
        core.addresses[0],
        "ltcmweb1qqv2g556ddyyqxvr25avn9vsepu75fnygr92pwxewl72ua26su57nsqu5r89cpckmja4m7mc3tm3jremaddk8637697afaswaee78z3ztyy8sevgm"
    );

    let mwebd = by_scheme("mwebd");
    assert_eq!(mwebd.scan_path, "m/1000'/2'/0'/0'");
    assert_eq!(mwebd.spend_path, "m/1000'/2'/0'/1'");
    assert_eq!(
        mwebd.addresses[0],
        "ltcmweb1qqw8xawh8m5qrkdaw3sx3mhycrraqt9djvrnhf7yxtfykuhl7nuanvqa6909hkvx8vgqlx82c59chdn0uuzlmt3pl6yk59dafqpqavdrnlgzu996x"
    );

    // The three schemes must disagree — a regression where they collapse to
    // one path would silently scan the wrong keyspace.
    let lip = by_scheme("lip-0004");
    assert_ne!(core.addresses[0], mwebd.addresses[0]);
    assert_ne!(core.addresses[0], lip.addresses[0]);
    assert_ne!(mwebd.addresses[0], lip.addresses[0]);
}

#[test]
fn restore_aezeed_matches_preview_and_reloads() {
    let preview = derive_preview(AEZEED_WORDS, None, WalletNetwork::Testnet, 1).unwrap();
    assert_eq!(preview.kind, "aezeed seed");
    // Birthday 0 = lnd's Bitcoin genesis epoch.
    assert_eq!(preview.birthday_unix, Some(1_231_006_505));

    let dir = tempdir().unwrap();
    let secrets: Arc<dyn SecretStore> = Arc::new(MemoryStore::new());

    let restored = {
        let app = with_secrets(Arc::clone(&secrets));
        app.restore(dir.path(), restore_req(AEZEED_WORDS, MwebScheme::Mwebd))
            .expect("restore aezeed")
    };
    assert_eq!(restored.receive_address, preview.bip84_external[0]);
    assert!(restored.receive_address.starts_with("tltc1"));

    // The stored payload is tagged (`aezeed:`), and a fresh app instance must
    // reload the identical wallet from it.
    let stored = secrets.get_mnemonic().unwrap().unwrap();
    assert!(stored.starts_with("aezeed:"), "stored payload: {stored}");
    let loaded = {
        let app = with_secrets(secrets);
        app.load(dir.path()).expect("reload aezeed wallet")
    };
    assert_eq!(loaded.receive_address, restored.receive_address);
}

#[test]
fn restore_root_zprv_matches_preview_and_reloads() {
    let preview = derive_preview(ROOT_ZPRV, None, WalletNetwork::Testnet, 1).unwrap();
    assert_eq!(preview.kind, "extended private key");
    assert_eq!(preview.depth, 0);
    // BIP32 test vector 1 master fingerprint.
    assert_eq!(preview.master_fingerprint, "3442193e");

    let dir = tempdir().unwrap();
    let secrets: Arc<dyn SecretStore> = Arc::new(MemoryStore::new());

    let restored = {
        let app = with_secrets(Arc::clone(&secrets));
        app.restore(dir.path(), restore_req(ROOT_ZPRV, MwebScheme::default()))
            .expect("restore zprv")
    };
    assert_eq!(restored.receive_address, preview.bip84_external[0]);

    let stored = secrets.get_mnemonic().unwrap().unwrap();
    assert!(stored.starts_with("xprv:"), "stored payload: {stored}");
    let loaded = {
        let app = with_secrets(secrets);
        app.load(dir.path()).expect("reload zprv wallet")
    };
    assert_eq!(loaded.receive_address, restored.receive_address);
}

#[test]
fn account_level_zprv_is_rejected_with_clear_error() {
    let dir = tempdir().unwrap();
    let app = with_secrets(Arc::new(MemoryStore::new()));
    let err = app
        .restore(dir.path(), restore_req(CHILD_ZPRV, MwebScheme::default()))
        .expect_err("account-level key must be rejected");
    let msg = err.to_string();
    assert!(msg.contains("depth 1"), "{msg}");
    assert!(msg.contains("root"), "{msg}");
    assert!(!app.exists(dir.path()), "no wallet files should be left");
}

/// End-to-end through the production `WalletApp` (encrypted secret store and
/// live MWEB runtime): the restore scheme decides which MWEB addresses the
/// wallet hands out, and it survives a lock/reload cycle.
#[test]
fn mweb_scheme_selects_receive_addresses_end_to_end() {
    let preview = derive_preview(AEZEED_WORDS, None, WalletNetwork::Testnet, 1).unwrap();
    let mwebd_addr0 = &preview
        .mweb
        .iter()
        .find(|s| s.scheme == "mwebd")
        .unwrap()
        .addresses[0];
    let core_addr0 = &preview
        .mweb
        .iter()
        .find(|s| s.scheme == "litecoin-core")
        .unwrap()
        .addresses[0];

    let dir = tempdir().unwrap();
    let app = WalletApp::new(dir.path());
    app.restore(
        dir.path(),
        restore_req(AEZEED_WORDS, MwebScheme::Mwebd),
        "correct horse battery staple",
    )
    .expect("restore under mwebd scheme");

    let addr = app.mweb_receive_address().expect("mweb receive address");
    assert_eq!(&addr, mwebd_addr0, "wallet must derive under mwebd");
    assert_ne!(&addr, core_addr0);

    // Reload from disk: scheme comes back from wallet_meta.json.
    app.lock();
    app.unlock(wallet_core::UnlockRequest {
        passphrase: "correct horse battery staple".into(),
    })
    .expect("unlock");
    app.load(dir.path()).expect("reload");
    assert_eq!(
        app.mweb_receive_address().expect("address after reload"),
        addr
    );
}
