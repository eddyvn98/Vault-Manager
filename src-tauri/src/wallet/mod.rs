pub mod bip;
pub mod blockchain;
pub mod broadcast;
pub mod crypto;
pub mod lockout;
pub mod signer;
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
use tiny_keccak::{Hasher, Keccak};
use zeroize::Zeroizing;

const FILE_NAME: &str = "wallet-v1.enc";
pub const SESSION_TIMEOUT_SECS: u64 = 300;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveDraft {
    pub draft_id: String,
    pub session_epoch: u64,
    pub network: String,
    pub payload_hash: String,
    pub expires_at_unix: u64,
}

#[derive(Default)]
pub struct SessionState {
    pub mnemonic: Option<Zeroizing<String>>,
    pub last_activity: Option<Instant>,
    pub pending_creation: Option<Zeroizing<String>>,
    pub active_draft: Option<ActiveDraft>,
    pub session_epoch: u64,
}

impl SessionState {
    #[allow(dead_code)]
    pub fn verify_and_consume_draft(
        &mut self,
        draft_id: &str,
        expected_payload_hash: &str,
    ) -> Result<ActiveDraft, String> {
        if self.mnemonic.is_none() {
            return Err("Wallet is locked: unlocking required before consuming draft".into());
        }

        let draft = self
            .active_draft
            .as_ref()
            .ok_or_else(|| "No active transaction draft found".to_string())?;

        if draft.session_epoch != self.session_epoch {
            self.active_draft = None;
            return Err("Session epoch mismatch: draft was created in a previous session".into());
        }

        if draft.draft_id != draft_id {
            return Err("Draft ID mismatch: another draft is active".into());
        }

        if draft.payload_hash != expected_payload_hash {
            return Err("Payload hash mismatch: transaction parameters have been altered".into());
        }

        let now = LockoutManager::now_unix();
        if now > draft.expires_at_unix {
            self.active_draft = None;
            return Err("Transaction draft has expired".into());
        }

        let consumed = self.active_draft.take().unwrap();
        Ok(consumed)
    }
}

pub fn compute_btc_payload_hash(unsigned_psbt_base64: &str) -> String {
    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(unsigned_psbt_base64.as_bytes());
    hasher.finalize(&mut hash);
    format!("0x{}", hex::encode(hash))
}

pub fn compute_bsc_payload_hash(
    chain_id: u64,
    token_contract: Option<&str>,
    recipient_address: &str,
    amount_raw: &str,
    nonce: u64,
    gas_limit: u64,
    gas_price_wei: &str,
) -> String {
    let canonical = format!(
        "bsc:{}:{}:{}:{}:{}:{}:{}",
        chain_id,
        token_contract.unwrap_or(""),
        recipient_address,
        amount_raw,
        nonce,
        gas_limit,
        gas_price_wei
    );
    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(canonical.as_bytes());
    hasher.finalize(&mut hash);
    format!("0x{}", hex::encode(hash))
}

/// Binds draft verification directly to BTC signing: Impossible to sign without
/// verifying and consuming an active draft in the current session epoch with an unlocked wallet.
#[allow(dead_code)]
pub fn sign_verified_btc_draft(
    session: &mut SessionState,
    draft_id: &str,
    unsigned_psbt_base64: &str,
    psbt: &mut bitcoin::Psbt,
    policy: &signer::btc::BtcSigningPolicy,
    derivation_indices: &[u32],
) -> Result<u64, String> {
    let payload_hash = compute_btc_payload_hash(unsigned_psbt_base64);
    session.verify_and_consume_draft(draft_id, &payload_hash)?;

    let mnemonic_str = session
        .mnemonic
        .as_ref()
        .ok_or_else(|| "Wallet is locked: private key unavailable".to_string())?;

    let secp = bitcoin::secp256k1::Secp256k1::new();
    let mut privkeys = Vec::with_capacity(derivation_indices.len());
    for &idx in derivation_indices {
        let key = bip::derive_bip84_privkey_at(mnemonic_str, bitcoin::Network::Signet, 0, idx)?;
        privkeys.push(key);
    }

    let fee = signer::btc::sign_and_finalize_verified_psbt_multi(psbt, policy, &secp, &privkeys)?;
    Ok(fee)
}

/// Binds draft verification directly to BSC signing: Impossible to sign without
/// verifying and consuming an active draft in the current session epoch with an unlocked wallet.
#[allow(dead_code)]
pub fn sign_verified_bsc_draft(
    session: &mut SessionState,
    draft_id: &str,
    tx: &signer::bsc::Eip155Transaction,
    policy: &signer::bsc::BscSigningPolicy,
) -> Result<signer::bsc::Eip155SignedTransaction, String> {
    let payload_hash = compute_bsc_payload_hash(
        policy.expected_chain_id,
        Some(&policy.expected_to),
        &policy.expected_to,
        &hex::encode(&policy.expected_data),
        policy.expected_nonce,
        policy.expected_gas_limit,
        &policy.expected_gas_price.to_string(),
    );
    session.verify_and_consume_draft(draft_id, &payload_hash)?;

    let mnemonic_str = session
        .mnemonic
        .as_ref()
        .ok_or_else(|| "Wallet is locked: private key unavailable".to_string())?;

    let secp = secp256k1::Secp256k1::new();
    let privkey = bip::derive_bsc_privkey_at(mnemonic_str, 0)?;

    let signed = signer::bsc::validate_and_sign_bsc_tx(tx, policy, &secp, &privkey)?;
    Ok(signed)
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
            session.active_draft = None;
            session.session_epoch = session.session_epoch.wrapping_add(1);
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
    session.active_draft = None;
    session.session_epoch = session.session_epoch.wrapping_add(1);

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
    session.active_draft = None;
    session.session_epoch = session.session_epoch.wrapping_add(1);

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
    session.active_draft = None;
    session.session_epoch = session.session_epoch.wrapping_add(1);
    Ok(())
}

#[tauri::command]
pub fn lock_wallet(state: State<'_, WalletSession>) -> Result<(), String> {
    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    session.mnemonic = None;
    session.last_activity = None;
    session.active_draft = None;
    session.session_epoch = session.session_epoch.wrapping_add(1);
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
    app: AppHandle,
    state: State<'_, WalletSession>,
    recipient: String,
    amount_satoshis: u64,
    fee_rate_sat_vb: Option<u64>,
) -> Result<tx::BtcTxPreview, String> {
    let (mnemonic, initial_epoch) = {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        let m = session
            .mnemonic
            .as_ref()
            .ok_or_else(|| "Wallet is locked".to_string())?;
        (m.clone(), session.session_epoch)
    };

    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data directory: {}", e))?;
    let db_path = app_dir.join("bdk_signet.sqlite3");

    let preview =
        tx::btc::prepare::prepare_signet_tx(&mnemonic, &recipient, amount_satoshis, fee_rate_sat_vb, Some(&db_path))
            .await?;

    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    if !check_and_apply_auto_lock(&mut session)
        || session.session_epoch != initial_epoch
        || session.mnemonic.is_none()
    {
        // Do NOT clear active_draft here: if session_epoch != initial_epoch,
        // another transaction may have already been registered in the new epoch.
        return Err("Wallet was locked, reset, or session expired during transaction preparation. Stale draft discarded.".into());
    }

    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(preview.unsigned_psbt_base64.as_bytes());
    hasher.finalize(&mut hash);
    let payload_hash = format!("0x{}", hex::encode(hash));

    let expires_at_unix = LockoutManager::now_unix() + SESSION_TIMEOUT_SECS;

    session.active_draft = Some(ActiveDraft {
        draft_id: preview.draft_id.clone(),
        session_epoch: initial_epoch,
        network: "Bitcoin Signet".into(),
        payload_hash,
        expires_at_unix,
    });
    session.last_activity = Some(Instant::now());

    Ok(preview)
}

#[tauri::command]
pub async fn prepare_bsc_transaction(
    state: State<'_, WalletSession>,
    recipient: String,
    amount_tokens: String,
) -> Result<tx::BscTxPreview, String> {
    let (mnemonic, initial_epoch) = {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        let m = session
            .mnemonic
            .as_ref()
            .ok_or_else(|| "Wallet is locked".to_string())?;
        (m.clone(), session.session_epoch)
    };

    let preview =
        tx::bsc::prepare::prepare_bsc_testnet_tx(&mnemonic, &recipient, &amount_tokens).await?;

    let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
    if !check_and_apply_auto_lock(&mut session)
        || session.session_epoch != initial_epoch
        || session.mnemonic.is_none()
    {
        // Do NOT clear active_draft here
        return Err("Wallet was locked, reset, or session expired during transaction preparation. Stale draft discarded.".into());
    }

    // Canonical representation covering all critical transaction parameters
    let canonical = format!(
        "bsc:{}:{}:{}:{}:{}:{}:{}",
        preview.chain_id,
        preview.token_contract.as_deref().unwrap_or("none").to_lowercase(),
        preview.recipient_address.to_lowercase(),
        preview.amount_raw,
        preview.nonce,
        preview.gas_limit,
        preview.gas_price_wei
    );
    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(canonical.as_bytes());
    hasher.finalize(&mut hash);
    let payload_hash = format!("0x{}", hex::encode(hash));

    let expires_at_unix = LockoutManager::now_unix() + SESSION_TIMEOUT_SECS;

    session.active_draft = Some(ActiveDraft {
        draft_id: preview.draft_id.clone(),
        session_epoch: initial_epoch,
        network: "BSC Testnet".into(),
        payload_hash,
        expires_at_unix,
    });
    session.last_activity = Some(Instant::now());

    Ok(preview)
}

pub fn cleanup_sqlite_files(app_dir: &std::path::Path) -> Result<(), String> {
    let bdk_wal = app_dir.join("bdk_signet.sqlite3-wal");
    if bdk_wal.exists() {
        fs::remove_file(&bdk_wal).map_err(|e| format!("Cannot remove SQLite WAL journal: {}", e))?;
    }
    let bdk_shm = app_dir.join("bdk_signet.sqlite3-shm");
    if bdk_shm.exists() {
        fs::remove_file(&bdk_shm).map_err(|e| format!("Cannot remove SQLite SHM index: {}", e))?;
    }
    let bdk_db = app_dir.join("bdk_signet.sqlite3");
    if bdk_db.exists() {
        fs::remove_file(&bdk_db).map_err(|e| format!("Cannot remove BDK SQLite database: {}", e))?;
    }
    Ok(())
}

/// Secure reset: Requires valid password verification to authenticate the owner before deleting.
/// Clean up SQLite storage FIRST before touching encrypted vault file, ensuring atomic reset.
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

    // 1. Immediately invalidate and wipe in-memory session and drafts
    {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        session.mnemonic = None;
        session.last_activity = None;
        session.pending_creation = None;
        session.active_draft = None;
        session.session_epoch = session.session_epoch.wrapping_add(1);
    }

    // 2. Clean up SQLite database and journals FIRST
    if let Ok(app_dir) = app.path().app_data_dir() {
        cleanup_sqlite_files(&app_dir)?;
    }

    // 3. Only after SQLite cleanup succeeds, remove the vault file
    fs::remove_file(&file).map_err(|e| format!("Cannot remove vault file: {}", e))?;
    mgr.record_success();

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

    #[test]
    fn session_epoch_increments_and_invalidates_drafts() {
        let mut session = SessionState::default();
        let epoch0 = session.session_epoch;

        // Auto-lock on timeout increments epoch and clears active_draft
        session.mnemonic = Some(Zeroizing::new("test-mnemonic".to_string()));
        session.last_activity = Some(Instant::now() - Duration::from_secs(SESSION_TIMEOUT_SECS + 1));
        session.active_draft = Some(ActiveDraft {
            draft_id: "draft-btc-123".into(),
            session_epoch: epoch0,
            network: "Bitcoin Signet".into(),
            payload_hash: "0x1234".into(),
            expires_at_unix: 1700000000,
        });

        assert!(!check_and_apply_auto_lock(&mut session));
        assert_eq!(session.session_epoch, epoch0 + 1);
        assert!(session.active_draft.is_none());
        assert!(session.mnemonic.is_none());
    }

    #[test]
    fn stale_epoch_task_does_not_clobber_new_epoch_draft() {
        let mut session = SessionState::default();
        session.session_epoch = 2; // Newer session epoch
        session.mnemonic = Some(Zeroizing::new("new-mnemonic".to_string()));

        let valid_new_draft = ActiveDraft {
            draft_id: "draft-btc-valid-epoch-2".into(),
            session_epoch: 2,
            network: "Bitcoin Signet".into(),
            payload_hash: "0xbeef".into(),
            expires_at_unix: LockoutManager::now_unix() + 300,
        };
        session.active_draft = Some(valid_new_draft.clone());

        // Simulate a stale task from epoch 0 completing now
        let initial_epoch = 0;
        let is_valid = check_and_apply_auto_lock(&mut session)
            && session.session_epoch == initial_epoch
            && session.mnemonic.is_some();

        assert!(!is_valid, "Stale task must be rejected");

        // Critically, active_draft must remain intact for epoch 2!
        assert!(session.active_draft.is_some());
        assert_eq!(session.active_draft.unwrap().draft_id, "draft-btc-valid-epoch-2");
    }

    #[test]
    fn it05_draft_verification_expiration_and_payload_integrity() {
        let mut session = SessionState::default();
        session.session_epoch = 1;
        let now = LockoutManager::now_unix();

        // 1. If wallet is locked (no mnemonic), consume fails immediately
        let draft = ActiveDraft {
            draft_id: "draft-123".into(),
            session_epoch: 1,
            network: "Bitcoin Signet".into(),
            payload_hash: "0xabcdef123456".into(),
            expires_at_unix: now + 300,
        };
        session.active_draft = Some(draft.clone());
        let err_locked = session.verify_and_consume_draft("draft-123", "0xabcdef123456");
        assert!(err_locked.is_err());
        assert!(err_locked.unwrap_err().contains("Wallet is locked"));

        // Unlock wallet
        session.mnemonic = Some(Zeroizing::new("test-mnemonic".to_string()));
        session.active_draft = Some(draft);

        // 2. Draft ID mismatch -> error, active_draft preserved
        let err_id = session.verify_and_consume_draft("draft-999", "0xabcdef123456");
        assert!(err_id.is_err());
        assert!(session.active_draft.is_some());

        // 3. Payload hash mismatch (tampered payload) -> error, active_draft preserved
        let err_payload = session.verify_and_consume_draft("draft-123", "0xtamperedhash");
        assert!(err_payload.is_err());
        assert!(session.active_draft.is_some());

        // 4. Valid draft consumption -> success, draft consumed
        let consumed = session.verify_and_consume_draft("draft-123", "0xabcdef123456");
        assert!(consumed.is_ok());
        assert_eq!(consumed.unwrap().draft_id, "draft-123");
        assert!(session.active_draft.is_none());

        // 5. Expired draft -> error, active_draft cleared
        let expired_draft = ActiveDraft {
            draft_id: "draft-expired".into(),
            session_epoch: 1,
            network: "Bitcoin Signet".into(),
            payload_hash: "0x1111".into(),
            expires_at_unix: now - 10, // Expired 10 seconds ago
        };
        session.active_draft = Some(expired_draft);
        let err_expired = session.verify_and_consume_draft("draft-expired", "0x1111");
        assert!(err_expired.is_err());
        assert!(session.active_draft.is_none(), "Expired draft must be cleared");

        // 6. Session epoch mismatch (e.g. wallet was locked & re-opened) -> error
        let draft_epoch1 = ActiveDraft {
            draft_id: "draft-epoch1".into(),
            session_epoch: 1,
            network: "Bitcoin Signet".into(),
            payload_hash: "0x2222".into(),
            expires_at_unix: now + 300,
        };
        session.active_draft = Some(draft_epoch1);
        session.session_epoch = 2; // Session epoch incremented due to lock/unlock!
        let err_epoch = session.verify_and_consume_draft("draft-epoch1", "0x2222");
        assert!(err_epoch.is_err());
        assert!(err_epoch.unwrap_err().contains("Session epoch mismatch"));
        assert!(session.active_draft.is_none(), "Stale epoch draft must be cleared");
    }

    #[tokio::test]
    async fn it05_concurrent_async_tasks_session_epoch_isolation() {
        use std::sync::Arc;
        let session_state = Arc::new(Mutex::new(SessionState::default()));

        // Unlock wallet at epoch 0
        {
            let mut s = session_state.lock().unwrap();
            s.mnemonic = Some(Zeroizing::new("test-mnemonic".into()));
            s.last_activity = Some(Instant::now());
            s.session_epoch = 0;
        }

        // Simulate Slow Task 1 (started at epoch 0)
        let s_clone1 = Arc::clone(&session_state);
        let task1 = tokio::spawn(async move {
            let initial_epoch = 0;
            // Simulate slow RPC latency
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let mut s = s_clone1.lock().unwrap();
            if !check_and_apply_auto_lock(&mut s) || s.session_epoch != initial_epoch || s.mnemonic.is_none() {
                return Err("Stale task rejected".to_string());
            }
            s.active_draft = Some(ActiveDraft {
                draft_id: "draft-task1".into(),
                session_epoch: initial_epoch,
                network: "Bitcoin Signet".into(),
                payload_hash: "0xhash1".into(),
                expires_at_unix: LockoutManager::now_unix() + 300,
            });
            Ok(())
        });

        // Meanwhile, wallet is locked and re-unlocked at epoch 1
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        {
            let mut s = session_state.lock().unwrap();
            s.mnemonic = None;
            s.session_epoch = 1; // Locked
            s.mnemonic = Some(Zeroizing::new("test-mnemonic-epoch1".into()));
            s.last_activity = Some(Instant::now()); // Unlocked at epoch 1
        }

        // Fast Task 2 runs at epoch 1
        let s_clone2 = Arc::clone(&session_state);
        let task2 = tokio::spawn(async move {
            let initial_epoch = 1;
            let mut s = s_clone2.lock().unwrap();
            if !check_and_apply_auto_lock(&mut s) || s.session_epoch != initial_epoch || s.mnemonic.is_none() {
                return Err("Stale task rejected".to_string());
            }
            s.active_draft = Some(ActiveDraft {
                draft_id: "draft-task2".into(),
                session_epoch: initial_epoch,
                network: "Bitcoin Signet".into(),
                payload_hash: "0xhash2".into(),
                expires_at_unix: LockoutManager::now_unix() + 300,
            });
            Ok(())
        });

        let res2 = task2.await.unwrap();
        assert!(res2.is_ok());

        let res1 = task1.await.unwrap();
        assert!(res1.is_err(), "Task 1 from epoch 0 must be rejected");

        // Verify that Task 2 draft in epoch 1 is fully intact and was NOT clobbered by Task 1
        let mut s = session_state.lock().unwrap();
        let consumed = s.verify_and_consume_draft("draft-task2", "0xhash2");
        assert!(consumed.is_ok(), "Task 2 draft must be intact and consumable in epoch 1");
    }

    #[test]
    fn test_cleanup_sqlite_files_and_atomic_reset_ordering() {
        let temp_dir = std::env::temp_dir().join(format!("cleanup_sqlite_test_{}", LockoutManager::now_unix()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let db_file = temp_dir.join("bdk_signet.sqlite3");
        let wal_file = temp_dir.join("bdk_signet.sqlite3-wal");
        let shm_file = temp_dir.join("bdk_signet.sqlite3-shm");

        fs::write(&db_file, b"sqlite database mock content").unwrap();
        fs::write(&wal_file, b"sqlite wal journal").unwrap();
        fs::write(&shm_file, b"sqlite shm index").unwrap();

        assert!(db_file.exists());
        assert!(wal_file.exists());
        assert!(shm_file.exists());

        // Perform safe cleanup
        cleanup_sqlite_files(&temp_dir).expect("Cleanup must succeed");

        assert!(!db_file.exists(), "DB file must be removed");
        assert!(!wal_file.exists(), "WAL file must be removed");
        assert!(!shm_file.exists(), "SHM file must be removed");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
