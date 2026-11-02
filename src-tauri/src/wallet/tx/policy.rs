use std::str::FromStr;
use bitcoin::{Address, Network};
use tiny_keccak::{Hasher, Keccak};

pub const DUST_LIMIT_SATOSHIS: u64 = 546;
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

pub fn validate_btc_fee(amount_satoshis: u64, fee_satoshis: u64) -> Result<(), String> {
    if fee_satoshis == 0 {
        return Err("Transaction fee cannot be zero".into());
    }
    if fee_satoshis > MAX_FEE_SATOSHIS {
        return Err(format!(
            "Calculated fee ({} sats) exceeds maximum allowed safety limit ({} sats)",
            fee_satoshis, MAX_FEE_SATOSHIS
        ));
    }
    if amount_satoshis > 2_000 && fee_satoshis > (amount_satoshis / 4) {
        return Err(format!(
            "Fee ({} sats) is disproportionately high (>25% of transfer amount {} sats)",
            fee_satoshis, amount_satoshis
        ));
    }
    Ok(())
}

/// Compute canonical EIP-55 mixed-case checksum for an EVM address hex string (excluding 0x).
pub fn to_eip55_checksum(hex_part: &str) -> String {
    let lower = hex_part.to_ascii_lowercase();
    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(lower.as_bytes());
    hasher.finalize(&mut hash);

    let mut checksummed = String::with_capacity(42);
    checksummed.push_str("0x");
    for (i, c) in lower.chars().enumerate() {
        let nibble = (hash[i / 2] >> (if i % 2 == 0 { 4 } else { 0 })) & 0x0f;
        if nibble >= 8 {
            checksummed.push(c.to_ascii_uppercase());
        } else {
            checksummed.push(c);
        }
    }
    checksummed
}

/// Validate EVM recipient address with strict EIP-55 checksum enforcement.
/// - Returns canonical EIP-55 checksummed address if valid.
/// - Accepts all-lowercase or all-uppercase unchecksummed addresses (canonicalizing them).
/// - Rejects mixed-case addresses that fail EIP-55 checksum verification.
/// - Rejects 0x0000... zero/burn address.
pub fn validate_evm_recipient(address: &str) -> Result<String, String> {
    let clean = address.trim();
    if clean.len() != 42 || (!clean.starts_with("0x") && !clean.starts_with("0X")) {
        return Err("Recipient EVM address must be 42 characters starting with 0x".into());
    }

    let hex_part = &clean[2..];
    if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Recipient EVM address contains non-hex characters".into());
    }

    if hex_part.chars().all(|c| c == '0') {
        return Err("Cannot send funds to the zero/burn address".into());
    }

    let canonical = to_eip55_checksum(hex_part);

    // If input has mixed-case letters, it MUST match the EIP-55 checksum exactly
    let has_lower = hex_part.chars().any(|c| matches!(c, 'a'..='f'));
    let has_upper = hex_part.chars().any(|c| matches!(c, 'A'..='F'));

    if has_lower && has_upper && clean[2..] != canonical[2..] {
        return Err("Invalid EIP-55 checksum: mixed-case EVM address has incorrect capitalization".into());
    }

    Ok(canonical)
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

        // Zero fee rejected
        assert!(validate_btc_fee(10_000, 0).is_err());

        // Normal fee accepted
        assert!(validate_btc_fee(10_000, 300).is_ok());

        // Fee exceeding safety limit (>50,000 sats) rejected
        assert!(validate_btc_fee(1_000_000, 50_001).is_err());
        assert!(validate_btc_fee(1_000_000, 50_000).is_ok());

        // Disproportionate fee (>25% of amount for amount > 2,000 sats) rejected
        assert!(validate_btc_fee(10_000, 2_600).is_err());
        assert!(validate_btc_fee(10_000, 2_400).is_ok());
    }

    #[test]
    fn evm_policy_validation() {
        let valid_checksummed = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";
        assert_eq!(validate_evm_recipient(valid_checksummed).unwrap(), valid_checksummed);

        // Lowercase is accepted and canonicalized to EIP-55
        let lower = "0x9858effd232b4033e47d90003d41ec34ecaeda94";
        assert_eq!(validate_evm_recipient(lower).unwrap(), valid_checksummed);

        // Uppercase is accepted and canonicalized to EIP-55
        let upper = "0x9858EFFD232B4033E47D90003D41EC34ECAEDA94";
        assert_eq!(validate_evm_recipient(upper).unwrap(), valid_checksummed);

        // Invalid mixed-case checksum must be rejected
        let corrupted_mixed = "0x9858effD232B4033E47d90003D41EC34EcaEda94";
        assert!(validate_evm_recipient(corrupted_mixed).is_err());

        // Zero address rejection
        assert!(validate_evm_recipient("0x0000000000000000000000000000000000000000").is_err());

        // Invalid length or chars
        assert!(validate_evm_recipient("0x123").is_err());
        assert!(validate_evm_recipient("9858EfFD232B4033E47d90003D41EC34EcaEda94").is_err());
        assert!(validate_evm_recipient("0x9858EfFD232B4033E47d90003D41EC34EcaEda9Z").is_err());
    }
}
