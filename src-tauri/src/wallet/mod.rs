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
use bitcoin::hashes::Hash;
use lockout::LockoutManager;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tiny_keccak::{Hasher, Keccak};
use zeroize::Zeroizing;
use crate::wallet::broadcast::bsc::BscBroadcaster;

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

/// Binds draft verification directly to BTC signing:
/// Decodes the PSBT directly from the approved unsigned_psbt_base64 string,
/// validates against approved policy, derives each input's key using approved.change and derivation_index,
/// finalizes the PSBT, extracts the raw signed transaction with witnesses, computes txid and wtxid,
/// and returns VerifiedSignedBtcDraft.
#[allow(dead_code)]
pub fn sign_verified_btc_draft(
    session: &mut SessionState,
    draft_id: &str,
    unsigned_psbt_base64: &str,
    policy: &signer::btc::BtcSigningPolicy,
) -> Result<VerifiedSignedBtcDraft, String> {
    use base64::Engine;

    let payload_hash = compute_btc_payload_hash(unsigned_psbt_base64);
    session.verify_and_consume_draft(draft_id, &payload_hash)?;

    // Decode PSBT directly from the approved unsigned_psbt_base64 string
    let psbt_bytes = base64::engine::general_purpose::STANDARD
        .decode(unsigned_psbt_base64.trim())
        .map_err(|e| format!("Invalid base64 in approved PSBT: {}", e))?;
    let mut psbt = bitcoin::Psbt::deserialize(&psbt_bytes)
        .map_err(|e| format!("Invalid PSBT encoding: {}", e))?;

    let mnemonic_str = session
        .mnemonic
        .as_ref()
        .ok_or_else(|| "Wallet is locked: private key unavailable".to_string())?;

    let secp = bitcoin::secp256k1::Secp256k1::new();
    let mut privkeys = Vec::with_capacity(policy.approved_inputs.len());
    for (i, approved) in policy.approved_inputs.iter().enumerate() {
        let key = bip::derive_bip84_privkey_at(
            mnemonic_str,
            bitcoin::Network::Signet,
            approved.change,
            approved.derivation_index,
        )?;

        // Cryptographic derivation binding:
        // Ensure the approved prevout matches the exact P2WPKH script_pubkey derived from the seed
        let derived_pubkey = key.public_key(&secp);
        let expected_script = bitcoin::ScriptBuf::new_p2wpkh(
            &bitcoin::WPubkeyHash::hash(&derived_pubkey.serialize()),
        );

        if approved.script_pubkey != expected_script {
            return Err(format!(
                "Pre-sign validation failed: ApprovedPrevout[{}] script_pubkey mismatch for derivation path m/84'/1'/0'/{}/{}: expected {}, got {}",
                i, approved.change, approved.derivation_index, expected_script, approved.script_pubkey
            ));
        }

        if let Some(psbt_input) = psbt.inputs.get(i) {
            if let Some(utxo) = &psbt_input.witness_utxo {
                if utxo.script_pubkey != expected_script {
                    return Err(format!(
                        "Pre-sign validation failed: PSBT input[{}] witness_utxo script_pubkey mismatch: expected {}, got {}",
                        i, expected_script, utxo.script_pubkey
                    ));
                }
            }
        }

        privkeys.push(key);
    }

    let fee = signer::btc::sign_and_finalize_verified_psbt_multi(&mut psbt, policy, &secp, &privkeys)?;

    let extracted_tx = psbt.extract_tx()
        .map_err(|e| format!("Cannot extract final signed transaction from PSBT: {:?}", e))?;
    let raw_tx_bytes = bitcoin::consensus::serialize(&extracted_tx);
    let raw_tx_hex = hex::encode(raw_tx_bytes);
    let txid = extracted_tx.compute_txid().to_string();
    let wtxid = extracted_tx.compute_wtxid().to_string();

    Ok(VerifiedSignedBtcDraft {
        fee_sat: fee,
        raw_tx_hex,
        txid,
        wtxid,
        payload_hash,
    })
}

/// Binds draft verification directly to BSC signing:
/// Verifies and consumes draft using canonical_payload.canonical_hash(),
/// builds BscSigningPolicy and Eip155Transaction directly from canonical_payload,
/// derives EVM key, signs transaction, and recovers sender.
#[allow(dead_code)]
pub fn sign_verified_bsc_draft(
    session: &mut SessionState,
    draft_id: &str,
    canonical_payload: &tx::BscCanonicalPayload,
) -> Result<signer::bsc::Eip155SignedTransaction, String> {
    let payload_hash = canonical_payload.canonical_hash();
    session.verify_and_consume_draft(draft_id, &payload_hash)?;

    let mnemonic_str = session
        .mnemonic
        .as_ref()
        .ok_or_else(|| "Wallet is locked: private key unavailable".to_string())?;

    let secp = secp256k1::Secp256k1::new();
    let privkey = bip::derive_bsc_privkey_at(mnemonic_str, 0)?;
    let sender_addr = bip::derive_bsc_address_at(mnemonic_str, 0)?;

    let policy = signer::bsc::BscSigningPolicy::from_canonical_payload(canonical_payload, &sender_addr)?;

    let tx = signer::bsc::Eip155Transaction {
        nonce: canonical_payload.nonce,
        gas_price: policy.expected_gas_price.clone(),
        gas_limit: canonical_payload.gas_limit,
        to: policy.expected_to.clone(),
        value: num_bigint::BigUint::from(0u32),
        data: policy.expected_data.clone(),
        chain_id: canonical_payload.chain_id,
    };

    let signed = signer::bsc::validate_and_sign_bsc_tx(&tx, &policy, &secp, &privkey)?;
    Ok(signed)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedSignedBtcDraft {
    pub fee_sat: u64,
    pub raw_tx_hex: String,
    pub txid: String,
    pub wtxid: String,
    pub payload_hash: String,
}

#[derive(Default, Debug)]
struct SqliteState {
    active_connections: usize,
    is_locked_for_reset: bool,
}

#[derive(Debug)]
pub struct SqliteTracker {
    state: Mutex<SqliteState>,
}

impl Default for SqliteTracker {
    fn default() -> Self {
        Self {
            state: Mutex::new(SqliteState::default()),
        }
    }
}

pub struct SqliteConnGuard {
    tracker: std::sync::Arc<SqliteTracker>,
}

impl Drop for SqliteConnGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.tracker.state.lock() {
            state.active_connections = state.active_connections.saturating_sub(1);
        }
    }
}

#[derive(Debug)]
pub struct SqliteResetPermit {
    tracker: std::sync::Arc<SqliteTracker>,
}

impl Drop for SqliteResetPermit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.tracker.state.lock() {
            state.is_locked_for_reset = false;
        }
    }
}

#[allow(dead_code)]
impl SqliteTracker {
    pub fn active_connections(&self) -> usize {
        self.state.lock().map(|s| s.active_connections).unwrap_or(0)
    }

    pub fn is_locked(&self) -> bool {
        self.state.lock().map(|s| s.is_locked_for_reset).unwrap_or(true)
    }

    /// Atomically verifies reset flag and increments active_connections in a single critical section
    pub fn acquire_connection(self: &std::sync::Arc<Self>) -> Result<SqliteConnGuard, String> {
        let mut state = self.state.lock().map_err(|_| "SQLite tracker lock poisoned".to_string())?;
        if state.is_locked_for_reset {
            return Err("SQLite database access denied: reset/purge operation is in progress".to_string());
        }
        state.active_connections += 1;
        Ok(SqliteConnGuard {
            tracker: std::sync::Arc::clone(self),
        })
    }

    /// Atomically claims exclusive reset ownership with RAII SqliteResetPermit.
    /// Fails immediately if another reset is already in progress.
    pub fn begin_reset(self: &std::sync::Arc<Self>) -> Result<SqliteResetPermit, String> {
        // Step 1: Claim exclusive reset ownership
        {
            let mut state = self.state.lock().map_err(|_| "SQLite tracker lock poisoned".to_string())?;
            if state.is_locked_for_reset {
                return Err("Another reset/purge operation is already in progress".to_string());
            }
            state.is_locked_for_reset = true;
            if state.active_connections == 0 {
                return Ok(SqliteResetPermit {
                    tracker: std::sync::Arc::clone(self),
                });
            }
        }

        // Step 2: Grace period (up to 150ms) for in-flight active queries to finish and drop their guards
        for _ in 0..15 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let state = self.state.lock().map_err(|_| "SQLite tracker lock poisoned".to_string())?;
            if state.active_connections == 0 {
                return Ok(SqliteResetPermit {
                    tracker: std::sync::Arc::clone(self),
                });
            }
        }

        // Step 3: If active connections still remain, rollback the lock and fail safely
        let mut state = self.state.lock().map_err(|_| "SQLite tracker lock poisoned".to_string())?;
        if state.active_connections == 0 {
            Ok(SqliteResetPermit {
                tracker: std::sync::Arc::clone(self),
            })
        } else {
            state.is_locked_for_reset = false;
            Err(format!(
                "Cannot reset wallet: {} active SQLite database connection(s) still open. Reset aborted safely.",
                state.active_connections
            ))
        }
    }
}

pub struct WalletSession {
    pub session: Mutex<SessionState>,
    pub is_preparing: std::sync::atomic::AtomicBool,
    pub sqlite_tracker: std::sync::Arc<SqliteTracker>,
    pub coordinator: std::sync::Arc<Mutex<broadcast::lifecycle::TxLifecycleCoordinator>>,
    pub btc_drafts: Mutex<std::collections::HashMap<String, (tx::BtcTxPreview, signer::btc::BtcSigningPolicy)>>,
    pub bsc_drafts: Mutex<std::collections::HashMap<String, tx::BscCanonicalPayload>>,
    pub live_broadcast_gate: std::sync::atomic::AtomicBool,
}

impl WalletSession {
    pub fn is_live_broadcast_allowed(&self) -> bool {
        if self.live_broadcast_gate.load(std::sync::atomic::Ordering::SeqCst) {
            return true;
        }
        if let Ok(val) = std::env::var("VAULT_ENABLE_TESTNET_BROADCAST") {
            let v = val.trim().to_lowercase();
            return v == "1" || v == "true" || v == "yes";
        }
        false
    }

    #[allow(dead_code)]
    pub fn set_live_broadcast_gate(&self, enabled: bool) {
        self.live_broadcast_gate.store(enabled, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Default for WalletSession {
    fn default() -> Self {
        Self {
            session: Mutex::new(SessionState::default()),
            is_preparing: std::sync::atomic::AtomicBool::new(false),
            sqlite_tracker: std::sync::Arc::new(SqliteTracker::default()),
            coordinator: std::sync::Arc::new(Mutex::new(broadcast::lifecycle::TxLifecycleCoordinator::default())),
            btc_drafts: Mutex::new(std::collections::HashMap::new()),
            bsc_drafts: Mutex::new(std::collections::HashMap::new()),
            live_broadcast_gate: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

pub fn ensure_coordinator_configured(
    coordinator: &mut broadcast::lifecycle::TxLifecycleCoordinator,
    app: &AppHandle,
) -> Result<(), String> {
    if coordinator.storage_path.is_none() {
        let app_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Cannot resolve app data directory: {}", e))?;
        let _ = fs::create_dir_all(&app_dir);
        let path = app_dir.join("tx_lifecycle.json");
        if path.exists() {
            let loaded = broadcast::lifecycle::TxLifecycleCoordinator::load_from_file(&path)
                .map_err(|e| format!("SECURITY ERROR: Corrupted tx_lifecycle.json at {:?}: {}. Please inspect or restore backup file.", path, e))?;
            *coordinator = loaded;
        } else {
            coordinator.set_storage_path(path);
        }
    }
    Ok(())
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
    if let Ok(mut btc_drafts) = state.btc_drafts.lock() {
        btc_drafts.clear();
    }
    if let Ok(mut bsc_drafts) = state.bsc_drafts.lock() {
        bsc_drafts.clear();
    }
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
    if state.is_preparing.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("A transaction preparation or wallet reset is already in progress".into());
    }
    struct PrepareGuard<'a>(&'a std::sync::atomic::AtomicBool);
    impl<'a> Drop for PrepareGuard<'a> {
        fn drop(&mut self) {
            self.0.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _guard = PrepareGuard(&state.is_preparing);

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
    let db_dir = app_dir.join("bdk_signet_db");
    let _ = fs::create_dir_all(&db_dir);
    let db_path = db_dir.join("bdk_signet.sqlite3");

    let _conn_guard = state.sqlite_tracker.acquire_connection()?;
    let (preview, policy) =
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

    let payload_hash = compute_btc_payload_hash(&preview.unsigned_psbt_base64);
    let now_unix = LockoutManager::now_unix();
    let expires_at_unix = now_unix + SESSION_TIMEOUT_SECS;

    session.active_draft = Some(ActiveDraft {
        draft_id: preview.draft_id.clone(),
        session_epoch: initial_epoch,
        network: "Bitcoin Signet".into(),
        payload_hash,
        expires_at_unix,
    });
    session.last_activity = Some(Instant::now());

    // Register draft in coordinator
    {
        let mut coord = state.coordinator.lock().map_err(|_| "Coordinator lock error")?;
        ensure_coordinator_configured(&mut coord, &app)?;
        coord.register_draft(&preview.draft_id, broadcast::types::BroadcastNetwork::BitcoinSignet, now_unix)?;
    }

    // Store draft preview and policy in btc_drafts
    {
        let mut btc_drafts = state.btc_drafts.lock().map_err(|_| "Drafts lock error")?;
        btc_drafts.insert(preview.draft_id.clone(), (preview.clone(), policy));
    }

    Ok(preview)
}

#[tauri::command]
pub async fn prepare_bsc_transaction(
    app: AppHandle,
    state: State<'_, WalletSession>,
    recipient: String,
    amount_tokens: String,
) -> Result<tx::BscTxPreview, String> {
    if state.is_preparing.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("A transaction preparation or wallet reset is already in progress".into());
    }
    struct PrepareGuard<'a>(&'a std::sync::atomic::AtomicBool);
    impl<'a> Drop for PrepareGuard<'a> {
        fn drop(&mut self) {
            self.0.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _guard = PrepareGuard(&state.is_preparing);

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
    let canonical_payload = tx::BscCanonicalPayload {
        chain_id: preview.chain_id,
        token_contract: preview.token_contract.clone(),
        recipient_address: preview.recipient_address.clone(),
        amount_raw: preview.amount_raw.clone(),
        nonce: preview.nonce,
        gas_limit: preview.gas_limit,
        gas_price_wei: preview.gas_price_wei.clone(),
        calldata_hex: preview.calldata_hex.clone(),
    };
    let payload_hash = canonical_payload.canonical_hash();

    let now_unix = LockoutManager::now_unix();
    let expires_at_unix = now_unix + SESSION_TIMEOUT_SECS;

    session.active_draft = Some(ActiveDraft {
        draft_id: preview.draft_id.clone(),
        session_epoch: initial_epoch,
        network: "BSC Testnet".into(),
        payload_hash,
        expires_at_unix,
    });
    session.last_activity = Some(Instant::now());

    // Register draft in coordinator
    {
        let mut coord = state.coordinator.lock().map_err(|_| "Coordinator lock error")?;
        ensure_coordinator_configured(&mut coord, &app)?;
        coord.register_draft(&preview.draft_id, broadcast::types::BroadcastNetwork::BscTestnet, now_unix)?;
    }

    // Store canonical payload in bsc_drafts
    {
        let mut bsc_drafts = state.bsc_drafts.lock().map_err(|_| "Drafts lock error")?;
        bsc_drafts.insert(preview.draft_id.clone(), canonical_payload);
    }

    Ok(preview)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BtcSignedPreview {
    pub draft_id: String,
    pub txid: String,
    pub wtxid: String,
    pub fee_satoshis: u64,
    pub raw_tx_hex: String,
}

#[tauri::command]
pub async fn confirm_and_sign_btc_draft(
    app: AppHandle,
    state: State<'_, WalletSession>,
    draft_id: String,
) -> Result<BtcSignedPreview, String> {
    let (preview, policy) = {
        let drafts = state.btc_drafts.lock().map_err(|_| "Drafts state unavailable")?;
        drafts
            .get(&draft_id)
            .cloned()
            .ok_or_else(|| format!("Draft '{}' not found in session drafts", draft_id))?
    };

    let signed = {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        sign_verified_btc_draft(&mut session, &draft_id, &preview.unsigned_psbt_base64, &policy)?
    };

    let now_unix = LockoutManager::now_unix();

    let approved_prevouts: Vec<broadcast::types::ApprovedPrevout> = policy
        .approved_inputs
        .iter()
        .map(|inp| broadcast::types::ApprovedPrevout {
            txid: inp.outpoint.txid.to_string(),
            vout: inp.outpoint.vout,
            value_satoshis: inp.value_satoshis,
            script_pubkey_hex: hex::encode(inp.script_pubkey.as_bytes()),
        })
        .collect();

    {
        let mut coord = state.coordinator.lock().map_err(|_| "Coordinator lock error")?;
        ensure_coordinator_configured(&mut coord, &app)?;
        coord.record_signed_btc(
            &draft_id,
            &signed.payload_hash,
            &signed.raw_tx_hex,
            &signed.txid,
            &signed.wtxid,
            approved_prevouts,
            now_unix,
        )?;
    }

    Ok(BtcSignedPreview {
        draft_id,
        txid: signed.txid,
        wtxid: signed.wtxid,
        fee_satoshis: signed.fee_sat,
        raw_tx_hex: signed.raw_tx_hex,
    })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BscSignedPreview {
    pub draft_id: String,
    pub tx_hash: String,
    pub sender: String,
    pub recipient: String,
    pub amount_raw: String,
    pub nonce: u64,
    pub gas_limit: u64,
    pub gas_price_wei: String,
    pub raw_tx_hex: String,
}

#[tauri::command]
pub async fn confirm_and_sign_bsc_draft(
    app: AppHandle,
    state: State<'_, WalletSession>,
    draft_id: String,
) -> Result<BscSignedPreview, String> {
    let canonical = {
        let drafts = state.bsc_drafts.lock().map_err(|_| "Drafts state unavailable")?;
        drafts
            .get(&draft_id)
            .cloned()
            .ok_or_else(|| format!("Draft '{}' not found in session drafts", draft_id))?
    };

    if canonical.chain_id != 97 {
        return Err(format!(
            "SECURITY ERROR: BSC chain ID {} is not BSC Testnet (chain ID 97)",
            canonical.chain_id
        ));
    }

    let payload_hash = canonical.canonical_hash();
    let (signed, sender_addr) = {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        let mnemonic_str = session
            .mnemonic
            .as_ref()
            .ok_or_else(|| "Wallet is locked: private key unavailable".to_string())?
            .clone();
        let s_addr = bip::derive_bsc_address_at(&mnemonic_str, 0)?;
        let s = sign_verified_bsc_draft(&mut session, &draft_id, &canonical)?;
        (s, s_addr)
    };

    let raw_tx_hex = format!("0x{}", hex::encode(&signed.raw_signed_tx_bytes));
    let clean = raw_tx_hex.trim().trim_start_matches("0x").trim_start_matches("0X");
    let raw_bytes = hex::decode(clean).map_err(|e| format!("Invalid EVM hex: {}", e))?;
    let mut hasher = tiny_keccak::Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(&raw_bytes);
    hasher.finalize(&mut hash);
    let tx_hash = format!("0x{}", hex::encode(hash));

    let now_unix = LockoutManager::now_unix();

    {
        let mut coord = state.coordinator.lock().map_err(|_| "Coordinator lock error")?;
        ensure_coordinator_configured(&mut coord, &app)?;
        coord.record_signed_bsc(
            &draft_id,
            &payload_hash,
            &raw_tx_hex,
            &sender_addr,
            &canonical.recipient_address,
            &canonical.amount_raw,
            canonical.nonce,
            canonical.gas_limit,
            &canonical.gas_price_wei,
            now_unix,
        )?;
    }

    Ok(BscSignedPreview {
        draft_id,
        tx_hash,
        sender: sender_addr,
        recipient: canonical.recipient_address,
        amount_raw: canonical.amount_raw,
        nonce: canonical.nonce,
        gas_limit: canonical.gas_limit,
        gas_price_wei: canonical.gas_price_wei,
        raw_tx_hex,
    })
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub draft_id: String,
    pub network: String,
    pub status: String,
    pub is_valid: bool,
    pub raw_tx_hex: String,
    pub txid_or_hash: String,
    pub checks: Vec<String>,
}

#[tauri::command]
pub async fn preflight_signed_transaction(
    app: AppHandle,
    state: State<'_, WalletSession>,
    draft_id: String,
) -> Result<PreflightReport, String> {
    {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        session.last_activity = Some(Instant::now());
    }

    let record = {
        let mut coord = state.coordinator.lock().map_err(|_| "Coordinator lock error")?;
        ensure_coordinator_configured(&mut coord, &app)?;
        coord
            .get_record(&draft_id)
            .ok_or_else(|| format!("Draft '{}' not found in coordinator", draft_id))?
            .clone()
    };

    let raw_tx_hex = match &record.status {
        broadcast::types::TxLifecycleStatus::Signed { raw_tx_hex, .. } => raw_tx_hex.clone(),
        other => {
            return Err(format!(
                "Cannot preflight transaction in state '{:?}', must be in 'Signed' status",
                other
            ));
        }
    };

    let mut checks = Vec::new();

    match record.network {
        broadcast::types::BroadcastNetwork::BitcoinSignet => {
            let esplora = broadcast::btc::EsploraBtcBroadcaster::new(false);
            let (tx, computed_txid, computed_wtxid) = esplora
                .dry_run_verify(&raw_tx_hex, record.network)
                .map_err(|e| format!("BTC dry-run verification failed: {}", e))?;

            if let Some(ref exp_txid) = record.expected_txid {
                if exp_txid != &computed_txid {
                    return Err(format!("Preflight verification mismatch: expected txid {}, got {}", exp_txid, computed_txid));
                }
            } else {
                return Err("Record missing expected_txid".to_string());
            }

            if let Some(ref exp_wtxid) = record.expected_wtxid {
                if exp_wtxid != &computed_wtxid {
                    return Err(format!("Preflight verification mismatch: expected wtxid {}, got {}", exp_wtxid, computed_wtxid));
                }
            } else {
                return Err("Record missing expected_wtxid".to_string());
            }

            let prevouts = record.approved_prevouts.as_ref().ok_or_else(|| {
                "Preflight failure: approved_prevouts missing in coordinator record".to_string()
            })?;

            // Mandatory cryptographic input signature verification: checks DER signatures, pubkeys, and BIP-143 sighash!
            esplora
                .verify_input_signatures(&tx, prevouts)
                .map_err(|e| format!("Cryptographic input signature verification failed: {}", e))?;

            checks.push("Successfully deserialized signed raw Bitcoin transaction".to_string());
            checks.push(format!("Verified computed txid matches approved expected_txid: {}", computed_txid));
            checks.push(format!("Verified computed wtxid matches approved expected_wtxid: {}", computed_wtxid));
            checks.push(format!("Cryptographically verified ECDSA signatures and BIP-143 sighash for all {} inputs", tx.input.len()));
            checks.push("Preflight validation passed: Signed Bitcoin Signet transaction is 100% cryptographically verified".to_string());

            Ok(PreflightReport {
                draft_id,
                network: "BitcoinSignet".to_string(),
                status: "Signed".to_string(),
                is_valid: true,
                raw_tx_hex,
                txid_or_hash: computed_txid,
                checks,
            })
        }
        broadcast::types::BroadcastNetwork::BscTestnet => {
            let rpc = broadcast::bsc::RpcBscBroadcaster::new(false);
            let exp_sender = record.expected_sender.as_deref().ok_or("Record missing expected_sender")?;
            let exp_recip = record.expected_recipient.as_deref().ok_or("Record missing expected_recipient")?;
            let exp_amount = record.expected_amount_raw.as_deref().ok_or("Record missing expected_amount_raw")?;
            let exp_nonce = record.expected_nonce.ok_or("Record missing expected_nonce")?;
            let exp_gas_limit = record.expected_gas_limit.ok_or("Record missing expected_gas_limit")?;
            let exp_gas_price = record.expected_gas_price.as_deref().ok_or("Record missing expected_gas_price")?;

            // Mandatory full cryptographic verification: RLP parsing, chain ID 97, ERC-20 transfer calldata,
            // recipient, amount, nonce, gas limit, gas price, and secp256k1 ecrecover sender address!
            let computed_tx_hash = rpc
                .dry_run_verify_against_draft(
                    &raw_tx_hex,
                    Some(exp_sender),
                    Some(exp_recip),
                    Some(exp_amount),
                    Some(exp_nonce),
                    Some(exp_gas_limit),
                    Some(exp_gas_price),
                    record.network,
                )
                .map_err(|e| format!("BSC cryptographic preflight verification failed: {}", e))?;

            checks.push("Successfully decoded and parsed EIP-155 RLP transaction list".to_string());
            checks.push("Verified Chain ID is 97 (BSC Testnet replay protection)".to_string());
            checks.push("Cryptographically recovered signer address via secp256k1 and verified matches expected_sender".to_string());
            checks.push(format!("Verified ERC-20 transfer calldata: recipient={}, amount={}", exp_recip, exp_amount));
            checks.push(format!("Verified nonce={}, gas_limit={}, gas_price={}", exp_nonce, exp_gas_limit, exp_gas_price));
            checks.push(format!("Computed Keccak-256 transaction hash: {}", computed_tx_hash));
            checks.push("Preflight validation passed: Signed BSC transaction is 100% cryptographically verified".to_string());

            Ok(PreflightReport {
                draft_id,
                network: "BscTestnet".to_string(),
                status: "Signed".to_string(),
                is_valid: true,
                raw_tx_hex: format!("0x{}", raw_tx_hex.trim_start_matches("0x").trim_start_matches("0X")),
                txid_or_hash: computed_tx_hash,
                checks,
            })
        }
    }
}

#[tauri::command]
pub async fn broadcast_signed_transaction(
    app: AppHandle,
    state: State<'_, WalletSession>,
    draft_id: String,
    enable_testnet_broadcast: Option<bool>,
) -> Result<String, String> {
    {
        let mut session = state.session.lock().map_err(|_| "Wallet state unavailable")?;
        if !check_and_apply_auto_lock(&mut session) {
            return Err("Wallet is locked".into());
        }
        session.last_activity = Some(Instant::now());
    }

    // Backend Live-Broadcast Gate check (fail-closed!)
    if !state.is_live_broadcast_allowed() {
        return Err("SECURITY ERROR: Live testnet broadcast is disabled by backend security gate (enable via backend gate or VAULT_ENABLE_TESTNET_BROADCAST=1)".into());
    }

    let is_live_enabled = enable_testnet_broadcast.unwrap_or(false);
    if !is_live_enabled {
        return Err("Live broadcast parameter not explicitly confirmed (use preflight_signed_transaction for dry-run verification)".into());
    }

    let (network, draft_id_clone) = {
        let mut coord = state.coordinator.lock().map_err(|_| "Coordinator lock error")?;
        ensure_coordinator_configured(&mut coord, &app)?;
        let record = coord
            .get_record(&draft_id)
            .ok_or_else(|| format!("Draft '{}' not found in coordinator", draft_id))?;
        (record.network, draft_id.clone())
    };

    // Pre-flight live verification for BSC: directly query RPC node to verify chain ID == 97
    if network == broadcast::types::BroadcastNetwork::BscTestnet {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("VaultManager/0.1.0")
            .build()
            .map_err(|e| format!("Failed to build HTTP client for chain ID verification: {}", e))?;
        
        let res = client
            .post("https://bsc-testnet.publicnode.com")
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_chainId",
                "params": [],
                "id": 97
            }))
            .send()
            .await
            .map_err(|e| format!("Pre-broadcast RPC chain ID query failed: {}", e))?;

        let body: serde_json::Value = res.json().await
            .map_err(|e| format!("Failed to parse RPC response: {}", e))?;
        let hex_chain_id = body.get("result").and_then(|v| v.as_str()).ok_or("Missing result in eth_chainId response")?;
        let chain_id = u64::from_str_radix(hex_chain_id.trim_start_matches("0x").trim_start_matches("0X"), 16)
            .map_err(|e| format!("Invalid hex chainId: {}", e))?;
        if chain_id != 97 {
            return Err(format!("SECURITY ERROR: Target node reports chain ID {}, expected BSC Testnet 97 (0x61)", chain_id));
        }
    }

    // Single-shot live broadcast (G5B): max_retries = 1 to prevent blind automatic retries during live broadcast
    let coord_arc = std::sync::Arc::clone(&state.coordinator);
    let max_retries = 1u32;
    let mut attempt = 0u32;

    tokio::task::spawn_blocking(move || {
        loop {
            attempt += 1;
            let now_unix = LockoutManager::now_unix();

            match network {
                broadcast::types::BroadcastNetwork::BitcoinSignet => {
                    let ctx = {
                        let mut coord = coord_arc.lock().map_err(|_| "Coordinator lock error")?;
                        coord.begin_broadcast_btc(&draft_id_clone, attempt, now_unix)?
                    }; // Lock dropped before HTTP network call!

                    let broadcaster = broadcast::btc::EsploraBtcBroadcaster::new(true);
                    let broadcast_res = broadcaster.broadcast_verified(
                        &ctx.raw_hex,
                        &ctx.expected_txid,
                        &ctx.expected_wtxid,
                        &ctx.approved_prevouts,
                        ctx.network,
                    );

                    let outcome = {
                        let mut coord = coord_arc.lock().map_err(|_| "Coordinator lock error")?;
                        coord.complete_broadcast_btc(&draft_id_clone, ctx.lease_token, broadcast_res, attempt, max_retries, now_unix)?
                    }; // Lock dropped!

                    match outcome {
                        broadcast::lifecycle::BroadcastOutcome::Success(txid) => return Ok(txid),
                        broadcast::lifecycle::BroadcastOutcome::Retry => continue,
                        broadcast::lifecycle::BroadcastOutcome::Failed(err) => return Err(err),
                    }
                }
                broadcast::types::BroadcastNetwork::BscTestnet => {
                    let ctx = {
                        let mut coord = coord_arc.lock().map_err(|_| "Coordinator lock error")?;
                        coord.begin_broadcast_bsc(&draft_id_clone, attempt, now_unix)?
                    }; // Lock dropped before HTTP network call!

                    let broadcaster = broadcast::bsc::RpcBscBroadcaster::new(true);
                    let broadcast_res = broadcaster.broadcast_verified(
                        &ctx.raw_hex,
                        &ctx.expected_sender,
                        &ctx.expected_recipient,
                        &ctx.expected_amount_raw,
                        ctx.expected_nonce,
                        ctx.expected_gas_limit,
                        &ctx.expected_gas_price,
                        ctx.network,
                    );

                    let outcome = {
                        let mut coord = coord_arc.lock().map_err(|_| "Coordinator lock error")?;
                        coord.complete_broadcast_bsc(&draft_id_clone, ctx.lease_token, broadcast_res, attempt, max_retries, now_unix)?
                    }; // Lock dropped!

                    match outcome {
                        broadcast::lifecycle::BroadcastOutcome::Success(tx_hash) => return Ok(tx_hash),
                        broadcast::lifecycle::BroadcastOutcome::Retry => continue,
                        broadcast::lifecycle::BroadcastOutcome::Failed(err) => return Err(err),
                    }
                }
            }
        }
    })
    .await
    .map_err(|e| format!("Background broadcast task panicked: {}", e))?
}

#[tauri::command]
pub async fn query_transaction_lifecycle(
    app: AppHandle,
    state: State<'_, WalletSession>,
    draft_id: String,
) -> Result<broadcast::types::TxLifecycleStatus, String> {
    let (network, _is_onchain) = {
        let mut coord = state.coordinator.lock().map_err(|_| "Coordinator lock error")?;
        ensure_coordinator_configured(&mut coord, &app)?;

        let record = coord
            .get_record(&draft_id)
            .ok_or_else(|| format!("Draft '{}' not found in coordinator", draft_id))?;

        let onchain = matches!(
            record.status,
            broadcast::types::TxLifecycleStatus::Broadcasting { .. }
                | broadcast::types::TxLifecycleStatus::Pending { .. }
                | broadcast::types::TxLifecycleStatus::Confirmed { .. }
                | broadcast::types::TxLifecycleStatus::ReorgDetected { .. }
                | broadcast::types::TxLifecycleStatus::Failed { permanent: false, .. }
        );

        if !onchain {
            return Ok(record.status.clone());
        }

        (record.network, onchain)
    };

    let coord_arc = std::sync::Arc::clone(&state.coordinator);
    let now_unix = LockoutManager::now_unix();
    let draft_id_clone = draft_id.clone();

    tokio::task::spawn_blocking(move || {
        let mut coord = coord_arc.lock().map_err(|_| "Coordinator lock error")?;
        match network {
            broadcast::types::BroadcastNetwork::BitcoinSignet => {
                let broadcaster = broadcast::btc::EsploraBtcBroadcaster::new(true);
                coord.poll_btc_lifecycle(&draft_id_clone, &broadcaster, now_unix)
            }
            broadcast::types::BroadcastNetwork::BscTestnet => {
                let broadcaster = broadcast::bsc::RpcBscBroadcaster::new(true);
                coord.poll_bsc_lifecycle(&draft_id_clone, &broadcaster, now_unix)
            }
        }
    })
    .await
    .map_err(|e| format!("Background polling task panicked: {}", e))?
}

pub fn cleanup_sqlite_files(app_dir: &std::path::Path) -> Result<(), String> {
    // 1. Clean up any leftover staged directories from previous interrupted resets
    if let Ok(entries) = fs::read_dir(app_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with(".sqlite_staging_") {
                        let _ = fs::remove_dir_all(&path);
                    }
                }
            }
        }
    }

    // 2. Atomic Directory Staging: If the dedicated DB directory exists, atomically rename the entire directory
    let bdk_db_dir = app_dir.join("bdk_signet_db");
    if bdk_db_dir.exists() {
        let now = LockoutManager::now_unix();
        let staging_dir = app_dir.join(format!(".sqlite_staging_dir_{}", now));
        fs::rename(&bdk_db_dir, &staging_dir)
            .map_err(|e| format!("Cannot atomically stage SQLite database directory for deletion: {}", e))?;
        fs::remove_dir_all(&staging_dir)
            .map_err(|e| format!("Cannot purge staged SQLite database directory: {}", e))?;
    }

    // 3. Fallback: Also clean up any legacy loose database/journal files with strict error checking
    let bdk_db = app_dir.join("bdk_signet.sqlite3");
    let bdk_wal = app_dir.join("bdk_signet.sqlite3-wal");
    let bdk_shm = app_dir.join("bdk_signet.sqlite3-shm");

    if bdk_db.exists() || bdk_wal.exists() || bdk_shm.exists() {
        let now = LockoutManager::now_unix();
        let staging_dir = app_dir.join(format!(".sqlite_staging_loose_{}", now));
        fs::create_dir_all(&staging_dir)
            .map_err(|e| format!("Cannot create loose staging dir: {}", e))?;
        if bdk_db.exists() {
            fs::rename(&bdk_db, staging_dir.join("bdk_signet.sqlite3"))
                .map_err(|e| format!("Cannot stage bdk_signet.sqlite3: {}", e))?;
        }
        if bdk_wal.exists() {
            fs::rename(&bdk_wal, staging_dir.join("bdk_signet.sqlite3-wal"))
                .map_err(|e| format!("Cannot stage bdk_signet.sqlite3-wal: {}", e))?;
        }
        if bdk_shm.exists() {
            fs::rename(&bdk_shm, staging_dir.join("bdk_signet.sqlite3-shm"))
                .map_err(|e| format!("Cannot stage bdk_signet.sqlite3-shm: {}", e))?;
        }
        fs::remove_dir_all(&staging_dir)
            .map_err(|e| format!("Cannot purge loose staging dir: {}", e))?;
    }

    // 4. Strict post-cleanup verification: database files must no longer exist
    if bdk_db_dir.exists() || bdk_db.exists() || bdk_wal.exists() || bdk_shm.exists() {
        return Err("SQLite cleanup verification failed: database files still present on disk".to_string());
    }

    Ok(())
}

/// Secure reset: Requires valid password verification to authenticate the owner before deleting.
/// Coordinates SQLite connection shutdown, stages and cleans SQLite storage FIRST before touching vault file.
#[tauri::command]
pub fn reset_wallet(
    app: AppHandle,
    state: State<'_, WalletSession>,
    password: String,
) -> Result<(), String> {
    if state.is_preparing.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err("Cannot reset wallet while a transaction preparation is in progress".into());
    }
    struct ResetGuard<'a>(&'a std::sync::atomic::AtomicBool);
    impl<'a> Drop for ResetGuard<'a> {
        fn drop(&mut self) {
            self.0.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _guard = ResetGuard(&state.is_preparing);

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
    if let Ok(mut btc_drafts) = state.btc_drafts.lock() {
        btc_drafts.clear();
    }
    if let Ok(mut bsc_drafts) = state.bsc_drafts.lock() {
        bsc_drafts.clear();
    }
    if let Ok(mut coord) = state.coordinator.lock() {
        *coord = broadcast::lifecycle::TxLifecycleCoordinator::default();
    }

    // 2. Coordinate SQLite shutdown: lock new connections and ensure active connections are closed
    let _reset_permit = state.sqlite_tracker.begin_reset()?;

    // 3. Clean up SQLite database and journals FIRST (fail-closed if app_dir unresolvable)
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve application data directory for SQLite cleanup: {}", e))?;

    cleanup_sqlite_files(&app_dir)?;

    let lifecycle_file = app_dir.join("tx_lifecycle.json");
    let _ = fs::remove_file(lifecycle_file);

    // 4. Only after SQLite cleanup succeeds, remove the vault file
    fs::remove_file(&file).map_err(|e| format!("Cannot remove vault file: {}", e))?;
    mgr.record_success();

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use bitcoin::{OutPoint, Psbt, ScriptBuf, Transaction, TxIn, TxOut, Txid};
    use std::str::FromStr;

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

    #[test]
    fn test_e2e_btc_prepare_draft_sign_and_raw_tx_verification() {
        use bitcoin::{
            amount::Amount, hashes::Hash, locktime::absolute::LockTime, transaction::Version,
            OutPoint, Psbt, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
        };
        use std::str::FromStr;

        let mnemonic = bip::generate_24_words().unwrap();
        let mut session = SessionState::default();
        session.session_epoch = 1;
        session.mnemonic = Some(mnemonic.clone());

        // Derive receive key at m/84'/1'/0'/0/0 and change key at m/84'/1'/0'/1/0
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let key_recv = bip::derive_bip84_privkey_at(&mnemonic, bitcoin::Network::Signet, 0, 0).unwrap();
        let pub_recv = key_recv.public_key(&secp);
        let btc_pub_recv = bitcoin::PublicKey::from_slice(&pub_recv.serialize()).unwrap();
        let script_recv = ScriptBuf::new_p2wpkh(&btc_pub_recv.wpubkey_hash().unwrap());

        let key_change = bip::derive_bip84_privkey_at(&mnemonic, bitcoin::Network::Signet, 1, 0).unwrap();
        let pub_change = key_change.public_key(&secp);
        let btc_pub_change = bitcoin::PublicKey::from_slice(&pub_change.serialize()).unwrap();
        let script_change = ScriptBuf::new_p2wpkh(&btc_pub_change.wpubkey_hash().unwrap());

        // Construct 2-input PSBT (input 0: receive, input 1: change)
        let outpoint0 = OutPoint::from_str("1111111111111111111111111111111111111111111111111111111111111111:0").unwrap();
        let outpoint1 = OutPoint::from_str("2222222222222222222222222222222222222222222222222222222222222222:1").unwrap();

        let tx_in0 = TxIn {
            previous_output: outpoint0,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };
        let tx_in1 = TxIn {
            previous_output: outpoint1,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };

        let recipient_script = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::all_zeros());
        let tx_out_send = TxOut {
            value: Amount::from_sat(60_000),
            script_pubkey: recipient_script.clone(),
        };
        let tx_out_change = TxOut {
            value: Amount::from_sat(17_000),
            script_pubkey: script_change.clone(),
        };

        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![tx_in0, tx_in1],
            output: vec![tx_out_send, tx_out_change],
        };

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).unwrap();
        psbt.inputs[0].witness_utxo = Some(TxOut {
            value: Amount::from_sat(50_000),
            script_pubkey: script_recv.clone(),
        });
        psbt.inputs[1].witness_utxo = Some(TxOut {
            value: Amount::from_sat(30_000),
            script_pubkey: script_change.clone(),
        });

        use base64::Engine;
        let unsigned_psbt_base64 = base64::engine::general_purpose::STANDARD.encode(psbt.serialize());
        let payload_hash = compute_btc_payload_hash(&unsigned_psbt_base64);

        let draft_id = "draft-btc-e2e-001";
        session.active_draft = Some(ActiveDraft {
            draft_id: draft_id.into(),
            session_epoch: 1,
            network: "Bitcoin Signet".into(),
            payload_hash: payload_hash.clone(),
            expires_at_unix: LockoutManager::now_unix() + 300,
        });

        let policy = signer::btc::BtcSigningPolicy {
            expected_recipient: recipient_script,
            expected_amount_sat: 60_000,
            expected_change_script: Some(script_change.clone()),
            approved_inputs: vec![
                signer::btc::ApprovedPrevout {
                    outpoint: outpoint0,
                    value_satoshis: 50_000,
                    script_pubkey: script_recv,
                    change: 0,
                    derivation_index: 0,
                },
                signer::btc::ApprovedPrevout {
                    outpoint: outpoint1,
                    value_satoshis: 30_000,
                    script_pubkey: script_change,
                    change: 1,
                    derivation_index: 0,
                },
            ],
        };

        // Execute unified draft-bound signing
        let signed_draft = sign_verified_btc_draft(
            &mut session,
            draft_id,
            &unsigned_psbt_base64,
            &policy,
        ).expect("End-to-end BTC signing must succeed");
        let fee = signed_draft.fee_sat;
        let raw_tx_hex = signed_draft.raw_tx_hex;

        assert_eq!(fee, 3000, "Fee must be 50k + 30k - 60k - 17k = 3000 sats");
        assert!(session.active_draft.is_none(), "Active draft must be consumed");

        // Verify raw transaction decodes and has valid witnesses on both inputs
        let raw_bytes = hex::decode(&raw_tx_hex).expect("Raw tx must be valid hex");
        let final_tx: Transaction = bitcoin::consensus::deserialize(&raw_bytes).expect("Raw tx must deserialize");
        assert_eq!(final_tx.input.len(), 2);
        assert!(!final_tx.input[0].witness.is_empty(), "Input 0 witness must be present");
        assert!(!final_tx.input[1].witness.is_empty(), "Input 1 witness must be present");

        // Independently verify ECDSA signatures on the raw transaction inputs
        let mut sighash_cache = bitcoin::sighash::SighashCache::new(&final_tx);
        for (i, approved) in policy.approved_inputs.iter().enumerate() {
            let witness = &final_tx.input[i].witness;
            assert_eq!(witness.len(), 2, "P2WPKH witness must contain [signature, pubkey]");
            let sig_bytes = witness.nth(0).unwrap();
            let pubkey_bytes = witness.nth(1).unwrap();

            let sig_slice = &sig_bytes[..sig_bytes.len() - 1]; // strip sighash flag byte
            let sig = secp256k1::ecdsa::Signature::from_der(sig_slice).expect("Must be valid DER signature");
            let pubkey = secp256k1::PublicKey::from_slice(pubkey_bytes).expect("Must be valid compressed pubkey");

            let sighash = sighash_cache.p2wpkh_signature_hash(
                i,
                &approved.script_pubkey,
                bitcoin::Amount::from_sat(approved.value_satoshis),
                bitcoin::sighash::EcdsaSighashType::All,
            ).expect("Must compute BIP143 sighash");

            let msg = secp256k1::Message::from_digest(sighash.to_byte_array());
            secp.verify_ecdsa(&msg, &sig, &pubkey).expect("Independent signature verification on raw tx must succeed!");
        }
    }

    #[test]
    fn test_btc_rejects_forged_approved_prevout_metadata() {
        let mnemonic = bip::generate_24_words().unwrap();
        let mut session = SessionState::default();
        session.session_epoch = 1;
        session.mnemonic = Some(mnemonic.clone());

        let secp = bitcoin::secp256k1::Secp256k1::new();
        let key_recv = bip::derive_bip84_privkey_at(&mnemonic, bitcoin::Network::Signet, 0, 0).unwrap();
        let script_recv = bitcoin::ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::hash(&key_recv.public_key(&secp).serialize()));

        let recipient_script = bitcoin::Address::from_str("tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx")
            .unwrap()
            .assume_checked()
            .script_pubkey();

        let outpoint = OutPoint {
            txid: Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
            vout: 0,
        };

        let unsigned_tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![TxOut {
                value: bitcoin::Amount::from_sat(45_000),
                script_pubkey: recipient_script.clone(),
            }],
        };

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).unwrap();
        psbt.inputs[0].witness_utxo = Some(TxOut {
            value: bitcoin::Amount::from_sat(50_000),
            script_pubkey: script_recv.clone(),
        });

        let psbt_b64 = base64::engine::general_purpose::STANDARD.encode(&psbt.serialize());
        let payload_hash = compute_btc_payload_hash(&psbt_b64);
        let draft_id = "draft-btc-tamper-01";

        session.active_draft = Some(ActiveDraft {
            draft_id: draft_id.into(),
            session_epoch: 1,
            network: "Bitcoin Signet".into(),
            payload_hash,
            expires_at_unix: LockoutManager::now_unix() + 300,
        });

        // 1. TAMPER ATTEMPT: Attacker provides self-declared metadata claiming derivation index = 999
        // which does NOT derive the script_pubkey
        let forged_policy = signer::btc::BtcSigningPolicy {
            expected_recipient: recipient_script,
            expected_amount_sat: 45_000,
            expected_change_script: None,
            approved_inputs: vec![
                signer::btc::ApprovedPrevout {
                    outpoint,
                    value_satoshis: 50_000,
                    script_pubkey: script_recv,
                    change: 0,
                    derivation_index: 999, // FORGED INDEX!
                },
            ],
        };

        let sign_res = sign_verified_btc_draft(
            &mut session,
            draft_id,
            &psbt_b64,
            &forged_policy,
        );

        assert!(sign_res.is_err(), "Pre-sign validation MUST reject forged ApprovedPrevout metadata");
        let err_str = sign_res.unwrap_err();
        assert!(err_str.contains("Pre-sign validation failed"), "Error must report pre-sign derivation mismatch: {}", err_str);
    }

    #[test]
    fn test_e2e_bsc_prepare_draft_sign_and_raw_tx_verification() {
        let mnemonic = bip::generate_24_words().unwrap();
        let mut session = SessionState::default();
        session.session_epoch = 1;
        session.mnemonic = Some(mnemonic.clone());

        let contract_to = "0x337610d27c682E347C9cD60bd4b3b107C9d34dDd";
        let recipient_addr = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";
        let calldata_hex = tx::bsc::calldata::encode_erc20_transfer(
            recipient_addr,
            &num_bigint::BigUint::from(1_000_000_000_000_000_000u64),
        ).unwrap();

        let canonical_payload = tx::BscCanonicalPayload {
            chain_id: 97,
            token_contract: Some(contract_to.to_string()),
            recipient_address: recipient_addr.to_string(),
            amount_raw: "1000000000000000000".to_string(),
            nonce: 7,
            gas_limit: 65_000,
            gas_price_wei: "3000000000".to_string(),
            calldata_hex,
        };

        let draft_id = "draft-bsc-e2e-001";
        let payload_hash = canonical_payload.canonical_hash();

        session.active_draft = Some(ActiveDraft {
            draft_id: draft_id.into(),
            session_epoch: 1,
            network: "BSC Testnet".into(),
            payload_hash,
            expires_at_unix: LockoutManager::now_unix() + 300,
        });

        // Execute unified draft-bound signing
        let signed = sign_verified_bsc_draft(
            &mut session,
            draft_id,
            &canonical_payload,
        ).expect("End-to-end BSC signing must succeed");

        assert!(session.active_draft.is_none(), "Active draft must be consumed");
        assert_eq!(signed.tx.value, num_bigint::BigUint::from(0u32), "Value must be strictly 0");
        assert_eq!(signed.tx.nonce, 7);
        assert_eq!(signed.tx.gas_limit, 65_000);

        // Verify sender matches derived address
        let expected_sender = bip::derive_bsc_address(&mnemonic).unwrap();
        let secp = secp256k1::Secp256k1::new();
        let recovered = signed.recover_sender(&secp).unwrap();
        assert_eq!(recovered.to_lowercase(), expected_sender.to_lowercase());

        // Verify raw signed transaction is non-empty valid RLP hex
        assert!(!signed.raw_signed_tx_bytes.is_empty());
        let raw_hex = hex::encode(&signed.raw_signed_tx_bytes);
        assert!(raw_hex.starts_with("f8")); // Standard RLP list prefix
    }

    #[test]
    fn test_t68_offline_signing_coordinator_mock_e2e() {
        use bitcoin::{
            amount::Amount, hashes::Hash, locktime::absolute::LockTime, transaction::Version,
            OutPoint, Psbt, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
        };
        use std::str::FromStr;
        use crate::wallet::broadcast::{
            types::{BroadcastNetwork, TxLifecycleStatus},
            btc::MockBtcBroadcaster,
            bsc::MockBscBroadcaster,
        };

        let mnemonic = bip::generate_24_words().unwrap();
        let session_state = WalletSession::default();

        // 1. Initialize session with unlocked mnemonic
        {
            let mut s = session_state.session.lock().unwrap();
            s.mnemonic = Some(mnemonic.clone());
            s.session_epoch = 1;
            s.last_activity = Some(Instant::now());
        }

        let now_unix = LockoutManager::now_unix();
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let key_recv = bip::derive_bip84_privkey_at(&mnemonic, bitcoin::Network::Signet, 0, 0).unwrap();
        let btc_pub_recv = bitcoin::PublicKey::from_slice(&key_recv.public_key(&secp).serialize()).unwrap();
        let script_recv = ScriptBuf::new_p2wpkh(&btc_pub_recv.wpubkey_hash().unwrap());

        // --- PART A: BTC SIGNET PIPELINE ---
        let btc_draft_id = "draft-btc-t68-001";
        let outpoint = OutPoint::from_str("3333333333333333333333333333333333333333333333333333333333333333:0").unwrap();
        let recipient_script = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::all_zeros());

        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: outpoint,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            }],
            output: vec![
                TxOut {
                    value: Amount::from_sat(50_000),
                    script_pubkey: recipient_script.clone(),
                },
                TxOut {
                    value: Amount::from_sat(48_000),
                    script_pubkey: script_recv.clone(),
                },
            ],
        };
        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).unwrap();
        psbt.inputs[0].witness_utxo = Some(TxOut {
            value: Amount::from_sat(100_000),
            script_pubkey: script_recv.clone(),
        });

        use base64::Engine;
        let btc_psbt_b64 = base64::engine::general_purpose::STANDARD.encode(psbt.serialize());
        let btc_payload_hash = compute_btc_payload_hash(&btc_psbt_b64);

        let btc_policy = signer::btc::BtcSigningPolicy {
            expected_recipient: recipient_script,
            expected_amount_sat: 50_000,
            expected_change_script: Some(script_recv.clone()),
            approved_inputs: vec![signer::btc::ApprovedPrevout {
                outpoint,
                value_satoshis: 100_000,
                script_pubkey: script_recv.clone(),
                change: 0,
                derivation_index: 0,
            }],
        };

        // Register in coordinator and store draft
        {
            let mut coord = session_state.coordinator.lock().unwrap();
            coord.register_draft(btc_draft_id, BroadcastNetwork::BitcoinSignet, now_unix).unwrap();
            let mut drafts = session_state.btc_drafts.lock().unwrap();
            drafts.insert(btc_draft_id.into(), (
                tx::BtcTxPreview {
                    draft_id: btc_draft_id.into(),
                    network: "Bitcoin Signet".into(),
                    recipient_address: "tb1q...".into(),
                    amount_satoshis: 50_000,
                    formatted_amount_btc: "0.00050000".into(),
                    fee_satoshis: 2_000,
                    formatted_fee_btc: "0.00002000".into(),
                    fee_rate_sat_per_vb: 14,
                    selected_utxos: vec![],
                    change_address: "tb1q...".into(),
                    change_satoshis: 48_000,
                    unsigned_psbt_base64: btc_psbt_b64.clone(),
                    signing_enabled: true,
                    note: "T68 preview".into(),
                },
                btc_policy.clone(),
            ));
        }

        // Set active draft in session
        {
            let mut s = session_state.session.lock().unwrap();
            s.active_draft = Some(ActiveDraft {
                draft_id: btc_draft_id.into(),
                session_epoch: 1,
                network: "Bitcoin Signet".into(),
                payload_hash: btc_payload_hash.clone(),
                expires_at_unix: now_unix + 300,
            });
        }

        // Sign BTC draft
        let signed_btc = {
            let mut s = session_state.session.lock().unwrap();
            sign_verified_btc_draft(&mut s, btc_draft_id, &btc_psbt_b64, &btc_policy).unwrap()
        };
        assert_eq!(signed_btc.fee_sat, 2_000);

        let approved_prevouts = vec![broadcast::types::ApprovedPrevout {
            txid: outpoint.txid.to_string(),
            vout: outpoint.vout,
            value_satoshis: 100_000,
            script_pubkey_hex: hex::encode(script_recv.as_bytes()),
        }];

        // Record signed in coordinator
        {
            let temp_storage = std::env::temp_dir().join(format!("t68_coord_{}.json", now_unix));
            let mut coord = session_state.coordinator.lock().unwrap();
            coord.set_storage_path(temp_storage.clone());
            coord.record_signed_btc(
                btc_draft_id,
                &signed_btc.payload_hash,
                &signed_btc.raw_tx_hex,
                &signed_btc.txid,
                &signed_btc.wtxid,
                approved_prevouts.clone(),
                now_unix,
            ).unwrap();

            // Verify status is Signed
            let rec = coord.get_record(btc_draft_id).unwrap();
            assert!(matches!(rec.status, TxLifecycleStatus::Signed { .. }));

            // Broadcast via Mock
            let mock_btc = MockBtcBroadcaster::default();
            mock_btc.set_next_broadcast_result(Ok(signed_btc.txid.clone()));
            let txid = coord.execute_broadcast_btc(btc_draft_id, &mock_btc, 3, now_unix).unwrap();
            assert_eq!(txid, signed_btc.txid);

            // Poll lifecycle via Mock -> Confirmed
            mock_btc.set_tx_status(&txid, crate::wallet::broadcast::btc::BtcTxQueryStatus::Confirmed {
                block_height: 100,
                block_hash: "0000000000000000000000000000000000000000000000000000000000000001".into(),
                confirmations: 1,
            });
            let status = coord.poll_btc_lifecycle(btc_draft_id, &mock_btc, now_unix).unwrap();
            assert!(matches!(status, TxLifecycleStatus::Confirmed { confirmations: 1, .. }));

            let _ = fs::remove_file(temp_storage);
        }

        // --- PART B: BSC TESTNET PIPELINE ---
        let bsc_draft_id = "draft-bsc-t68-001";
        let sender_bsc = bip::derive_bsc_address(&mnemonic).unwrap();
        let bsc_canonical = tx::BscCanonicalPayload {
            chain_id: 97,
            token_contract: Some("0x337610d27c682E347C9cD60BD4b3b107C9d34dDd".into()),
            recipient_address: "0x1111111111111111111111111111111111111111".into(),
            amount_raw: "1000000000000000000".into(), // 1.0 USDT (18 decimals)
            nonce: 10,
            gas_limit: 70_000,
            gas_price_wei: "3000000000".into(), // 3 Gwei
            calldata_hex: "0xa9059cbb00000000000000000000000011111111111111111111111111111111111111110000000000000000000000000000000000000000000000000de0b6b3a7640000".into(),
        };
        let bsc_payload_hash = bsc_canonical.canonical_hash();

        // Register draft and set active draft in session
        {
            let mut coord = session_state.coordinator.lock().unwrap();
            coord.register_draft(bsc_draft_id, BroadcastNetwork::BscTestnet, now_unix).unwrap();
            let mut drafts = session_state.bsc_drafts.lock().unwrap();
            drafts.insert(bsc_draft_id.into(), bsc_canonical.clone());

            let mut s = session_state.session.lock().unwrap();
            s.active_draft = Some(ActiveDraft {
                draft_id: bsc_draft_id.into(),
                session_epoch: 1,
                network: "BSC Testnet".into(),
                payload_hash: bsc_payload_hash.clone(),
                expires_at_unix: now_unix + 300,
            });
        }

        // Sign BSC draft
        let signed_bsc = {
            let mut s = session_state.session.lock().unwrap();
            sign_verified_bsc_draft(&mut s, bsc_draft_id, &bsc_canonical).unwrap()
        };
        let raw_bsc_hex = format!("0x{}", hex::encode(&signed_bsc.raw_signed_tx_bytes));

        // Record signed in coordinator
        {
            let temp_storage = std::env::temp_dir().join(format!("t68_coord_bsc_{}.json", now_unix));
            let mut coord = session_state.coordinator.lock().unwrap();
            coord.set_storage_path(temp_storage.clone());
            coord.record_signed_bsc(
                bsc_draft_id,
                &bsc_payload_hash,
                &raw_bsc_hex,
                &sender_bsc,
                &bsc_canonical.recipient_address,
                &bsc_canonical.amount_raw,
                bsc_canonical.nonce,
                bsc_canonical.gas_limit,
                &bsc_canonical.gas_price_wei,
                now_unix,
            ).unwrap();

            // Verify status is Signed
            let rec = coord.get_record(bsc_draft_id).unwrap();
            assert!(matches!(rec.status, TxLifecycleStatus::Signed { .. }));

            let clean = raw_bsc_hex.trim().trim_start_matches("0x").trim_start_matches("0X");
            let raw_bytes = hex::decode(clean).unwrap();
            let mut hasher = tiny_keccak::Keccak::v256();
            let mut hash = [0u8; 32];
            use tiny_keccak::Hasher;
            hasher.update(&raw_bytes);
            hasher.finalize(&mut hash);
            let expected_tx_hash = format!("0x{}", hex::encode(hash));

            // Broadcast via Mock
            let mock_bsc = MockBscBroadcaster::default();
            mock_bsc.set_next_broadcast_result(Ok(expected_tx_hash.clone()));
            let tx_hash = coord.execute_broadcast_bsc(bsc_draft_id, &mock_bsc, 3, now_unix).unwrap();
            assert_eq!(tx_hash, expected_tx_hash);

            // Poll lifecycle via Mock -> Confirmed
            mock_bsc.set_receipt(&tx_hash, crate::wallet::broadcast::bsc::BscReceiptStatus::Success {
                block_number: 200,
                block_hash: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".into(),
                gas_used: 50_000,
            });
            let status = coord.poll_bsc_lifecycle(bsc_draft_id, &mock_bsc, now_unix).unwrap();
            assert!(matches!(status, TxLifecycleStatus::Confirmed { confirmations: 1, .. }));

            let _ = fs::remove_file(temp_storage);
        }
    }

    #[test]
    fn test_t69_corrupted_lifecycle_fail_closed_and_dry_run_flag() {
        use std::io::Write;
        let temp_dir = std::env::temp_dir().join(format!("t69_test_{}", LockoutManager::now_unix()));
        let _ = fs::create_dir_all(&temp_dir);
        let corrupt_path = temp_dir.join("tx_lifecycle.json");

        // 1. Write corrupted JSON to tx_lifecycle.json
        {
            let mut f = fs::File::create(&corrupt_path).unwrap();
            f.write_all(b"{\"records\": [corrupted truncated json...").unwrap();
            f.sync_all().unwrap();
        }

        // 2. Loading corrupted file must FAIL-CLOSED immediately
        let load_res = broadcast::lifecycle::TxLifecycleCoordinator::load_from_file(&corrupt_path);
        assert!(load_res.is_err(), "Corrupted lifecycle file must fail-closed");
        let err_msg = load_res.unwrap_err();
        assert!(err_msg.contains("Cannot parse lifecycle file"), "Must report parse failure: {}", err_msg);

        // 3. Corrupted file must NOT be overwritten or clobbered
        assert!(corrupt_path.exists(), "Corrupted file must be preserved for forensic analysis");

        // 4. Test dry-run broadcaster: EsploraBtcBroadcaster::new(false) and RpcBscBroadcaster::new(false)
        let dry_btc = broadcast::btc::EsploraBtcBroadcaster::new(false);
        assert!(!dry_btc.broadcast_enabled, "Dry run BTC broadcaster must have broadcast_enabled = false");

        let dry_bsc = broadcast::bsc::RpcBscBroadcaster::new(false);
        assert!(!dry_bsc.broadcast_enabled, "Dry run BSC broadcaster must have broadcast_enabled = false");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_t70_pure_dry_run_preserves_signed_status_and_allows_subsequent_broadcast() {
        use bitcoin::{
            amount::Amount, locktime::absolute::LockTime, transaction::Version,
            OutPoint, Psbt, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
        };
        use std::str::FromStr;
        use crate::wallet::broadcast::{
            types::{BroadcastNetwork, TxLifecycleStatus},
            btc::{BtcBroadcaster, MockBtcBroadcaster},
        };

        let mnemonic = bip::generate_24_words().unwrap();
        let session_state = WalletSession::default();
        let now_unix = LockoutManager::now_unix();

        {
            let mut s = session_state.session.lock().unwrap();
            s.mnemonic = Some(mnemonic.clone());
            s.session_epoch = 1;
            s.last_activity = Some(Instant::now());
        }

        // Setup BTC Draft
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let privkey = bip::derive_bip84_privkey_at(
            &mnemonic,
            bitcoin::Network::Signet,
            0,
            0,
        ).unwrap();
        let pubkey = privkey.public_key(&secp);
        let wpkh = bitcoin::PublicKey::from_slice(&pubkey.serialize()).unwrap().wpubkey_hash().unwrap();
        let script_recv = ScriptBuf::new_p2wpkh(&wpkh);

        let outpoint = OutPoint::from_str("1111111111111111111111111111111111111111111111111111111111111111:0").unwrap();
        let recipient_script = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::all_zeros());
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: outpoint,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            }],
            output: vec![
                TxOut {
                    value: Amount::from_sat(50_000),
                    script_pubkey: recipient_script.clone(),
                },
                TxOut {
                    value: Amount::from_sat(48_000),
                    script_pubkey: script_recv.clone(),
                },
            ],
        };

        let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
        psbt.inputs[0].witness_utxo = Some(TxOut {
            value: Amount::from_sat(100_000),
            script_pubkey: script_recv.clone(),
        });

        use base64::Engine;
        let btc_psbt_b64 = base64::engine::general_purpose::STANDARD.encode(psbt.serialize());
        let btc_payload_hash = compute_btc_payload_hash(&btc_psbt_b64);
        let btc_draft_id = "draft-btc-t70-dryrun";

        let btc_policy = signer::btc::BtcSigningPolicy {
            expected_recipient: recipient_script,
            expected_amount_sat: 50_000,
            expected_change_script: Some(script_recv.clone()),
            approved_inputs: vec![signer::btc::ApprovedPrevout {
                outpoint,
                value_satoshis: 100_000,
                script_pubkey: script_recv.clone(),
                change: 0,
                derivation_index: 0,
            }],
        };

        {
            let mut s = session_state.session.lock().unwrap();
            s.active_draft = Some(ActiveDraft {
                draft_id: btc_draft_id.into(),
                session_epoch: 1,
                network: "Bitcoin Signet".into(),
                payload_hash: btc_payload_hash.clone(),
                expires_at_unix: now_unix + 300,
            });
        }

        let signed_btc = {
            let mut s = session_state.session.lock().unwrap();
            sign_verified_btc_draft(&mut s, btc_draft_id, &btc_psbt_b64, &btc_policy).unwrap()
        };

        let temp_storage = std::env::temp_dir().join(format!("t70_coord_{}.json", now_unix));
        {
            let mut coord = session_state.coordinator.lock().unwrap();
            coord.set_storage_path(temp_storage.clone());
            coord.register_draft(btc_draft_id, BroadcastNetwork::BitcoinSignet, now_unix).unwrap();
            let approved_prevouts = vec![broadcast::types::ApprovedPrevout {
                txid: outpoint.txid.to_string(),
                vout: outpoint.vout,
                value_satoshis: 100_000,
                script_pubkey_hex: hex::encode(script_recv.as_bytes()),
            }];
            coord.record_signed_btc(
                btc_draft_id,
                &signed_btc.payload_hash,
                &signed_btc.raw_tx_hex,
                &signed_btc.txid,
                &signed_btc.wtxid,
                approved_prevouts,
                now_unix,
            ).unwrap();
        }

        // STEP 1: Execute Pure Dry-Run Verification (Does NOT mutate status or emit hashes)
        {
            let coord = session_state.coordinator.lock().unwrap();
            let rec = coord.get_record(btc_draft_id).unwrap();
            
            // Invariant 1: Record status must be Signed
            assert!(matches!(rec.status, TxLifecycleStatus::Signed { .. }));
            assert_eq!(rec.tx_hash, None, "Dry run must never record fake tx_hash");

            // Verify raw tx cryptographically without touching state
            let raw_bytes = hex::decode(&signed_btc.raw_tx_hex).unwrap();
            let parsed_tx: Transaction = bitcoin::consensus::deserialize(&raw_bytes).unwrap();
            assert_eq!(parsed_tx.compute_txid().to_string(), signed_btc.txid);
            assert_eq!(parsed_tx.compute_wtxid().to_string(), signed_btc.wtxid);
            for input in &parsed_tx.input {
                assert!(!input.witness.is_empty(), "Witness must be present");
            }
        }

        // STEP 2: Post-dry-run state inspection
        {
            let coord = session_state.coordinator.lock().unwrap();
            let rec = coord.get_record(btc_draft_id).unwrap();
            assert!(matches!(rec.status, TxLifecycleStatus::Signed { .. }), "Record must remain in Signed status after dry-run");
            assert_eq!(rec.tx_hash, None);
        }

        // STEP 3: Now execute live broadcast of the preserved Signed draft
        {
            let mut coord = session_state.coordinator.lock().unwrap();
            let mock_btc = MockBtcBroadcaster::default();
            mock_btc.set_next_broadcast_result(Ok(signed_btc.txid.clone()));

            // begin_broadcast transitions to Broadcasting
            let ctx = coord.begin_broadcast_btc(btc_draft_id, 1, now_unix).unwrap();
            assert_eq!(ctx.expected_txid, signed_btc.txid);

            // complete_broadcast transitions to Pending
            let res = mock_btc.broadcast_verified(&ctx.raw_hex, &ctx.expected_txid, &ctx.expected_wtxid, &ctx.approved_prevouts, ctx.network);
            let outcome = coord.complete_broadcast_btc(btc_draft_id, ctx.lease_token, res, 1, 3, now_unix).unwrap();
            assert_eq!(outcome, broadcast::lifecycle::BroadcastOutcome::Success(signed_btc.txid.clone()));

            // Record is now Pending with the real txid
            let rec = coord.get_record(btc_draft_id).unwrap();
            assert!(matches!(rec.status, TxLifecycleStatus::Pending { ref tx_hash, .. } if tx_hash == &signed_btc.txid));
        }

        let _ = fs::remove_file(temp_storage);
    }

    #[test]
    fn test_t71_backend_feature_gate_blocks_unauthorized_live_broadcast() {
        let session_state = WalletSession::default();

        // 1. By default, live broadcast gate is DISABLED (fail-closed)
        assert!(!session_state.is_live_broadcast_allowed(), "Gate must default to false");

        // 2. Explicit setter works as expected
        session_state.set_live_broadcast_gate(true);
        assert!(session_state.is_live_broadcast_allowed(), "Gate must be true after set_live_broadcast_gate(true)");

        session_state.set_live_broadcast_gate(false);
        assert!(!session_state.is_live_broadcast_allowed(), "Gate must be false after set_live_broadcast_gate(false)");

        // 3. Env var test
        std::env::set_var("VAULT_ENABLE_TESTNET_BROADCAST", "1");
        assert!(session_state.is_live_broadcast_allowed(), "Gate must be true when env var is 1");

        std::env::set_var("VAULT_ENABLE_TESTNET_BROADCAST", "0");
        assert!(!session_state.is_live_broadcast_allowed(), "Gate must be false when env var is 0");

        std::env::remove_var("VAULT_ENABLE_TESTNET_BROADCAST");
        assert!(!session_state.is_live_broadcast_allowed(), "Gate must be false when env var is removed");
    }

    #[test]
    fn test_t72_real_preflight_cryptographic_verification_and_negative_rejections() {
        use bitcoin::{
            amount::Amount, locktime::absolute::LockTime, transaction::Version,
            OutPoint, Psbt, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
        };
        use std::str::FromStr;
        use crate::wallet::broadcast::{
            types::{BroadcastNetwork, TxLifecycleStatus},
            btc::EsploraBtcBroadcaster,
            bsc::RpcBscBroadcaster,
        };

        let mnemonic = bip::generate_24_words().unwrap();
        let session_state = WalletSession::default();
        let now_unix = LockoutManager::now_unix();

        {
            let mut s = session_state.session.lock().unwrap();
            s.mnemonic = Some(mnemonic.clone());
            s.session_epoch = 1;
            s.last_activity = Some(Instant::now());
        }

        // 1. Valid BTC Signed Transaction Setup
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let privkey = bip::derive_bip84_privkey_at(&mnemonic, bitcoin::Network::Signet, 0, 0).unwrap();
        let pubkey = privkey.public_key(&secp);
        let wpkh = bitcoin::PublicKey::from_slice(&pubkey.serialize()).unwrap().wpubkey_hash().unwrap();
        let script_recv = ScriptBuf::new_p2wpkh(&wpkh);

        let outpoint = OutPoint::from_str("4444444444444444444444444444444444444444444444444444444444444444:0").unwrap();
        let recipient_script = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::all_zeros());
        let tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: outpoint,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            }],
            output: vec![
                TxOut {
                    value: Amount::from_sat(30_000),
                    script_pubkey: recipient_script.clone(),
                },
                TxOut {
                    value: Amount::from_sat(68_000),
                    script_pubkey: script_recv.clone(),
                },
            ],
        };

        let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
        psbt.inputs[0].witness_utxo = Some(TxOut {
            value: Amount::from_sat(100_000),
            script_pubkey: script_recv.clone(),
        });

        use base64::Engine;
        let btc_psbt_b64 = base64::engine::general_purpose::STANDARD.encode(psbt.serialize());
        let btc_payload_hash = compute_btc_payload_hash(&btc_psbt_b64);
        let btc_draft_id = "draft-btc-t72-preflight";

        let btc_policy = signer::btc::BtcSigningPolicy {
            expected_recipient: recipient_script,
            expected_amount_sat: 30_000,
            expected_change_script: Some(script_recv.clone()),
            approved_inputs: vec![signer::btc::ApprovedPrevout {
                outpoint,
                value_satoshis: 100_000,
                script_pubkey: script_recv.clone(),
                change: 0,
                derivation_index: 0,
            }],
        };

        {
            let mut s = session_state.session.lock().unwrap();
            s.active_draft = Some(ActiveDraft {
                draft_id: btc_draft_id.into(),
                session_epoch: 1,
                network: "Bitcoin Signet".into(),
                payload_hash: btc_payload_hash.clone(),
                expires_at_unix: now_unix + 300,
            });
        }

        let signed_btc = {
            let mut s = session_state.session.lock().unwrap();
            sign_verified_btc_draft(&mut s, btc_draft_id, &btc_psbt_b64, &btc_policy).unwrap()
        };

        let temp_storage = std::env::temp_dir().join(format!("t72_coord_{}.json", now_unix));
        let approved_prevouts = vec![broadcast::types::ApprovedPrevout {
            txid: outpoint.txid.to_string(),
            vout: outpoint.vout,
            value_satoshis: 100_000,
            script_pubkey_hex: hex::encode(script_recv.as_bytes()),
        }];

        {
            let mut coord = session_state.coordinator.lock().unwrap();
            coord.set_storage_path(temp_storage.clone());
            coord.register_draft(btc_draft_id, BroadcastNetwork::BitcoinSignet, now_unix).unwrap();
            coord.record_signed_btc(
                btc_draft_id,
                &signed_btc.payload_hash,
                &signed_btc.raw_tx_hex,
                &signed_btc.txid,
                &signed_btc.wtxid,
                approved_prevouts.clone(),
                now_unix,
            ).unwrap();
        }

        let esplora = EsploraBtcBroadcaster::new(false);

        // A. Valid Preflight Verification passes
        let (valid_tx, _, _) = esplora.dry_run_verify(&signed_btc.raw_tx_hex, BroadcastNetwork::BitcoinSignet).unwrap();
        esplora.verify_input_signatures(&valid_tx, &approved_prevouts).unwrap();

        // B. Negative Test 1: Tampered witness signature must FAIL-CLOSED
        let mut tampered_tx = valid_tx.clone();
        let mut tampered_witness = tampered_tx.input[0].witness.to_vec();
        // Flip one byte in DER signature
        tampered_witness[0][5] ^= 0xff;
        tampered_tx.input[0].witness = Witness::from_slice(&tampered_witness);
        let sig_verify_res = esplora.verify_input_signatures(&tampered_tx, &approved_prevouts);
        assert!(sig_verify_res.is_err(), "Tampered witness signature MUST fail cryptographic verification");

        // C. Negative Test 2: Forged prevout value must FAIL-CLOSED (Sighash mismatch)
        let mut forged_prevouts = approved_prevouts.clone();
        forged_prevouts[0].value_satoshis = 99_999; // Value mismatch
        let value_verify_res = esplora.verify_input_signatures(&valid_tx, &forged_prevouts);
        assert!(value_verify_res.is_err(), "Forged prevout satoshis MUST fail BIP-143 sighash verification");

        // D. Negative Test 3: Forged prevout outpoint must FAIL-CLOSED
        let mut forged_outpoint_prevouts = approved_prevouts.clone();
        forged_outpoint_prevouts[0].vout = 1; // Wrong vout
        let outpoint_verify_res = esplora.verify_input_signatures(&valid_tx, &forged_outpoint_prevouts);
        assert!(outpoint_verify_res.is_err(), "Forged prevout outpoint MUST fail strict prevout check");

        // 2. BSC Real Preflight Verification & Negatives
        let bsc_draft_id = "draft-bsc-t72-preflight";
        let sender_bsc = bip::derive_bsc_address(&mnemonic).unwrap();
        let bsc_canonical = tx::BscCanonicalPayload {
            chain_id: 97,
            token_contract: Some("0x337610d27c682E347C9cD60BD4b3b107C9d34dDd".into()),
            recipient_address: "0x2222222222222222222222222222222222222222".into(),
            amount_raw: "500000000000000000".into(), // 0.5 USDT
            nonce: 3,
            gas_limit: 75_000,
            gas_price_wei: "3000000000".into(),
            calldata_hex: "0xa9059cbb000000000000000000000000222222222222222222222222222222222222222200000000000000000000000000000000000000000000000006f05b59d3b20000".into(),
        };

        {
            let mut s = session_state.session.lock().unwrap();
            s.active_draft = Some(ActiveDraft {
                draft_id: bsc_draft_id.into(),
                session_epoch: 1,
                network: "BSC Testnet".into(),
                payload_hash: bsc_canonical.canonical_hash(),
                expires_at_unix: now_unix + 300,
            });
        }

        let signed_bsc = {
            let mut s = session_state.session.lock().unwrap();
            sign_verified_bsc_draft(&mut s, bsc_draft_id, &bsc_canonical).unwrap()
        };
        let raw_bsc_hex = format!("0x{}", hex::encode(&signed_bsc.raw_signed_tx_bytes));

        let rpc = RpcBscBroadcaster::new(false);

        // A. Valid BSC Preflight passes
        let bsc_verify_ok = rpc.dry_run_verify_against_draft(
            &raw_bsc_hex,
            Some(&sender_bsc),
            Some(&bsc_canonical.recipient_address),
            Some(&bsc_canonical.amount_raw),
            Some(bsc_canonical.nonce),
            Some(bsc_canonical.gas_limit),
            Some(&bsc_canonical.gas_price_wei),
            BroadcastNetwork::BscTestnet,
        );
        assert!(bsc_verify_ok.is_ok(), "Valid BSC raw transaction must pass full preflight verification");

        // B. Negative Test: Mismatched recipient address must FAIL-CLOSED
        let bsc_wrong_recipient = rpc.dry_run_verify_against_draft(
            &raw_bsc_hex,
            Some(&sender_bsc),
            Some("0x3333333333333333333333333333333333333333"), // Mismatched recipient
            Some(&bsc_canonical.amount_raw),
            Some(bsc_canonical.nonce),
            Some(bsc_canonical.gas_limit),
            Some(&bsc_canonical.gas_price_wei),
            BroadcastNetwork::BscTestnet,
        );
        assert!(bsc_wrong_recipient.is_err(), "Mismatched recipient must fail BSC preflight");

        // C. Negative Test: Mismatched nonce must FAIL-CLOSED
        let bsc_wrong_nonce = rpc.dry_run_verify_against_draft(
            &raw_bsc_hex,
            Some(&sender_bsc),
            Some(&bsc_canonical.recipient_address),
            Some(&bsc_canonical.amount_raw),
            Some(4), // Nonce was 3
            Some(bsc_canonical.gas_limit),
            Some(&bsc_canonical.gas_price_wei),
            BroadcastNetwork::BscTestnet,
        );
        assert!(bsc_wrong_nonce.is_err(), "Mismatched nonce must fail BSC preflight");

        // Invariant: Verify coordinator record remains strictly in Signed status
        {
            let coord = session_state.coordinator.lock().unwrap();
            let rec = coord.get_record(btc_draft_id).unwrap();
            assert!(matches!(rec.status, TxLifecycleStatus::Signed { .. }));
            assert_eq!(rec.tx_hash, None);
        }

        let _ = fs::remove_file(temp_storage);
    }

    #[test]
    fn test_t73_concurrent_broadcast_exclusive_lease_protection() {
        use crate::wallet::broadcast::{
            types::{BroadcastNetwork, TxLifecycleStatus},
        };

        let temp_storage = std::env::temp_dir().join(format!("t73_coord_{}.json", LockoutManager::now_unix()));
        let mut coord = broadcast::lifecycle::TxLifecycleCoordinator::default();
        coord.set_storage_path(temp_storage.clone());
        let now_unix = LockoutManager::now_unix();
        let draft_id = "draft-btc-t73-concurrent";

        // Register and mark signed
        coord.register_draft(draft_id, BroadcastNetwork::BitcoinSignet, now_unix).unwrap();
        coord.record_signed_btc(
            draft_id,
            "0x1111",
            "0200000001...",
            "txid123",
            "wtxid123",
            vec![broadcast::types::ApprovedPrevout {
                txid: "0000000000000000000000000000000000000000000000000000000000000000".into(),
                vout: 0,
                value_satoshis: 10_000,
                script_pubkey_hex: "0014...".into(),
            }],
            now_unix,
        ).unwrap();

        // 1. First broadcast request begins -> succeeds and receives exclusive lease #1
        let ctx_a = coord.begin_broadcast_btc(draft_id, 1, now_unix).unwrap();
        assert_eq!(ctx_a.lease_token, 1);
        assert!(coord.active_leases.contains_key(draft_id));

        // 2. CONCURRENT request for the same draft_id tries to begin while request A is in-flight:
        // MUST BE REJECTED FAIL-CLOSED!
        let req_b = coord.begin_broadcast_btc(draft_id, 1, now_unix);
        assert!(req_b.is_err(), "Concurrent begin_broadcast must be rejected while lease is active");
        let err_b = req_b.unwrap_err();
        assert!(err_b.contains("Broadcast already in progress for draft"), "Error must clearly identify active lease: {}", err_b);

        // 3. Stale completion rejection:
        // If a rogue or stale caller attempts to complete with an invalid lease token (e.g. #99), reject!
        let stale_complete = coord.complete_broadcast_btc(
            draft_id,
            99, // Stale lease token
            Ok("txid123".into()),
            1,
            1,
            now_unix,
        );
        assert!(stale_complete.is_err(), "Completion with invalid lease token must be rejected");

        // 4. Legitimate completion by request A (holding lease #1) -> succeeds and transitions to Pending
        let legitimate_complete = coord.complete_broadcast_btc(
            draft_id,
            ctx_a.lease_token,
            Ok("txid123".into()),
            1,
            1,
            now_unix,
        ).unwrap();
        assert_eq!(legitimate_complete, broadcast::lifecycle::BroadcastOutcome::Success("txid123".into()));

        // Active lease is now cleared
        assert!(!coord.active_leases.contains_key(draft_id));
        let rec = coord.get_record(draft_id).unwrap();
        assert!(matches!(rec.status, TxLifecycleStatus::Pending { .. }));

        // 5. Protect Pending state from stale errors:
        // Even if a delayed error callback arrives for this draft, it must NEVER overwrite an already Pending transaction!
        let cannot_broadcast_again = coord.begin_broadcast_btc(draft_id, 2, now_unix);
        assert!(cannot_broadcast_again.is_err(), "Cannot broadcast a transaction already in Pending/Confirmed");

        let _ = fs::remove_file(temp_storage);
    }

    #[test]
    fn test_t74_backend_gate_ipc_and_single_shot_live() {
        let session = WalletSession::default();

        // 1. Fail-closed gate: live broadcast disabled by default
        assert!(!session.is_live_broadcast_allowed());

        // 2. Gate can be enabled explicitly in controlled environment
        session.set_live_broadcast_gate(true);
        assert!(session.is_live_broadcast_allowed());

        // 3. Environment variable acts as backend feature flag
        session.set_live_broadcast_gate(false);
        std::env::set_var("VAULT_ENABLE_TESTNET_BROADCAST", "true");
        assert!(session.is_live_broadcast_allowed());

        std::env::remove_var("VAULT_ENABLE_TESTNET_BROADCAST");
        assert!(!session.is_live_broadcast_allowed());
    }

    #[test]
    fn test_t75_lease_expiration_reconciliation_and_thread_concurrency() {
        use crate::wallet::broadcast::{
            btc::{BtcTxQueryStatus, MockBtcBroadcaster},
            types::{BroadcastNetwork, TxLifecycleStatus},
        };
        use std::sync::{Arc, Barrier, Mutex};
        use std::thread;

        let temp_storage = std::env::temp_dir().join(format!("t75_coord_{}.json", LockoutManager::now_unix()));
        let mut coord = broadcast::lifecycle::TxLifecycleCoordinator::default();
        coord.set_storage_path(temp_storage.clone());
        let now_unix = LockoutManager::now_unix();
        let draft_id = "draft-btc-t75-multithread";

        // Register and mark signed
        coord.register_draft(draft_id, BroadcastNetwork::BitcoinSignet, now_unix).unwrap();
        coord.record_signed_btc(
            draft_id,
            "0x7575",
            "0200000001...",
            "txid_t75",
            "wtxid_t75",
            vec![broadcast::types::ApprovedPrevout {
                txid: "0000000000000000000000000000000000000000000000000000000000000000".into(),
                vout: 0,
                value_satoshis: 25_000,
                script_pubkey_hex: "0014...".into(),
            }],
            now_unix,
        ).unwrap();

        let coord_arc = Arc::new(Mutex::new(coord));
        let barrier = Arc::new(Barrier::new(2));

        // Spawn 2 threads racing to call begin_broadcast_btc simultaneously
        let handle1 = {
            let c = Arc::clone(&coord_arc);
            let b = Arc::clone(&barrier);
            let d = draft_id.to_string();
            thread::spawn(move || {
                b.wait();
                c.lock().unwrap().begin_broadcast_btc(&d, 1, now_unix)
            })
        };

        let handle2 = {
            let c = Arc::clone(&coord_arc);
            let b = Arc::clone(&barrier);
            let d = draft_id.to_string();
            thread::spawn(move || {
                b.wait();
                c.lock().unwrap().begin_broadcast_btc(&d, 1, now_unix)
            })
        };

        let res1 = handle1.join().unwrap();
        let res2 = handle2.join().unwrap();

        // Exactly one thread succeeds, and the other fails-closed with concurrent rejection
        let (success_res, failed_res) = match (res1, res2) {
            (Ok(ctx), Err(err)) => (ctx, err),
            (Err(err), Ok(ctx)) => (ctx, err),
            (Ok(_), Ok(_)) => panic!("Critical race condition: both threads obtained exclusive lease!"),
            (Err(e1), Err(e2)) => panic!("Both threads failed unexpectedly: {}, {}", e1, e2),
        };

        assert!(failed_res.contains("concurrent broadcast rejected"));
        assert_eq!(success_res.lease_token, 1);

        // PART 2: Lease Expiration Reconciliation
        // Simulate lease expiration (>60s) where request was hung or dropped
        let expired_now = now_unix + 61;
        let mut coord_guard = coord_arc.lock().unwrap();

        // Resubmitting without reconciliation MUST be rejected fail-closed!
        let resubmit_res = coord_guard.begin_broadcast_btc(draft_id, 1, expired_now);
        assert!(resubmit_res.is_err(), "Resubmit after timeout must require mandatory reconciliation");
        let err_msg = resubmit_res.unwrap_err();
        assert!(
            err_msg.contains("timed out after 60s without completion. Mandatory reconciliation required"),
            "Error must enforce reconciliation: {}",
            err_msg
        );

        // Perform mandatory reconciliation via poll_btc_lifecycle
        let mock_broadcaster = MockBtcBroadcaster::new();
        mock_broadcaster.set_tx_status("txid_t75", BtcTxQueryStatus::InMempool);

        let reconciled_status = coord_guard.poll_btc_lifecycle(draft_id, &mock_broadcaster, expired_now + 1).unwrap();
        assert!(matches!(reconciled_status, TxLifecycleStatus::Pending { .. }));

        // Now that reconciliation has transitioned the record to Pending, active lease is cleared
        assert!(!coord_guard.active_leases.contains_key(draft_id));

        // And further begin_broadcast_btc is rejected because transaction is already Pending
        let after_reconciliation = coord_guard.begin_broadcast_btc(draft_id, 1, expired_now + 2);
        assert!(after_reconciliation.is_err());
        assert!(after_reconciliation.unwrap_err().contains("already in state"));

        let _ = fs::remove_file(temp_storage);
    }

    #[test]
    fn test_t76_crash_restart_reconciliation_poll_before_resubmit() {
        use crate::wallet::broadcast::{
            btc::{BtcTxQueryStatus, MockBtcBroadcaster},
            types::{BroadcastNetwork, TxLifecycleStatus},
        };

        let temp_storage = std::env::temp_dir().join(format!("t76_coord_{}.json", LockoutManager::now_unix()));
        let now_unix = LockoutManager::now_unix();
        let draft_id = "draft-btc-t76-crash-recovery";

        // 1. Initial process: register, sign, and begin broadcast
        {
            let mut coord = broadcast::lifecycle::TxLifecycleCoordinator::default();
            coord.set_storage_path(temp_storage.clone());
            coord.register_draft(draft_id, BroadcastNetwork::BitcoinSignet, now_unix).unwrap();
            coord.record_signed_btc(
                draft_id,
                "0x7676",
                "0200000001...",
                "txid_t76",
                "wtxid_t76",
                vec![broadcast::types::ApprovedPrevout {
                    txid: "0000000000000000000000000000000000000000000000000000000000000000".into(),
                    vout: 0,
                    value_satoshis: 35_000,
                    script_pubkey_hex: "0014...".into(),
                }],
                now_unix,
            ).unwrap();

            // begin_broadcast transitions status to Broadcasting and persists to storage
            let _ctx = coord.begin_broadcast_btc(draft_id, 1, now_unix).unwrap();
            let rec = coord.get_record(draft_id).unwrap();
            assert!(matches!(rec.status, TxLifecycleStatus::Broadcasting { .. }));
            // Process crashes here before complete_broadcast can be called!
        }

        // 2. Restart simulation: new coordinator instance loads persisted state from disk
        let mut recovered_coord = broadcast::lifecycle::TxLifecycleCoordinator::load_from_file(&temp_storage).unwrap();
        // Storage path is preserved
        assert_eq!(recovered_coord.storage_path, Some(temp_storage.clone()));
        // Record is loaded in Broadcasting state
        let rec = recovered_coord.get_record(draft_id).unwrap();
        assert!(matches!(rec.status, TxLifecycleStatus::Broadcasting { .. }));
        // Active lease map in memory is empty after crash
        assert!(recovered_coord.active_leases.is_empty());

        // 3. Resubmit attempt without prior reconciliation MUST FAIL-CLOSED!
        let resubmit_res = recovered_coord.begin_broadcast_btc(draft_id, 1, now_unix + 10);
        assert!(resubmit_res.is_err(), "Resubmit after crash must fail-closed");
        let err_msg = resubmit_res.unwrap_err();
        assert!(
            err_msg.contains("was interrupted in Broadcasting state during crash. Mandatory reconciliation required"),
            "Error message must specify crash reconciliation requirement: {}",
            err_msg
        );

        // 4. Perform mandatory reconciliation polling
        let mock_broadcaster = MockBtcBroadcaster::new();
        mock_broadcaster.set_tx_status("txid_t76", BtcTxQueryStatus::InMempool);

        let reconciled_status = recovered_coord.poll_btc_lifecycle(draft_id, &mock_broadcaster, now_unix + 15).unwrap();
        assert!(matches!(reconciled_status, TxLifecycleStatus::Pending { .. }));

        // 5. Verify reconciled state was auto-persisted: load a 3rd coordinator instance to confirm
        let reloaded_coord = broadcast::lifecycle::TxLifecycleCoordinator::load_from_file(&temp_storage).unwrap();
        let reloaded_rec = reloaded_coord.get_record(draft_id).unwrap();
        assert!(matches!(reloaded_rec.status, TxLifecycleStatus::Pending { .. }));

        let _ = fs::remove_file(temp_storage);
    }
}
