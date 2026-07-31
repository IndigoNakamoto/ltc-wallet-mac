use std::sync::Arc;

use tempfile::tempdir;
use wallet_core::{
    CreateWalletRequest, MemoryStore, RestoreWalletRequest, SecretStore, WalletApp, WalletError,
    WalletNetwork,
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
fn send_rejects_invalid_address() {
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

    let err = app
        .send(wallet_core::SendRequest {
            address: "not-an-address".into(),
            amount_sats: 1000,
            fee_rate_sat_vb: 1,
        })
        .expect_err("invalid address");
    assert!(matches!(err, WalletError::InvalidAddress(_)));
}

#[test]
fn create_after_orphaned_db_wipes_and_succeeds() {
    let dir = tempdir().unwrap();
    let secrets = Arc::new(MemoryStore::new());
    let app = WalletApp::with_secrets(Arc::clone(&secrets) as Arc<dyn wallet_core::SecretStore>);
    app.create(
        dir.path(),
        CreateWalletRequest {
            network: WalletNetwork::Testnet,
            electrum_url: None,
        },
    )
    .unwrap();

    // Simulate lost mnemonic secret while DB remains.
    secrets.delete_mnemonic().unwrap();
    assert!(app.exists(dir.path()));
    assert!(matches!(
        app.load(dir.path()).unwrap_err(),
        WalletError::MissingMnemonic
    ));

    let app2 = WalletApp::with_secrets(secrets);
    let resp = app2
        .create(
            dir.path(),
            CreateWalletRequest {
                network: WalletNetwork::Testnet,
                electrum_url: None,
            },
        )
        .expect("create after orphan wipe");
    assert!(resp.summary.receive_address.starts_with("tltc1"));
}

#[test]
fn create_marks_needs_full_scan_in_meta() {
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

    let meta_path = dir.path().join("wallet_meta.json");
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(meta_path).unwrap()).unwrap();
    assert_eq!(meta["needs_full_scan"], true);
}

#[test]
fn file_secret_store_roundtrip() {
    use wallet_core::FileSecretStore;
    let dir = tempdir().unwrap();
    let path = dir.path().join("wallet.mnemonic");
    let store = FileSecretStore::new(&path);
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    store.set_mnemonic(phrase).expect("set");
    let got = store.get_mnemonic().expect("get");
    assert_eq!(got.as_deref(), Some(phrase));
    // Fresh store instance must still read the file (process-restart equivalent).
    let store2 = FileSecretStore::new(&path);
    assert_eq!(store2.get_mnemonic().unwrap().as_deref(), Some(phrase));
    store2.delete_mnemonic().expect("delete");
    assert_eq!(store2.get_mnemonic().unwrap(), None);
}
