use argon2::Argon2;
use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{
    aead::{rand_core::RngCore, Aead, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const AAD: &[u8] = b"VaultManager:local:wallet:v1";
pub const MAX_PASSWORD_BYTES: usize = 1024;
pub const MIN_PASSWORD_CHARS: usize = 12;

#[derive(Serialize, Deserialize)]
pub struct Envelope {
    pub version: u8,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

pub fn password_ok(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_CHARS || password.len() > MAX_PASSWORD_BYTES {
        return Err("Password must be at least 12 characters and at most 1024 bytes".into());
    }
    Ok(())
}

pub fn derive_key(password: &str, salt_bytes: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    let mut result = Zeroizing::new([0u8; 32]);
    // OWASP-aligned explicit Argon2id parameters (64 MiB, 3 passes, 1 thread).
    let params = argon2::Params::new(64 * 1024, 3, 1, Some(32))
        .map_err(|_| "Invalid KDF parameters")?;
    let argon = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    argon
        .hash_password_into(password.as_bytes(), salt_bytes, &mut *result)
        .map_err(|_| "Key derivation failed")?;
    Ok(result)
}

pub fn encrypt(password: &str, payload: &str) -> Result<Vec<u8>, String> {
    password_ok(password)?;
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: payload.as_bytes(),
                aad: AAD,
            },
        )
        .map_err(|_| "Encryption failed")?;
    serde_json::to_vec(&Envelope {
        version: 1,
        salt: STANDARD.encode(salt),
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    })
    .map_err(|_| "Cannot encode vault".into())
}

pub fn decrypt(password: &str, raw: &[u8]) -> Result<Zeroizing<String>, String> {
    if raw.len() > 16_384 {
        return Err("Invalid vault file".into());
    }
    let env: Envelope = serde_json::from_slice(raw).map_err(|_| "Invalid vault file")?;
    if env.version != 1 {
        return Err("Unsupported vault version".into());
    }
    let salt = STANDARD.decode(env.salt).map_err(|_| "Invalid vault file")?;
    let nonce = STANDARD.decode(env.nonce).map_err(|_| "Invalid vault file")?;
    let ciphertext = STANDARD
        .decode(env.ciphertext)
        .map_err(|_| "Invalid vault file")?;
    if salt.len() != 16 || nonce.len() != 24 {
        return Err("Invalid vault file".into());
    }
    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: &ciphertext,
                    aad: AAD,
                },
            )
            .map_err(|_| "Incorrect password or corrupted vault".to_string())?,
    );
    let text = Zeroizing::new(
        String::from_utf8(plaintext.to_vec()).map_err(|_| "Invalid vault content".to_string())?,
    );
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encryption_roundtrip_and_wrong_password() {
        let msg = "test-mnemonic-phrase-recovery";
        let encrypted = encrypt("safe-password-123", msg).unwrap();
        assert_eq!(&*decrypt("safe-password-123", &encrypted).unwrap(), msg);
        assert!(decrypt("wrong-password-123", &encrypted).is_err());
    }

    #[test]
    fn encryption_is_randomized_and_tamper_evident() {
        let msg = "test-mnemonic-phrase-recovery";
        let a = encrypt("safe-password-123", msg).unwrap();
        let b = encrypt("safe-password-123", msg).unwrap();
        assert_ne!(a, b);
        let mut envelope: Envelope = serde_json::from_slice(&a).unwrap();
        envelope.ciphertext.replace_range(0..1, "X");
        assert!(decrypt("safe-password-123", &serde_json::to_vec(&envelope).unwrap()).is_err());
    }
}
