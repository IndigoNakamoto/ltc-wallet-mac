use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};
use wallet_core::{
    CombinedSummary, CreateWalletRequest, CreateWalletResponse, MigrateEncryptRequest,
    MwebBroadcastResult, MwebSendRequest, MwebSyncProgress, PeginRequest, PeginResult,
    PegoutRequest, RestoreWalletRequest, SendRequest, SendResult, SyncResult, TxRecord,
    UnlockRequest, UpdateSettingsRequest, WalletApp, WalletSettings, WalletSummary,
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
async fn wipe_wallet(
    app: AppHandle,
    state: State<'_, Arc<WalletApp>>,
) -> Result<(), String> {
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
            get_receive_address,
            get_mweb_receive_address,
            send_ltc,
            pegin_ltc,
            mweb_send_ltc,
            pegout_ltc,
            resync_mweb,
            mweb_sync_progress,
            get_settings,
            update_settings,
            wipe_wallet,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
