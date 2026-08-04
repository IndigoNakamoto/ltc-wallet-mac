use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};
use wallet_core::{
    AddressReuseHint, CombinedSummary, CreateWalletRequest, CreateWalletResponse, FeeEstimate,
    FeeLadder, MigrateEncryptRequest, MwebBroadcastResult, MwebSendPreview, MwebSendRequest,
    MwebSyncProgress, PeginPreview, PeginRequest, PeginResult, PegoutPreview, PegoutRequest,
    RestoreWalletRequest, SendPreview, SendRequest, SendResult, SetTxLabelRequest, SyncResult,
    TxEnrichment, TxRecord, UnlockRequest, UpdateSettingsRequest, WalletApp, WalletSettings,
    WalletSummary,
};

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn map_err(err: wallet_core::WalletError) -> String {
    err.to_string()
}

#[tauri::command]
async fn wallet_exists(
    app: AppHandle,
    state: State<'_, Arc<WalletApp>>,
) -> Result<bool, String> {
    let dir = data_dir(&app)?;
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.exists(&dir))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn wallet_is_locked(state: State<'_, Arc<WalletApp>>) -> Result<bool, String> {
    let wallet = Arc::clone(&state);
    Ok(wallet.is_locked())
}

#[tauri::command]
async fn wallet_needs_migration(state: State<'_, Arc<WalletApp>>) -> Result<bool, String> {
    let wallet = Arc::clone(&state);
    Ok(wallet.needs_migration())
}

#[tauri::command]
async fn unlock_wallet(
    state: State<'_, Arc<WalletApp>>,
    req: UnlockRequest,
) -> Result<(), String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.unlock(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn lock_wallet(state: State<'_, Arc<WalletApp>>) -> Result<(), String> {
    let wallet = Arc::clone(&state);
    wallet.lock();
    Ok(())
}

#[tauri::command]
async fn migrate_encrypt(
    state: State<'_, Arc<WalletApp>>,
    req: MigrateEncryptRequest,
) -> Result<(), String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.migrate_encrypt(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn create_wallet(
    app: AppHandle,
    state: State<'_, Arc<WalletApp>>,
    req: CreateWalletRequest,
    passphrase: String,
) -> Result<CreateWalletResponse, String> {
    let dir = data_dir(&app)?;
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        wallet.create(&dir, req, &passphrase).map_err(map_err)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn restore_wallet(
    app: AppHandle,
    state: State<'_, Arc<WalletApp>>,
    req: RestoreWalletRequest,
    passphrase: String,
) -> Result<WalletSummary, String> {
    let dir = data_dir(&app)?;
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        wallet.restore(&dir, req, &passphrase).map_err(map_err)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn load_wallet(
    app: AppHandle,
    state: State<'_, Arc<WalletApp>>,
) -> Result<WalletSummary, String> {
    let dir = data_dir(&app)?;
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.load(&dir).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn sync_wallet(state: State<'_, Arc<WalletApp>>) -> Result<SyncResult, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.sync().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_summary(state: State<'_, Arc<WalletApp>>) -> Result<WalletSummary, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.summary().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_combined_summary(
    state: State<'_, Arc<WalletApp>>,
) -> Result<CombinedSummary, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.combined_summary().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn address_reuse_hint(
    state: State<'_, Arc<WalletApp>>,
    address: String,
) -> Result<AddressReuseHint, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        wallet.address_reuse_hint(&address).map_err(map_err)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_tx_labels(
    state: State<'_, Arc<WalletApp>>,
) -> Result<HashMap<String, String>, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.get_tx_labels().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn set_tx_label(
    state: State<'_, Arc<WalletApp>>,
    req: SetTxLabelRequest,
) -> Result<(), String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.set_tx_label(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_transactions(state: State<'_, Arc<WalletApp>>) -> Result<Vec<TxRecord>, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.transactions().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_receive_address(state: State<'_, Arc<WalletApp>>) -> Result<String, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.receive_address().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_mweb_receive_address(state: State<'_, Arc<WalletApp>>) -> Result<String, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.mweb_receive_address().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn estimate_fee(state: State<'_, Arc<WalletApp>>) -> Result<FeeEstimate, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.estimate_fee().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn preview_send(
    state: State<'_, Arc<WalletApp>>,
    req: SendRequest,
) -> Result<SendPreview, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.preview_send(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn send_ltc(
    state: State<'_, Arc<WalletApp>>,
    req: SendRequest,
) -> Result<SendResult, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.send(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn preview_pegin(
    state: State<'_, Arc<WalletApp>>,
    req: PeginRequest,
) -> Result<PeginPreview, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.preview_pegin(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pegin_ltc(
    state: State<'_, Arc<WalletApp>>,
    req: PeginRequest,
) -> Result<PeginResult, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.pegin(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn preview_mweb_send(
    state: State<'_, Arc<WalletApp>>,
    req: MwebSendRequest,
) -> Result<MwebSendPreview, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.preview_mweb_send(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn mweb_send_ltc(
    state: State<'_, Arc<WalletApp>>,
    req: MwebSendRequest,
) -> Result<MwebBroadcastResult, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.mweb_send(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn preview_pegout(
    state: State<'_, Arc<WalletApp>>,
    req: PegoutRequest,
) -> Result<PegoutPreview, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.preview_pegout(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pegout_ltc(
    state: State<'_, Arc<WalletApp>>,
    req: PegoutRequest,
) -> Result<MwebBroadcastResult, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.pegout(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn resync_mweb(state: State<'_, Arc<WalletApp>>) -> Result<(), String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.resync_mweb().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

/// Switch the MWEB derivation scheme and rescan under it (wipes local MWEB
/// state only; transparent wallet data is untouched).
#[tauri::command]
async fn set_mweb_scheme(
    state: State<'_, Arc<WalletApp>>,
    scheme: wallet_core::MwebScheme,
) -> Result<(), String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.set_mweb_scheme(scheme).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

/// Lock-free snapshot of MWEB download progress; pollable while a sync runs.
#[tauri::command]
async fn mweb_sync_progress(
    state: State<'_, Arc<WalletApp>>,
) -> Result<MwebSyncProgress, String> {
    Ok(state.mweb_sync_progress())
}

#[tauri::command]
async fn get_settings(state: State<'_, Arc<WalletApp>>) -> Result<WalletSettings, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.settings().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn update_settings(
    state: State<'_, Arc<WalletApp>>,
    req: UpdateSettingsRequest,
) -> Result<(), String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.update_settings(req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn explorer_tx_url(
    state: State<'_, Arc<WalletApp>>,
    txid: String,
) -> Result<String, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.explorer_tx_url(&txid).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn explorer_block_url(
    state: State<'_, Arc<WalletApp>>,
    block_hash: String,
) -> Result<String, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        wallet.explorer_block_url(&block_hash).map_err(map_err)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Open an http(s) URL in the system browser (never navigates the WebView).
#[tauri::command]
async fn open_explorer_url(url: String) -> Result<(), String> {
    wallet_core::explorer::validate_open_url(&url).map_err(map_err)?;
    open_url_in_browser(&url)
}

fn open_url_in_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err("opening URLs is not supported on this platform".into());
    }
    Ok(())
}

#[tauri::command]
async fn fetch_tx_detail(
    state: State<'_, Arc<WalletApp>>,
    txid: String,
) -> Result<TxEnrichment, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.fetch_tx_detail(&txid).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn fetch_spot_price(state: State<'_, Arc<WalletApp>>) -> Result<f64, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.fetch_spot_price().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn fetch_fee_ladder(state: State<'_, Arc<WalletApp>>) -> Result<FeeLadder, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.fetch_fee_ladder().map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

/// Phrase the user must type before a wipe is executed. Checked here at the
/// IPC boundary (not only in the UI) so a scripted or compromised webview
/// cannot destroy the wallet with a bare `invoke("wipe_wallet")`.
const WIPE_CONFIRMATION_PHRASE: &str = "DELETE WALLET";

#[tauri::command]
async fn wipe_wallet(
    app: AppHandle,
    state: State<'_, Arc<WalletApp>>,
    confirmation: String,
) -> Result<(), String> {
    if confirmation.trim() != WIPE_CONFIRMATION_PHRASE {
        return Err(format!(
            "wipe refused: type {WIPE_CONFIRMATION_PHRASE} to confirm deleting all wallet data"
        ));
    }
    let dir = data_dir(&app)?;
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.wipe(&dir).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(&dir)?;
            app.manage(Arc::new(WalletApp::new(&dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            wallet_exists,
            wallet_is_locked,
            wallet_needs_migration,
            unlock_wallet,
            lock_wallet,
            migrate_encrypt,
            create_wallet,
            restore_wallet,
            load_wallet,
            sync_wallet,
            get_summary,
            get_combined_summary,
            list_transactions,
            address_reuse_hint,
            get_tx_labels,
            set_tx_label,
            get_receive_address,
            get_mweb_receive_address,
            estimate_fee,
            preview_send,
            send_ltc,
            preview_pegin,
            pegin_ltc,
            preview_mweb_send,
            mweb_send_ltc,
            preview_pegout,
            pegout_ltc,
            resync_mweb,
            set_mweb_scheme,
            mweb_sync_progress,
            get_settings,
            update_settings,
            explorer_tx_url,
            explorer_block_url,
            open_explorer_url,
            fetch_tx_detail,
            fetch_spot_price,
            fetch_fee_ladder,
            wipe_wallet,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
