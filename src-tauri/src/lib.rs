mod wallet;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(wallet::WalletSession::default())
        .invoke_handler(tauri::generate_handler![
            wallet::wallet_status,
            wallet::start_wallet_creation,
            wallet::finalize_wallet_creation,
            wallet::restore_wallet,
            wallet::unlock_wallet,
            wallet::lock_wallet,
            wallet::touch_session,
            wallet::reset_wallet
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Vault Manager local wallet");
}
