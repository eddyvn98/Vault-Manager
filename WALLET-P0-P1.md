# Tauri 2 personal wallet: P0-P1 (experimental)

This branch adds a separate Tauri 2 entrypoint for Windows, macOS and Android. **This is not a production wallet. Do not deposit any BTC, USDT or other assets.** The original web/password manager remains available through npm run dev and is **not** bundled into the native wallet UI.

## Commands
- npm install
- npm run build:wallet (TypeScript + frontend build)
- npm run wallet:dev (requires local Rust/Cargo and Tauri desktop prerequisites)
- npm run wallet:android:init (requires Android Studio SDK/NDK, Rust Android targets)
- npm run wallet:android:dev
- cd src-tauri && cargo test

## P0-P1 implementation
- Native Rust commands create/restore (24-word BIP39), unlock, lock, status.
- CSPRNG entropy; Argon2id (64 MiB, 3 passes) + XChaCha20-Poly1305 AEAD.
- Random salt and nonce per device; only an encrypted file stored in native app data; write once, no overwrite.
- Memory-only Zeroizing mnemonic while unlocked. Storage and mnemonic are separate on every device.
- Minimal UI shows one-time phrase and explicitly labels BTC/BSC features unavailable. No RPC, balances, signing, broadcast, import/export, key transmission, Telegram or AI access.
- Android lifecycle and secure-screen protections, timeout, password retry throttling, hardware-bound protection and recovery verification are **not complete**. A background/visibility lock is a best-effort UI safeguard, not an operating-system security boundary.

## Design gate before P2/P3
- Add independent signing authority for BTC and BSC with established libraries; verify BIP32/44/84 derivation and test vectors, BSC chain ID 56, BEP20 contract identity and 18 decimals.
- Validate signed transaction previews, fees, nonce/UTXO handling, replacement, replay resistance and chain/network mismatches.
- Encrypt backup with authenticated versioned formats; test recovery across Mac/Windows/Android using dedicated nonfunded phrases. Never check phrases into Git.
- Replace mock 2FA/security messaging in the legacy password manager before making claims about that product's security.
- Add Rust desktop/Android CI, integration tests, device lifecycle hardening, audited dependency lockfiles, security review, and a separate external audit before handling assets.

## Threat boundary
Only commands declared in the Tauri native command list are exposed to the wallet frontend. Never include legacy password manager code in wallet.html or allow runtime remote content. Encryption at rest does not protect a compromised unlocked device or a malicious process with equivalent local privileges. This code does not yet guarantee hardware-keystore binding, anti-bruteforce, side-channel resistance or secure phrase entry.
