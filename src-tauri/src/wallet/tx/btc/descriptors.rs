use std::str::FromStr;
use bip39::{Language, Mnemonic};
use bitcoin::{
    bip32::{DerivationPath, Xpriv, Xpub},
    secp256k1::Secp256k1,
    Network,
};

#[allow(dead_code)]
pub struct Bip84Descriptors {
    pub receive_descriptor: String,
    pub change_descriptor: String,
    pub account_xpub: String,
    pub fingerprint: String,
}

/// Derive watch-only BIP84 descriptors from mnemonic for Bitcoin Signet.
/// Security guarantee: Only public extended keys (xpub/tpub) and key origins are included;
/// No private keys (xprv) are ever included in the descriptors.
#[allow(dead_code)]
pub fn derive_signet_bip84_descriptors(mnemonic: &str) -> Result<Bip84Descriptors, String> {
    let m = Mnemonic::parse_in(Language::English, mnemonic.trim())
        .map_err(|_| "Invalid mnemonic phrase")?;
    let seed = m.to_seed("");
    let secp = Secp256k1::new();

    let master = Xpriv::new_master(Network::Signet, &seed).map_err(|e| e.to_string())?;
    let fingerprint = master.fingerprint(&secp).to_string();

    let account_path = DerivationPath::from_str("m/84'/1'/0'").map_err(|e| e.to_string())?;
    let account_xpriv = master.derive_priv(&secp, &account_path).map_err(|e| e.to_string())?;
    let account_xpub = Xpub::from_priv(&secp, &account_xpriv);
    let xpub_str = account_xpub.to_string();

    let receive_descriptor = format!("wpkh([{}/84'/1'/0']{}/0/*)", fingerprint, xpub_str);
    let change_descriptor = format!("wpkh([{}/84'/1'/0']{}/1/*)", fingerprint, xpub_str);

    Ok(Bip84Descriptors {
        receive_descriptor,
        change_descriptor,
        account_xpub: xpub_str,
        fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bip84_watch_only_descriptors_derivation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let desc = derive_signet_bip84_descriptors(mnemonic).unwrap();

        assert!(desc.receive_descriptor.starts_with("wpkh(["));
        assert!(desc.receive_descriptor.ends_with("/0/*)"));
        assert!(desc.change_descriptor.ends_with("/1/*)"));
        assert!(!desc.receive_descriptor.contains("prv"), "Watch-only descriptor must never contain private key");
        assert!(!desc.change_descriptor.contains("prv"), "Watch-only descriptor must never contain private key");
    }
}
