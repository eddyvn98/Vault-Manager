use std::str::FromStr;
use bip39::{Language, Mnemonic};
use bitcoin::{
    bip32::{DerivationPath, Xpriv},
    secp256k1::Secp256k1,
    Address, Network,
};
use tiny_keccak::{Hasher, Keccak};
use zeroize::Zeroizing;

pub const USDT_BEP20_CONTRACT: &str = "0x55d398326f99059fF775485246999027B3197955";

pub fn generate_24_words() -> Result<Zeroizing<String>, String> {
    let m = Mnemonic::generate_in(Language::English, 24)
        .map_err(|_| "Secure entropy generation failed".to_string())?;
    Ok(Zeroizing::new(m.to_string()))
}

pub fn validate_phrase(input: &str) -> Result<Zeroizing<String>, String> {
    let m = Mnemonic::parse_in(Language::English, input.trim())
        .map_err(|_| "Invalid recovery phrase or word not in BIP39 dictionary".to_string())?;
    if m.word_count() != 24 {
        return Err("Only 24-word recovery phrases are supported".into());
    }
    Ok(Zeroizing::new(m.to_string()))
}

/// Derive Native SegWit (P2WPKH) Bitcoin address (BIP84).
/// Mainnet path: m/84'/0'/0'/0/0 -> bc1q...
/// Signet path: m/84'/1'/0'/0/0 -> tb1q...
pub fn derive_btc_address(mnemonic: &str, network: Network) -> Result<String, String> {
    let m = Mnemonic::parse_in(Language::English, mnemonic.trim())
        .map_err(|_| "Invalid mnemonic phrase")?;
    let seed = m.to_seed("");
    let secp = Secp256k1::new();

    let master = Xpriv::new_master(network, &seed).map_err(|e| e.to_string())?;
    let path_str = match network {
        Network::Bitcoin => "m/84'/0'/0'/0/0",
        _ => "m/84'/1'/0'/0/0",
    };
    let path = DerivationPath::from_str(path_str).map_err(|e| e.to_string())?;
    let child = master.derive_priv(&secp, &path).map_err(|e| e.to_string())?;
    let secp_pubkey = child.to_priv().inner.public_key(&secp);
    let compressed = bitcoin::CompressedPublicKey(secp_pubkey);
    let address = Address::p2wpkh(&compressed, network);
    Ok(address.to_string())
}

/// Derive BNB Smart Chain (BSC) EVM address using standard BIP44 path (m/44'/60'/0'/0/0)
/// Formatted with standard EIP-55 mixed-case checksum.
pub fn derive_bsc_address(mnemonic: &str) -> Result<String, String> {
    let m = Mnemonic::parse_in(Language::English, mnemonic.trim())
        .map_err(|_| "Invalid mnemonic phrase")?;
    let seed = m.to_seed("");
    let secp = Secp256k1::new();

    let master = Xpriv::new_master(Network::Bitcoin, &seed).map_err(|e| e.to_string())?;
    let path = DerivationPath::from_str("m/44'/60'/0'/0/0").map_err(|e| e.to_string())?;
    let child = master.derive_priv(&secp, &path).map_err(|e| e.to_string())?;
    let pubkey = child.to_priv().inner.public_key(&secp);

    // Uncompressed pubkey: 65 bytes starting with 0x04
    let serialized = pubkey.serialize_uncompressed();
    let uncompressed_bytes = &serialized[1..]; // 64 bytes (x and y coordinates)

    // Keccak-256 hash
    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(uncompressed_bytes);
    hasher.finalize(&mut hash);

    let raw_addr = &hash[12..32]; // 20 bytes
    let hex_addr: String = raw_addr.iter().map(|b| format!("{:02x}", b)).collect();

    // Apply EIP-55 checksum
    let mut hasher2 = Keccak::v256();
    let mut hash2 = [0u8; 32];
    hasher2.update(hex_addr.as_bytes());
    hasher2.finalize(&mut hash2);

    let mut checksummed = String::with_capacity(42);
    checksummed.push_str("0x");
    for (i, c) in hex_addr.chars().enumerate() {
        let nibble = (hash2[i / 2] >> (if i % 2 == 0 { 4 } else { 0 })) & 0x0f;
        if nibble >= 8 {
            checksummed.push(c.to_ascii_uppercase());
        } else {
            checksummed.push(c);
        }
    }

    Ok(checksummed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bip39_generation_and_validation() {
        let p = generate_24_words().unwrap();
        let words: Vec<&str> = p.split_whitespace().collect();
        assert_eq!(words.len(), 24);

        let validated = validate_phrase(&p).unwrap();
        assert_eq!(&*validated, &*p);

        assert!(validate_phrase("invalid word sequence that fails").is_err());
        assert!(
            validate_phrase("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about")
                .is_err(),
            "Must reject 12-word phrases"
        );
    }

    #[test]
    fn derive_btc_and_bsc_addresses_deterministic() {
        // Standard BIP39 24-word test vector
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

        let btc_main = derive_btc_address(phrase, Network::Bitcoin).unwrap();
        assert!(btc_main.starts_with("bc1q"), "BTC mainnet address must start with bc1q: {}", btc_main);

        let btc_signet = derive_btc_address(phrase, Network::Signet).unwrap();
        assert!(btc_signet.starts_with("tb1q"), "BTC signet address must start with tb1q: {}", btc_signet);

        let bsc = derive_bsc_address(phrase).unwrap();
        assert!(bsc.starts_with("0x"), "BSC address must start with 0x: {}", bsc);
        assert_eq!(bsc.len(), 42, "BSC address must be 42 characters (0x + 40 hex)");

        // Repeat derivation to verify determinism
        let btc_main2 = derive_btc_address(phrase, Network::Bitcoin).unwrap();
        let bsc2 = derive_bsc_address(phrase).unwrap();
        assert_eq!(btc_main, btc_main2);
        assert_eq!(bsc, bsc2);
    }
}
