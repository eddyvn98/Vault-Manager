use num_bigint::BigUint;
use tiny_keccak::{Hasher, Keccak};

pub const ERC20_TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

/// Encodes an ERC-20 transfer(address _to, uint256 _value) calldata payload.
/// Format:
/// - 4 bytes: 0xa9059cbb (function selector)
/// - 32 bytes: zero-padded recipient address (left-padded 12 zero bytes + 20 address bytes)
/// - 32 bytes: big-endian uint256 amount (left-padded with zeros)
pub fn encode_erc20_transfer(to_address: &str, amount_tokens_wei: &BigUint) -> Result<String, String> {
    let clean_addr = to_address.trim().trim_start_matches("0x").trim_start_matches("0X");
    if clean_addr.len() != 40 {
        return Err("Recipient address must be 20 hex bytes (40 characters)".into());
    }

    if amount_tokens_wei.bits() > 256 {
        return Err("Amount exceeds uint256 maximum capacity (256 bits)".into());
    }

    // 1. Function selector (0xa9059cbb)
    let mut calldata = String::with_capacity(2 + 8 + 64 + 64);
    calldata.push_str("0x");
    for b in ERC20_TRANSFER_SELECTOR {
        calldata.push_str(&format!("{:02x}", b));
    }

    // 2. 32-byte zero-padded address
    calldata.push_str(&format!("{:0>64}", clean_addr.to_lowercase()));

    // 3. 32-byte big-endian amount in hex
    let amount_hex = amount_tokens_wei.to_str_radix(16);
    calldata.push_str(&format!("{:0>64}", amount_hex));

    Ok(calldata)
}

/// Compute 4-byte Keccak-256 selector from function signature string
#[allow(dead_code)]
pub fn compute_selector(signature: &str) -> [u8; 4] {
    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    hasher.update(signature.as_bytes());
    hasher.finalize(&mut hash);
    [hash[0], hash[1], hash[2], hash[3]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_erc20_selector() {
        let selector = compute_selector("transfer(address,uint256)");
        assert_eq!(selector, ERC20_TRANSFER_SELECTOR);
        assert_eq!(selector, [0xa9, 0x05, 0x9c, 0xbb]);
    }

    #[test]
    fn test_encode_erc20_transfer() {
        let recipient = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";
        // 1.5 tokens (1.5 * 10^18 = 1500000000000000000 = 0x14d1120d7b160000)
        let amount = BigUint::from(1500000000000000000u64);
        let calldata = encode_erc20_transfer(recipient, &amount).unwrap();

        assert_eq!(calldata.len(), 2 + 8 + 64 + 64);
        assert!(calldata.starts_with("0xa9059cbb"));
        assert!(calldata.contains("0000000000000000000000009858effd232b4033e47d90003d41ec34ecaeda94"));
        assert!(calldata.ends_with("00000000000000000000000000000000000000000000000014d1120d7b160000"));
    }
}
