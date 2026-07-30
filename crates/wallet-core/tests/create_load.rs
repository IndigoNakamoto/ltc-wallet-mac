use std::sync::Arc;

use tempfile::tempdir;
use wallet_core::{
    CreateWalletRequest, MemoryStore, RestoreWalletRequest, WalletApp, WalletError, WalletNetwork,
};

fn test_app() -> WalletApp {
    WalletApp::with_secrets(Arc::new(MemoryStore::new()))
}

#[test]
fn create_testnet_returns_tltc_address_and_mnemonic() {
    let dir = tempdir().unwrap();
    let app = test_app();

    let resp = app
        .create(
            dir.path(),
            CreateWalletRequest {
                network: WalletNetwork::Testnet,
                electrum_url: None,
            },
        )
        .expect("create");

    assert_eq!(resp.mnemonic.split_whitespace().count(), 12);
    assert!(
        resp.summary.receive_address.starts_with("tltc1"),
        "got {}",
        resp.summary.receive_address
    );
    assert_eq!(resp.summary.network, WalletNetwork::Testnet);
    assert_eq!(resp.summary.total_sats, 0);
    assert!(app.exists(dir.path()));
}

#[test]
fn create_then_load_round_trip() {
    let dir = tempdir().unwrap();
    let secrets: Arc<dyn wallet_core::SecretStore> = Arc::new(MemoryStore::new());

    let created = {
        let app = WalletApp::with_secrets(Arc::clone(&secrets));
        app.create(
            dir.path(),
            CreateWalletRequest {
                network: WalletNetwork::Testnet,
                electrum_url: Some("ssl://example.invalid:51002".into()),
            },
        )
        .expect("create")
    };

    let loaded = {
        let app = WalletApp::with_secrets(secrets);
        app.load(dir.path()).expect("load")
    };

    assert_eq!(loaded.network, WalletNetwork::Testnet);
    assert_eq!(loaded.receive_address, created.summary.receive_address);
    assert_eq!(loaded.total_sats, 0);
}

#[test]
fn second_create_fails_already_exists() {
    let dir = tempdir().unwrap();
    let app = test_app();
    let req = CreateWalletRequest {
        network: WalletNetwork::Testnet,
        electrum_url: None,
    };
    app.create(dir.path(), req.clone()).expect("first create");

    let err = app.create(dir.path(), req).expect_err("second create");
    assert!(matches!(err, WalletError::AlreadyExists));
}

#[test]
fn restore_known_mnemonic_is_deterministic() {
    // Same master key as bdk_wallet Bip84 doctest (tprv…AQ5R8L).
    // BIP39 mnemonic that yields a known first address is awkward without a fixture seed;
    // instead restore twice and assert identical receive addresses.
    let mnemonic =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();

    let summary_a = {
        let app = test_app();
        app.restore(
            dir_a.path(),
            RestoreWalletRequest {
                mnemonic: mnemonic.into(),
                network: WalletNetwork::Testnet,
                electrum_url: None,
            },
        )
        .expect("restore a")
    };

    let summary_b = {
        let app = test_app();
        app.restore(
            dir_b.path(),
            RestoreWalletRequest {
                mnemonic: mnemonic.into(),
                network: WalletNetwork::Testnet,
                electrum_url: None,
            },
        )
        .expect("restore b")
    };

    assert!(summary_a.receive_address.starts_with("tltc1"));
    assert_eq!(summary_a.receive_address, summary_b.receive_address);
}

#[test]
fn sync_and_send_are_stubbed() {
    let dir = tempdir().unwrap();
    let app = test_app();
    app.create(
        dir.path(),
        CreateWalletRequest {
            network: WalletNetwork::Testnet,
            electrum_url: None,
        },
    )
    .unwrap();

    assert!(matches!(
        app.sync().unwrap_err(),
        WalletError::NotImplemented("sync")
    ));
    assert!(matches!(
        app.send(wallet_core::SendRequest {
            address: "tltc1qhl85z42h7r4su5u37rvvw0gk8j2t3n9y82jk96".into(),
            amount_sats: 1000,
            fee_rate_sat_vb: 1,
        })
        .unwrap_err(),
        WalletError::NotImplemented("send")
    ));
}
