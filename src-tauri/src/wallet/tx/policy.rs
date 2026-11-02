use std::str::FromStr;
use bitcoin::{Address, Network};

pub const DUST_LIMIT_SATOSHIS: u64 = 546;
#[allow(dead_code)]
pub const MAX_FEE_SATOSHIS: u64 = 50_000;
pub const DEFAULT_SIGNET_FEE_RATE: u64 = 2; // sat/vB
pub const BSC_TESTNET_CHAIN_ID: u64 = 97;

pub fn validate_btc_signet_recipient(address: &str) -> Result<Address, String> {
    let clean = address.trim();
    if clean.is_empty() {
        return Err("Recipient Bitcoin address cannot be empty".into());
    }

    let addr = Address::from_str(clean)
        .map_err(|e| format!("Invalid Bitcoin address format: {}", e))?
        .require_network(Network::Signet)
        .map_err(|_| "Address is not valid for Bitcoin Signet (must start with tb1q...)".to_string())?;

    Ok(addr)
}

pub fn validate_btc_amount(amount_sats: u64) -> Result<(), String> {
    if amount_sats < DUST_LIMIT_SATOSHIS {
        return Err(format!(
            "Amount is below dust limit (minimum {} satoshis required)",
            DUST_LIMIT_SATOSHIS
        ));
    }
    if amount_sats > 21_000_000 * 100_000_000 {
        return Err("Amount exceeds maximum Bitcoin supply".into());
    }
    Ok(())
}

pub fn validate_evm_recipient(address: &str) -> Result<String, String> {
    let clean = address.trim();
    if clean.len() != 42 || !clean.starts_with("0x") {
        return Err("Recipient EVM address must be 42 characters starting with 0x".into());
    }

    let hex_part = &clean[2..];
    if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Recipient EVM address contains non-hex characters".into());
    }

    if clean.to_lowercase() == "0x0000000000000000000000000000000000000000" {
        return Err("Cannot send funds to the zero/burn address".into());
    }

    Ok(clean.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn btc_policy_validation() {
        // Valid Signet address
        let valid_signet = "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx";
        assert!(validate_btc_signet_recipient(valid_signet).is_ok());

        // Mainnet address must be rejected on Signet
        let mainnet_addr = "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu";
        assert!(validate_btc_signet_recipient(mainnet_addr).is_err());

        // Dust check
        assert!(validate_btc_amount(100).is_err());
        assert!(validate_btc_amount(546).is_ok());
        assert!(validate_btc_amount(10_000).is_ok());
    }

    #[test]
    fn evm_policy_validation() {
        let valid_evm = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";
        assert!(validate_evm_recipient(valid_evm).is_ok());

        // Zero address rejection
        assert!(validate_evm_recipient("0x0000000000000000000000000000000000000000").is_err());

        // Invalid length or chars
        assert!(validate_evm_recipient("0x123").is_err());
        assert!(validate_evm_recipient("9858EfFD232B4033E47d90003D41EC34EcaEda94").is_err());
        assert!(validate_evm_recipient("0x9858EfFD232B4033E47d90003D41EC34EcaEda9Z").is_err());
    }
}
