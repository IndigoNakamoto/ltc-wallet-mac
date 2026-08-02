use std::sync::Arc;

use tempfile::tempdir;
use wallet_core::{
    CreateWalletRequest, MemoryBackedApp, MemoryStore, RestoreWalletRequest, SecretStore,
    SendRequest, WalletError, WalletNetwork,
};

fn with_secrets(secrets: Arc<dyn SecretStore>) -> MemoryBackedApp {
    wallet_core::WalletApp::with_secrets(secrets)
}

#[test]
fn create_testnet_returns_tltc_address_and_mnemonic() {
    let dir = tempdir().unwrap();
    let app = with_secrets(Arc::new(MemoryStore::new()));

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
    let secrets: Arc<dyn SecretStore> = Arc::new(MemoryStore::new());

    let created = {
        let app = with_secrets(Arc::clone(&secrets));
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
        let app = with_secrets(secrets);
        app.load(dir.path()).expect("load")
    };

    assert_eq!(loaded.network, WalletNetwork::Testnet);
    assert_eq!(loaded.receive_address, created.summary.receive_address);
    assert_eq!(loaded.total_sats, 0);
}

#[test]
fn second_create_fails_already_exists() {
    let dir = tempdir().unwrap();
    let app = with_secrets(Arc::new(MemoryStore::new()));
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
    let mnemonic =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();

    let summary_a = {
        let app = with_secrets(Arc::new(MemoryStore::new()));
        app.restore(
            dir_a.path(),
            RestoreWalletRequest {
                mnemonic: mnemonic.into(),
                network: WalletNetwork::Testnet,
                electrum_url: None,
                mweb_scheme: Default::default(),
                aezeed_passphrase: None,
            },
        )
        .expect("restore a")
    };

    let summary_b = {
        let app = with_secrets(Arc::new(MemoryStore::new()));
        app.restore(
            dir_b.path(),
            RestoreWalletRequest {
                mnemonic: mnemonic.into(),
                network: WalletNetwork::Testnet,
                electrum_url: None,
                mweb_scheme: Default::default(),
                aezeed_passphrase: None,
            },
        )
        .expect("restore b")
    };

    assert!(summary_a.receive_address.starts_with("tltc1"));
    assert_eq!(summary_a.receive_address, summary_b.receive_address);
}

#[test]
fn receive_address_advances_and_summary_keeps_it() {
    let dir = tempdir().unwrap();
    let secrets: Arc<dyn SecretStore> = Arc::new(MemoryStore::new());
    let app = with_secrets(Arc::clone(&secrets));
    let created = app
        .create(
            dir.path(),
            CreateWalletRequest {
                network: WalletNetwork::Testnet,
                electrum_url: None,
            },
        )
        .expect("create");

    let first = created.summary.receive_address;
    let second = app.receive_address().expect("new address");
    let third = app.receive_address().expect("another address");

    assert_ne!(first, second);
    assert_ne!(second, third);
    assert!(second.starts_with("tltc1"));
    assert_eq!(app.summary().expect("summary").receive_address, third);

    let reloaded = {
        let app = with_secrets(secrets);
        app.load(dir.path()).expect("load")
    };
    assert_eq!(reloaded.receive_address, third);
}

#[test]
fn send_rejects_invalid_address() {
    let dir = tempdir().unwrap();
    let app = with_secrets(Arc::new(MemoryStore::new()));
    app.create(
        dir.path(),
        CreateWalletRequest {
            network: WalletNetwork::Testnet,
            electrum_url: None,
        },
    )
    .unwrap();

    let err = app
        .send(SendRequest {
            address: "not-an-address".into(),
            amount_sats: 1000,
            fee_rate_sat_vb: Some(1),
            drain: false,
        })
        .expect_err("invalid address");
    assert!(matches!(err, WalletError::InvalidAddress(_)));
}

#[test]
fn create_after_orphaned_db_wipes_and_succeeds() {
    let dir = tempdir().unwrap();
    let secrets = Arc::new(MemoryStore::new());
    let app = with_secrets(Arc::clone(&secrets) as Arc<dyn SecretStore>);
    app.create(
        dir.path(),
        CreateWalletRequest {
            network: WalletNetwork::Testnet,
            electrum_url: None,
        },
    )
    .unwrap();

    secrets.delete_mnemonic().unwrap();
    assert!(app.exists(dir.path()));
    assert!(matches!(
        app.load(dir.path()).unwrap_err(),
        WalletError::MissingMnemonic
    ));

    let app2 = with_secrets(secrets);
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
    let app = with_secrets(Arc::new(MemoryStore::new()));
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
    let store2 = FileSecretStore::new(&path);
    assert_eq!(store2.get_mnemonic().unwrap().as_deref(), Some(phrase));
    store2.delete_mnemonic().expect("delete");
    assert_eq!(store2.get_mnemonic().unwrap(), None);
}
