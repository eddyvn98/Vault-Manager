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

/// Derive Native SegWit (P2WPKH) Bitcoin address (BIP84) at specific change and index branch.
/// Mainnet path: m/84'/0'/0'/{change}/{index}
/// Signet path: m/84'/1'/0'/{change}/{index}
pub fn derive_btc_address_at(
    mnemonic: &str,
    network: Network,
    change: u32,
    index: u32,
) -> Result<String, String> {
    let m = Mnemonic::parse_in(Language::English, mnemonic.trim())
        .map_err(|_| "Invalid mnemonic phrase")?;
    let seed = m.to_seed("");
    let secp = Secp256k1::new();

    let master = Xpriv::new_master(network, &seed).map_err(|e| e.to_string())?;
    let coin_type = match network {
        Network::Bitcoin => 0,
        _ => 1,
    };
    let path_str = format!("m/84'/{}'/0'/{}/{}", coin_type, change, index);
    let path = DerivationPath::from_str(&path_str).map_err(|e| e.to_string())?;
    let child = master.derive_priv(&secp, &path).map_err(|e| e.to_string())?;
    let secp_pubkey = child.to_priv().inner.public_key(&secp);
    let compressed = bitcoin::CompressedPublicKey(secp_pubkey);

    let address = Address::p2wpkh(&compressed, network);
    Ok(address.to_string())
}

pub fn derive_btc_address(mnemonic: &str, network: Network) -> Result<String, String> {
    derive_btc_address_at(mnemonic, network, 0, 0)
}

/// Derive BNB Smart Chain (BSC) EVM address at specific address index.
/// Path: m/44'/60'/0'/0/{index}
/// Formatted with standard EIP-55 mixed-case checksum.
pub fn derive_bsc_address_at(mnemonic: &str, index: u32) -> Result<String, String> {
    let m = Mnemonic::parse_in(Language::English, mnemonic.trim())
        .map_err(|_| "Invalid mnemonic phrase")?;
    let seed = m.to_seed("");
    let secp = Secp256k1::new();

    let master = Xpriv::new_master(Network::Bitcoin, &seed).map_err(|e| e.to_string())?;
    let path_str = format!("m/44'/60'/0'/0/{}", index);
    let path = DerivationPath::from_str(&path_str).map_err(|e| e.to_string())?;
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

pub fn derive_bsc_address(mnemonic: &str) -> Result<String, String> {
    derive_bsc_address_at(mnemonic, 0)
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
    fn official_bip84_and_evm_test_vectors() {
        // Official BIP-0084 test vector
        // https://github.com/bitcoin/bips/blob/master/bip-0084.mediawiki
        let official_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        // First receive address: m/84'/0'/0'/0/0
        let addr0 = derive_btc_address_at(official_mnemonic, Network::Bitcoin, 0, 0).unwrap();
        assert_eq!(addr0, "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu", "Must match official BIP84 receive address 0");

        // Second receive address: m/84'/0'/0'/0/1
        let addr1 = derive_btc_address_at(official_mnemonic, Network::Bitcoin, 0, 1).unwrap();
        assert_eq!(addr1, "bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g", "Must match official BIP84 receive address 1");

        // First change address: m/84'/0'/0'/1/0
        let change0 = derive_btc_address_at(official_mnemonic, Network::Bitcoin, 1, 0).unwrap();
        assert_eq!(change0, "bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el", "Must match official BIP84 change address 0");

        // Standard EVM BIP44 m/44'/60'/0'/0/0 test vector
        let evm0 = derive_bsc_address_at(official_mnemonic, 0).unwrap();
        assert_eq!(evm0, "0x9858EfFD232B4033E47d90003D41EC34EcaEda94", "Must match official EVM m/44'/60'/0'/0/0 address");
    }
}
