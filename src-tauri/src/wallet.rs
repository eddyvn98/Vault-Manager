use std::{fs, sync::Mutex};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use base64::{engine::general_purpose::STANDARD, Engine};
use bip39::{Language, Mnemonic};
use chacha20poly1305::{aead::{Aead, KeyInit, OsRng, rand_core::RngCore}, XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use zeroize::Zeroizing;

const FILE_NAME: &str = "wallet-v1.enc";
const AAD: &[u8] = b"VaultManager:local:wallet:v1";
const MAX_PASSWORD_BYTES: usize = 1024;
const MIN_PASSWORD_CHARS: usize = 12;

#[derive(Default)]
pub struct WalletSession(Mutex<Option<Zeroizing<String>>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletStatus { exists: bool, unlocked: bool, signing_enabled: bool, network: &'static str }

#[derive(Serialize, Deserialize)]
struct Envelope { version: u8, salt: String, nonce: String, ciphertext: String }

fn path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|_| "Cannot resolve application data directory".to_string())?;
    Ok(dir.join(FILE_NAME))
}

fn phrase(input: &str) -> Result<Zeroizing<String>, String> {
    let m = Mnemonic::parse_in(Language::English, input.trim()).map_err(|_| "Invalid recovery phrase".to_string())?;
    if m.word_count() != 24 { return Err("Only 24-word recovery phrases are supported".into()); }
    Ok(Zeroizing::new(m.to_string()))
}

fn password_ok(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_CHARS || password.len() > MAX_PASSWORD_BYTES {
        return Err("Password must be at least 12 characters and at most 1024 bytes".into());
    }
    Ok(())
}

fn derive_key(password: &str, salt_bytes: &[u8]) -> Result<Zeroizing<[u8;32]>, String> {
    let mut result = Zeroizing::new([0u8; 32]);
    // OWASP-aligned explicit Argon2id parameters; benchmark on low-memory Android devices.
    let params = argon2::Params::new(64 * 1024, 3, 1, Some(32)).map_err(|_| "Invalid KDF parameters")?;
    let argon = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    argon.hash_password_into(password.as_bytes(), salt_bytes, &mut *result)
        .map_err(|_| "Key derivation failed")?;
    Ok(result)
}

fn encrypt(password: &str, recovery: &str) -> Result<Vec<u8>, String> {
    password_ok(password)?;
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let ciphertext = cipher.encrypt(XNonce::from_slice(&nonce),
        chacha20poly1305::aead::Payload { msg: recovery.as_bytes(), aad: AAD })
        .map_err(|_| "Encryption failed")?;
    serde_json::to_vec(&Envelope {
        version: 1, salt: STANDARD.encode(salt), nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext)
    }).map_err(|_| "Cannot encode vault".into())
}

fn decrypt(password: &str, raw: &[u8]) -> Result<Zeroizing<String>, String> {
    if raw.len() > 16_384 { return Err("Invalid vault file".into()); }
    let env: Envelope = serde_json::from_slice(raw).map_err(|_| "Invalid vault file")?;
    if env.version != 1 { return Err("Unsupported vault version".into()); }
    let salt = STANDARD.decode(env.salt).map_err(|_| "Invalid vault file")?;
    let nonce = STANDARD.decode(env.nonce).map_err(|_| "Invalid vault file")?;
    let ciphertext = STANDARD.decode(env.ciphertext).map_err(|_| "Invalid vault file")?;
    if salt.len() != 16 || nonce.len() != 24 { return Err("Invalid vault file".into()); }
    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let plaintext = Zeroizing::new(cipher.decrypt(XNonce::from_slice(&nonce),
        chacha20poly1305::aead::Payload { msg: &ciphertext, aad: AAD })
        .map_err(|_| "Incorrect password or corrupted vault".to_string())?);
    let text = Zeroizing::new(String::from_utf8(plaintext.to_vec()).map_err(|_| "Invalid vault content".to_string())?);
    phrase(&text)
}

fn write_new(app: &AppHandle, password: &str, recovery: &str) -> Result<(), String> {
    let bytes = encrypt(password, recovery)?;
    let file = path(app)?;
    let dir = file.parent().ok_or("Invalid vault path")?;
    fs::create_dir_all(dir).map_err(|_| "Cannot create vault folder")?;
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut handle = options.open(&file).map_err(|_| "Vault already exists or cannot be written")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&file, fs::Permissions::from_mode(0o600));
    }
    if handle.write_all(&bytes).and_then(|_| handle.sync_all()).is_err() {
        drop(handle);
        let _ = fs::remove_file(&file);
        return Err("Unable to persist vault".into());
    }
    Ok(())
}

#[tauri::command]
pub fn wallet_status(app: AppHandle, state: State<'_, WalletSession>) -> Result<WalletStatus, String> {
    let unlocked = state.0.lock().map_err(|_| "Wallet state unavailable")?.is_some();
    Ok(WalletStatus { exists: path(&app)?.exists(), unlocked, signing_enabled: false, network: "offline-only" })
}

#[tauri::command]
pub fn create_wallet(app: AppHandle, state: State<'_, WalletSession>, password: String) -> Result<String, String> {
    if path(&app)?.exists() { return Err("A wallet already exists on this device".into()); }
    password_ok(&password)?;
    let recovery = Zeroizing::new(Mnemonic::generate_in(Language::English, 24)
        .map_err(|_| "Secure entropy generation failed")?.to_string());
    write_new(&app, &password, &recovery)?;
    *state.0.lock().map_err(|_| "Wallet state unavailable")? = None;
    // The UI must ask the owner to write this phrase down before navigating away.
    Ok(recovery.to_string())
}

#[tauri::command]
pub fn restore_wallet(app: AppHandle, state: State<'_, WalletSession>, password: String, recovery: String) -> Result<(), String> {
    if path(&app)?.exists() { return Err("Vault exists; restore cannot overwrite it".into()); }
    let valid = phrase(&recovery)?;
    write_new(&app, &password, &valid)?;
    *state.0.lock().map_err(|_| "Wallet state unavailable")? = None;
    Ok(())
}

#[tauri::command]
pub fn unlock_wallet(app: AppHandle, state: State<'_, WalletSession>, password: String) -> Result<(), String> {
    let raw = fs::read(path(&app)?).map_err(|_| "No vault found on this device")?;
    let recovery = decrypt(&password, &raw)?;
    *state.0.lock().map_err(|_| "Wallet state unavailable")? = Some(recovery);
    Ok(())
}

#[tauri::command]
pub fn lock_wallet(state: State<'_, WalletSession>) -> Result<(), String> {
    *state.0.lock().map_err(|_| "Wallet state unavailable")? = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encryption_roundtrip_and_wrong_password() {
        let mnemonic = Mnemonic::generate_in(Language::English, 24).unwrap().to_string();
        let encrypted = encrypt("safe-password-123", &mnemonic).unwrap();
        assert_eq!(&*decrypt("safe-password-123", &encrypted).unwrap(), &mnemonic);
        assert!(decrypt("wrong-password-123", &encrypted).is_err());
    }
    #[test]
    fn encryption_is_randomized_and_tamper_evident() {
        let mnemonic = Mnemonic::generate_in(Language::English, 24).unwrap().to_string();
        let a = encrypt("safe-password-123", &mnemonic).unwrap();
        let b = encrypt("safe-password-123", &mnemonic).unwrap();
        assert_ne!(a, b);
        let mut envelope: Envelope = serde_json::from_slice(&a).unwrap();
        envelope.ciphertext.replace_range(0..1, "X");
        assert!(decrypt("safe-password-123", &serde_json::to_vec(&envelope).unwrap()).is_err());
    }
    #[test]
    fn recovery_validates_checksum_and_length() {
        assert!(phrase("not a valid phrase").is_err());
        assert!(password_ok("short").is_err());
    }
}
