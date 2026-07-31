use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};
use wallet_core::{
    CreateWalletRequest, CreateWalletResponse, RestoreWalletRequest, SendRequest, SendResult,
    SyncResult, WalletApp, WalletSummary,
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
async fn create_wallet(
    app: AppHandle,
    state: State<'_, Arc<WalletApp>>,
    req: CreateWalletRequest,
) -> Result<CreateWalletResponse, String> {
    let dir = data_dir(&app)?;
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.create(&dir, req).map_err(map_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn restore_wallet(
    app: AppHandle,
    state: State<'_, Arc<WalletApp>>,
    req: RestoreWalletRequest,
) -> Result<WalletSummary, String> {
    let dir = data_dir(&app)?;
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.restore(&dir, req).map_err(map_err))
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
async fn get_receive_address(state: State<'_, Arc<WalletApp>>) -> Result<String, String> {
    let wallet = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || wallet.receive_address().map_err(map_err))
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
            create_wallet,
            restore_wallet,
            load_wallet,
            sync_wallet,
            get_summary,
            get_receive_address,
            send_ltc,
            wipe_wallet,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
