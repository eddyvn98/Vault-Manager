mod wallet;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(wallet::WalletSession::default())
        .invoke_handler(tauri::generate_handler![
            wallet::wallet_status,
            wallet::create_wallet,
            wallet::restore_wallet,
            wallet::unlock_wallet,
            wallet::lock_wallet
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Vault Manager local wallet");
}
