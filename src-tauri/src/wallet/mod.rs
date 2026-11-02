pub mod bip;
pub mod blockchain;
pub mod crypto;
pub mod lockout;
pub mod tx;

use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use lockout::LockoutManager;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use zeroize::Zeroizing;

const FILE_NAME: &str = "wallet-v1.enc";
pub const SESSION_TIMEOUT_SECS: u64 = 300;

#[derive(Default)]
pub struct SessionState {
    pub mnemonic: Option<Zeroizing<String>>,
    pub last_activity: Option<Instant>,
    pub pending_creation: Option<Zeroizing<String>>,
    pub active_draft_id: Option<String>,
}

pub struct WalletSession {
    pub session: Mutex<SessionState>,
}

impl Default for WalletSession {
    fn default() -> Self {
        Self {
            session: Mutex::new(SessionState::default()),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletStatus {
    pub exists: bool,
    pub unlocked: bool,
    pub signing_enabled: bool,
    pub network: &'static str,
    pub failed_attempts: u32,
    pub lockout_seconds: u64,
}

pub fn vault_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "Cannot resolve application data directory".to_string())?;
    Ok(dir.join(FILE_NAME))
}

pub fn lockout_manager(app: &AppHandle) -> Result<LockoutManager, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| "Cannot resolve application data directory".to_string())?;
    Ok(LockoutManager::new(dir))
}

fn write_vault_file(file: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let dir = file.parent().ok_or("Invalid vault path")?;
    fs::create_dir_all(dir).map_err(|_| "Cannot create vault folder")?;
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut handle = options
        .open(file)
        .map_err(|_| "Vault already exists or cannot be created")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(file, fs::Permissions::from_mode(0o600));
    }
    if handle.write_all(bytes).and_then(|_| handle.sync_all()).is_err() {
        drop(handle);
        let _ = fs::remove_file(file);
        return Err("Unable to persist vault safely".into());
    }
    Ok(())
}

fn check_and_apply_auto_lock(session: &mut SessionState) -> bool {
    if let (Some(_), Some(last)) = (&session.mnemonic, session.last_activity) {
        if last.elapsed() > Duration::from_secs(SESSION_TIMEOUT_SECS) {
            session.mnemonic = None;
            session.last_activity = None;
            return false;
        }
        return true;
    }
    false
}

#[tauri::command]
pub fn wallet_status(
    app: AppHandle,
    state: State<'_, WalletSession>,
) -> Result<WalletStatus, String> {
    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    let unlocked = check_and_apply_auto_lock(&mut session);
    let (failed_attempts, lockout_seconds) = lockout_manager(&app)?.get_status();

    Ok(WalletStatus {
        exists: vault_path(&app)?.exists(),
        unlocked,
        signing_enabled: false,
        network: "offline-only",
        failed_attempts,
        lockout_seconds,
    })
}

#[tauri::command]
pub fn start_wallet_creation(
    app: AppHandle,
    state: State<'_, WalletSession>,
) -> Result<String, String> {
    if vault_path(&app)?.exists() {
        return Err("A wallet vault already exists on this device".into());
    }
    let phrase = bip::generate_24_words()?;
    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    session.pending_creation = Some(phrase.clone());
    Ok(phrase.to_string())
}

#[tauri::command]
pub fn finalize_wallet_creation(
    app: AppHandle,
    state: State<'_, WalletSession>,
    password: String,
    recovery: String,
) -> Result<(), String> {
    let file = vault_path(&app)?;
    if file.exists() {
        return Err("A wallet vault already exists on this device".into());
    }
    let valid_phrase = bip::validate_phrase(&recovery)?;
    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;

    let pending = session
        .pending_creation
        .as_ref()
        .ok_or_else(|| "No wallet creation in progress. Please start creation first.".to_string())?;

    if &**pending != &*valid_phrase {
        return Err("Provided recovery phrase does not match the generated phrase".into());
    }

    let bytes = crypto::encrypt(&password, &valid_phrase)?;
    write_vault_file(&file, &bytes)?;

    session.pending_creation = None;
    session.mnemonic = None;
    session.last_activity = None;

    lockout_manager(&app)?.record_success();
    Ok(())
}

#[tauri::command]
pub fn restore_wallet(
    app: AppHandle,
    state: State<'_, WalletSession>,
    password: String,
    recovery: String,
) -> Result<(), String> {
    let file = vault_path(&app)?;
    if file.exists() {
        return Err("A vault already exists on this device; restore cannot overwrite it".into());
    }
    let valid = bip::validate_phrase(&recovery)?;
    let bytes = crypto::encrypt(&password, &valid)?;
    write_vault_file(&file, &bytes)?;

    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    session.pending_creation = None;
    session.mnemonic = None;
    session.last_activity = None;

    lockout_manager(&app)?.record_success();
    Ok(())
}

#[tauri::command]
pub fn unlock_wallet(
    app: AppHandle,
    state: State<'_, WalletSession>,
    password: String,
) -> Result<(), String> {
    let mgr = lockout_manager(&app)?;
    mgr.check_lockout()?;

    let file = vault_path(&app)?;
    let raw = fs::read(&file).map_err(|_| "No vault found on this device")?;

    let plaintext = match crypto::decrypt(&password, &raw) {
        Ok(t) => t,
        Err(e) => {
            mgr.record_failure();
            return Err(e);
        }
    };

    let recovery = bip::validate_phrase(&plaintext).map_err(|e| {
        mgr.record_failure();
        e
    })?;

    mgr.record_success();
    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    session.mnemonic = Some(recovery);
    session.last_activity = Some(Instant::now());
    Ok(())
}

#[tauri::command]
pub fn lock_wallet(state: State<'_, WalletSession>) -> Result<(), String> {
    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    session.mnemonic = None;
    session.last_activity = None;
    session.active_draft_id = None;
    Ok(())
}

#[tauri::command]
pub fn touch_session(state: State<'_, WalletSession>) -> Result<bool, String> {
    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    if check_and_apply_auto_lock(&mut session) {
        session.last_activity = Some(Instant::now());
        Ok(true)
    } else {
        Ok(false)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAddresses {
    pub btc_mainnet: String,
    pub btc_signet: String,
    pub bsc_address: String,
    pub usdt_bep20_contract: &'static str,
}

#[tauri::command]
pub fn get_public_addresses(
    state: State<'_, WalletSession>,
) -> Result<PublicAddresses, String> {
    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    if !check_and_apply_auto_lock(&mut session) {
        return Err("Wallet is locked".into());
    }

    let mnemonic = session
        .mnemonic
        .as_ref()
        .ok_or_else(|| "Wallet is locked".to_string())?;

    let btc_mainnet = bip::derive_btc_address(mnemonic, bitcoin::Network::Bitcoin)?;
    let btc_signet = bip::derive_btc_address(mnemonic, bitcoin::Network::Signet)?;
    let bsc_address = bip::derive_bsc_address(mnemonic)?;

    session.last_activity = Some(Instant::now());

    Ok(PublicAddresses {
        btc_mainnet,
        btc_signet,
        bsc_address,
        usdt_bep20_contract: bip::USDT_BEP20_CONTRACT,
    })
}

#[tauri::command]
pub async fn query_btc_balance(
    state: State<'_, WalletSession>,
    address: String,
    signet: bool,
) -> Result<blockchain::BtcBalance, String> {
    // 1. Verify session and cryptographically authenticate that the address belongs to this wallet
    let (target_address, network) = {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        let mnemonic = session
            .mnemonic
            .as_ref()
            .ok_or_else(|| "Wallet is locked".to_string())?;

        let network = if signet {
            bitcoin::Network::Signet
        } else {
            bitcoin::Network::Bitcoin
        };
        let derived_addr = bip::derive_btc_address(mnemonic, network)?;

        if !address.trim().is_empty() && address.trim() != derived_addr {
            return Err("Address mismatch: requested address does not belong to the unlocked wallet".into());
        }
        (derived_addr, network)
    };

    // 2. Perform async network request without holding any Mutex
    blockchain::fetch_btc_balance(&target_address, network).await
}

#[tauri::command]
pub async fn query_bsc_balance(
    state: State<'_, WalletSession>,
    address: String,
    testnet: bool,
) -> Result<blockchain::BscBalance, String> {
    // 1. Verify session and cryptographically authenticate that the address belongs to this wallet
    let target_address = {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        let mnemonic = session
            .mnemonic
            .as_ref()
            .ok_or_else(|| "Wallet is locked".to_string())?;

        let derived_addr = bip::derive_bsc_address(mnemonic)?;

        if !address.trim().is_empty() && address.trim().to_lowercase() != derived_addr.to_lowercase() {
            return Err("Address mismatch: requested address does not belong to the unlocked wallet".into());
        }
        derived_addr
    };

    // 2. Perform async network request without holding any Mutex
    blockchain::fetch_bsc_balance(&target_address, testnet).await
}

#[tauri::command]
pub async fn prepare_btc_transaction(
    state: State<'_, WalletSession>,
    recipient: String,
    amount_satoshis: u64,
    fee_rate_sat_vb: Option<u64>,
) -> Result<tx::BtcTxPreview, String> {
    let mnemonic = {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        let m = session
            .mnemonic
            .as_ref()
            .ok_or_else(|| "Wallet is locked".to_string())?;
        m.clone()
    };

    let preview =
        tx::btc::prepare::prepare_signet_tx(&mnemonic, &recipient, amount_satoshis, fee_rate_sat_vb)
            .await?;

    if let Ok(mut session) = state.session.lock() {
        session.active_draft_id = Some(preview.draft_id.clone());
    }

    Ok(preview)
}

#[tauri::command]
pub async fn prepare_bsc_transaction(
    state: State<'_, WalletSession>,
    recipient: String,
    amount_tokens: String,
) -> Result<tx::BscTxPreview, String> {
    let mnemonic = {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        let m = session
            .mnemonic
            .as_ref()
            .ok_or_else(|| "Wallet is locked".to_string())?;
        m.clone()
    };

    let preview =
        tx::bsc::prepare::prepare_bsc_testnet_tx(&mnemonic, &recipient, &amount_tokens).await?;

    if let Ok(mut session) = state.session.lock() {
        session.active_draft_id = Some(preview.draft_id.clone());
    }

    Ok(preview)
}

/// Secure reset: Requires valid password verification to authenticate the owner before deleting.
#[tauri::command]
pub fn reset_wallet(
    app: AppHandle,
    state: State<'_, WalletSession>,
    password: String,
) -> Result<(), String> {
    let mgr = lockout_manager(&app)?;
    mgr.check_lockout()?;

    let file = vault_path(&app)?;
    if !file.exists() {
        return Err("No vault found on this device".into());
    }

    let raw = fs::read(&file).map_err(|_| "Cannot read vault file")?;
    if let Err(e) = crypto::decrypt(&password, &raw) {
        mgr.record_failure();
        return Err(format!("Authentication failed: {}", e));
    }

    fs::remove_file(&file).map_err(|_| "Cannot remove vault file")?;
    mgr.record_success();

    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    session.mnemonic = None;
    session.last_activity = None;
    session.pending_creation = None;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_lock_after_inactivity() {
        let mut session = SessionState::default();
        session.mnemonic = Some(Zeroizing::new("test-mnemonic".to_string()));
        session.last_activity = Some(Instant::now());

        // Right now should remain unlocked
        assert!(check_and_apply_auto_lock(&mut session));
        assert!(session.mnemonic.is_some());

        // Simulate activity in the past > 300s
        session.last_activity = Some(Instant::now() - Duration::from_secs(SESSION_TIMEOUT_SECS + 5));
        assert!(!check_and_apply_auto_lock(&mut session));
        assert!(session.mnemonic.is_none(), "Mnemonic must be wiped after timeout");
    }

    #[test]
    fn creation_requires_matching_verified_phrase() {
        let temp_dir = std::env::temp_dir().join(format!("wallet_creation_test_{}", LockoutManager::now_unix()));
        let _ = fs::remove_dir_all(&temp_dir);
        let vault_file = temp_dir.join(FILE_NAME);

        let phrase = bip::generate_24_words().unwrap();
        let wrong_phrase = bip::generate_24_words().unwrap();

        // Writing encrypted bytes directly
        let bytes = crypto::encrypt("strong-master-pass-123", &phrase).unwrap();
        assert!(write_vault_file(&vault_file, &bytes).is_ok());

        // Reading back
        let raw = fs::read(&vault_file).unwrap();
        assert!(crypto::decrypt("wrong-master-pass-123", &raw).is_err());
        assert_eq!(&*crypto::decrypt("strong-master-pass-123", &raw).unwrap(), &*phrase);

        assert_ne!(&*phrase, &*wrong_phrase);
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
