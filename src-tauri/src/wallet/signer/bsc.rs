use num_bigint::BigUint;
use secp256k1::{
    ecdsa::{RecoverableSignature, RecoveryId},
    Message, Secp256k1, SecretKey,
};
use tiny_keccak::{Hasher, Keccak};
use crate::wallet::tx::policy;

pub mod rlp {
    use num_bigint::BigUint;
    use num_traits::Zero;

    pub fn encode_bytes(bytes: &[u8]) -> Vec<u8> {
        if bytes.len() == 1 && bytes[0] <= 0x7f {
            vec![bytes[0]]
        } else if bytes.len() <= 55 {
            let mut out = Vec::with_capacity(1 + bytes.len());
            out.push(0x80 + bytes.len() as u8);
            out.extend_from_slice(bytes);
            out
        } else {
            let len_bytes = encode_length(bytes.len());
            let mut out = Vec::with_capacity(1 + len_bytes.len() + bytes.len());
            out.push(0xb7 + len_bytes.len() as u8);
            out.extend_from_slice(&len_bytes);
            out.extend_from_slice(bytes);
            out
        }
    }

    pub fn encode_u64(val: u64) -> Vec<u8> {
        if val == 0 {
            vec![0x80]
        } else {
            let be_bytes = val.to_be_bytes();
            let first_nonzero = be_bytes.iter().position(|&b| b != 0).unwrap_or(8);
            encode_bytes(&be_bytes[first_nonzero..])
        }
    }

    pub fn encode_biguint(val: &BigUint) -> Vec<u8> {
        if val.is_zero() {
            vec![0x80]
        } else {
            let bytes = val.to_bytes_be();
            encode_bytes(&bytes)
        }
    }

    pub fn encode_list(items: &[Vec<u8>]) -> Vec<u8> {
        let total_len: usize = items.iter().map(|item| item.len()).sum();
        let mut out = Vec::new();
        if total_len <= 55 {
            out.push(0xc0 + total_len as u8);
        } else {
            let len_bytes = encode_length(total_len);
            out.push(0xf7 + len_bytes.len() as u8);
            out.extend_from_slice(&len_bytes);
        }
        for item in items {
            out.extend_from_slice(item);
        }
        out
    }

    fn encode_length(len: usize) -> Vec<u8> {
        let be = len.to_be_bytes();
        let first_nonzero = be.iter().position(|&b| b != 0).unwrap_or(be.len() - 1);
        be[first_nonzero..].to_vec()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eip155Transaction {
    pub nonce: u64,
    pub gas_price: BigUint,
    pub gas_limit: u64,
    pub to: String, // 42-char EVM address
    pub value: BigUint,
    pub data: Vec<u8>,
    pub chain_id: u64,
}

#[derive(Clone, Debug)]
pub struct Eip155SignedTransaction {
    pub tx: Eip155Transaction,
    pub v: u64,
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub raw_signed_tx_bytes: Vec<u8>,
}

impl Eip155Transaction {
    /// Encode the unsigned EIP-155 transaction to RLP items:
    /// [nonce, gasprice, gaslimit, to, value, data, chain_id, 0, 0]
    pub fn rlp_unsigned_bytes(&self) -> Result<Vec<u8>, String> {
        let clean_to = self.to.trim().trim_start_matches("0x");
        let to_bytes = hex::decode(clean_to)
            .map_err(|e| format!("Invalid hex address in 'to': {}", e))?;
        if to_bytes.len() != 20 {
            return Err("Recipient address must be 20 bytes".into());
        }

        let items = vec![
            rlp::encode_u64(self.nonce),
            rlp::encode_biguint(&self.gas_price),
            rlp::encode_u64(self.gas_limit),
            rlp::encode_bytes(&to_bytes),
            rlp::encode_biguint(&self.value),
            rlp::encode_bytes(&self.data),
            rlp::encode_u64(self.chain_id),
            rlp::encode_u64(0),
            rlp::encode_u64(0),
        ];

        Ok(rlp::encode_list(&items))
    }

    /// Compute Keccak-256 signing hash according to EIP-155
    pub fn signing_hash(&self) -> Result<[u8; 32], String> {
        let rlp_bytes = self.rlp_unsigned_bytes()?;
        let mut hasher = Keccak::v256();
        let mut hash = [0u8; 32];
        hasher.update(&rlp_bytes);
        hasher.finalize(&mut hash);
        Ok(hash)
    }

    /// Offline sign transaction with secp256k1 and produce standard EIP-155 signed payload
    pub fn sign(
        &self,
        secp: &Secp256k1<secp256k1::All>,
        secret_key: &SecretKey,
    ) -> Result<Eip155SignedTransaction, String> {
        let hash = self.signing_hash()?;
        let msg = Message::from_digest(hash);

        let recoverable_sig = secp.sign_ecdsa_recoverable(&msg, secret_key);
        let (rec_id, compact) = recoverable_sig.serialize_compact();

        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&compact[0..32]);
        s.copy_from_slice(&compact[32..64]);

        let v = (rec_id.to_i32() as u64) + self.chain_id * 2 + 35;

        // RLP encode signed transaction:
        // [nonce, gasprice, gaslimit, to, value, data, v, r, s]
        let clean_to = self.to.trim().trim_start_matches("0x");
        let to_bytes = hex::decode(clean_to)
            .map_err(|e| format!("Invalid hex address in 'to': {}", e))?;

        // Format r and s without leading zeros for RLP integer format
        let r_first = r.iter().position(|&b| b != 0).unwrap_or(31);
        let s_first = s.iter().position(|&b| b != 0).unwrap_or(31);

        let signed_items = vec![
            rlp::encode_u64(self.nonce),
            rlp::encode_biguint(&self.gas_price),
            rlp::encode_u64(self.gas_limit),
            rlp::encode_bytes(&to_bytes),
            rlp::encode_biguint(&self.value),
            rlp::encode_bytes(&self.data),
            rlp::encode_u64(v),
            rlp::encode_bytes(&r[r_first..]),
            rlp::encode_bytes(&s[s_first..]),
        ];

        let raw_signed_tx_bytes = rlp::encode_list(&signed_items);

        Ok(Eip155SignedTransaction {
            tx: self.clone(),
            v,
            r,
            s,
            raw_signed_tx_bytes,
        })
    }
}

impl Eip155SignedTransaction {
    /// Mathematically recover signer's address from (v, r, s) and transaction payload
    pub fn recover_sender(&self, secp: &Secp256k1<secp256k1::All>) -> Result<String, String> {
        let min_v = self.tx.chain_id * 2 + 35;
        if self.v < min_v || self.v > min_v + 1 {
            return Err(format!(
                "Invalid EIP-155 v ({}) for chain_id {}",
                self.v, self.tx.chain_id
            ));
        }

        let rec_id_val = (self.v - min_v) as i32;
        let rec_id = RecoveryId::from_i32(rec_id_val)
            .map_err(|e| format!("Invalid recovery ID: {}", e))?;

        let mut compact = [0u8; 64];
        compact[0..32].copy_from_slice(&self.r);
        compact[32..64].copy_from_slice(&self.s);

        let recoverable_sig = RecoverableSignature::from_compact(&compact, rec_id)
            .map_err(|e| format!("Invalid signature format: {}", e))?;

        let hash = self.tx.signing_hash()?;
        let msg = Message::from_digest(hash);

        let pubkey = secp
            .recover_ecdsa(&msg, &recoverable_sig)
            .map_err(|e| format!("Public key recovery failed: {}", e))?;

        let uncompressed = pubkey.serialize_uncompressed(); // 65 bytes
        let mut hasher = Keccak::v256();
        let mut hash_res = [0u8; 32];
        hasher.update(&uncompressed[1..]); // omit 0x04 prefix
        hasher.finalize(&mut hash_res);

        let raw_addr = &hash_res[12..32];
        let canonical_addr = policy::to_eip55_checksum(&hex::encode(raw_addr));
        Ok(canonical_addr)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BscSigningPolicy {
    pub expected_chain_id: u64,
    pub expected_to: String,
    pub expected_data: Vec<u8>,
    pub expected_sender: String,
    pub expected_value: BigUint, // Strictly zero for BEP-20 token transfers
    pub expected_nonce: u64,
    pub expected_gas_limit: u64,
    pub expected_gas_price: BigUint,
}

impl BscSigningPolicy {
    pub fn from_canonical_payload(
        payload: &crate::wallet::tx::BscCanonicalPayload,
        expected_sender: &str,
    ) -> Result<Self, String> {
        if payload.chain_id != crate::wallet::tx::policy::BSC_TESTNET_CHAIN_ID {
            return Err(format!(
                "Invalid chain ID: expected {} (BSC Testnet), got {}",
                crate::wallet::tx::policy::BSC_TESTNET_CHAIN_ID,
                payload.chain_id
            ));
        }

        let contract = payload
            .token_contract
            .as_ref()
            .ok_or_else(|| "Missing token contract address in canonical payload".to_string())?;

        if contract.to_lowercase() != crate::wallet::blockchain::BSC_TESTNET_USDT_CONTRACT.to_lowercase() {
            return Err(format!(
                "Unauthorized contract: only testnet USDT {} is permitted, got {}",
                crate::wallet::blockchain::BSC_TESTNET_USDT_CONTRACT,
                contract
            ));
        }

        let amount_wei = BigUint::parse_bytes(payload.amount_raw.as_bytes(), 10)
            .ok_or_else(|| "Invalid amount raw in canonical payload".to_string())?;

        // Independently re-encode expected calldata from recipient and amount
        let expected_calldata = crate::wallet::tx::bsc::calldata::encode_erc20_transfer(
            &payload.recipient_address,
            &amount_wei,
        )?;

        if payload.calldata_hex.trim().to_lowercase() != expected_calldata.trim().to_lowercase() {
            return Err(format!(
                "Calldata mismatch: payload calldata does not match authorized ERC20 transfer(to, amount): expected {}, got {}",
                expected_calldata, payload.calldata_hex
            ));
        }

        let clean_hex = payload.calldata_hex.trim().trim_start_matches("0x").trim_start_matches("0X");
        let data = hex::decode(clean_hex)
            .map_err(|e| format!("Invalid hex calldata in canonical payload: {}", e))?;

        let gas_price = BigUint::parse_bytes(payload.gas_price_wei.as_bytes(), 10)
            .ok_or_else(|| "Invalid gas price wei in canonical payload".to_string())?;

        Ok(Self {
            expected_chain_id: payload.chain_id,
            expected_to: contract.clone(),
            expected_data: data,
            expected_sender: expected_sender.to_string(),
            expected_value: BigUint::from(0u32),
            expected_nonce: payload.nonce,
            expected_gas_limit: payload.gas_limit,
            expected_gas_price: gas_price,
        })
    }
}

/// Validate transaction against approved policy, contract, calldata, gas fields, nonce, value,
/// and authorized sender address before signing.
pub fn validate_and_sign_bsc_tx(
    tx: &Eip155Transaction,
    policy: &BscSigningPolicy,
    secp: &Secp256k1<secp256k1::All>,
    secret_key: &SecretKey,
) -> Result<Eip155SignedTransaction, String> {
    if tx.chain_id != policy.expected_chain_id {
        return Err(format!(
            "Chain ID mismatch: expected {}, got {}",
            policy.expected_chain_id, tx.chain_id
        ));
    }

    if tx.to.to_lowercase() != policy.expected_to.to_lowercase() {
        return Err(format!(
            "Recipient contract mismatch: expected {}, got {}",
            policy.expected_to, tx.to
        ));
    }

    if tx.data != policy.expected_data {
        return Err("Transaction calldata does not match approved payload".into());
    }

    if tx.value != policy.expected_value {
        return Err(format!(
            "Transaction value mismatch: expected {} wei, got {} wei (unintended BNB transfer forbidden)",
            policy.expected_value, tx.value
        ));
    }

    if tx.nonce != policy.expected_nonce {
        return Err(format!(
            "Nonce mismatch: expected {}, got {}",
            policy.expected_nonce, tx.nonce
        ));
    }

    if tx.gas_limit != policy.expected_gas_limit {
        return Err(format!(
            "Gas limit mismatch: expected {}, got {}",
            policy.expected_gas_limit, tx.gas_limit
        ));
    }

    if tx.gas_price != policy.expected_gas_price {
        return Err(format!(
            "Gas price mismatch: expected {} wei, got {} wei",
            policy.expected_gas_price, tx.gas_price
        ));
    }

    // Verify gas policy
    let total_fee_wei = &tx.gas_price * BigUint::from(tx.gas_limit);
    let max_fee_wei = BigUint::from(50_000_000_000_000_000u64); // 0.05 BNB cap
    if total_fee_wei > max_fee_wei {
        return Err("Estimated fee exceeds safety cap of 0.05 BNB".into());
    }

    let signed = tx.sign(secp, secret_key)?;
    let recovered = signed.recover_sender(secp)?;

    if recovered.to_lowercase() != policy.expected_sender.to_lowercase() {
        return Err(format!(
            "Recovered sender ({}) does not match authorized owner address ({})",
            recovered, policy.expected_sender
        ));
    }

    Ok(signed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn it04_bsc_rpc_mock_policies_and_pre_sign_validation() {
        let secp = Secp256k1::new();
        let test_privkey = SecretKey::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
        
        // Derive expected sender
        let pubkey = test_privkey.public_key(&secp);
        let uncompressed = pubkey.serialize_uncompressed();
        let mut hasher = Keccak::v256();
        let mut hash_res = [0u8; 32];
        hasher.update(&uncompressed[1..]);
        hasher.finalize(&mut hash_res);
        let expected_owner = policy::to_eip55_checksum(&hex::encode(&hash_res[12..32]));

        let contract_to = "0x337610d27c682E347C9cD60bd4b3b107C9d34dDd";
        let valid_data = hex::decode("a9059cbb0000000000000000000000009858effd232b4033e47d90003d41ec34ecaeda940000000000000000000000000000000000000000000000000de0b6b3a7640000").unwrap();

        let tx = Eip155Transaction {
            nonce: 0,
            gas_price: BigUint::from(3_000_000_000u64), // 3 Gwei
            gas_limit: 65_000,
            to: contract_to.into(),
            value: BigUint::from(0u32),
            data: valid_data.clone(),
            chain_id: 97,
        };

        let policy = BscSigningPolicy {
            expected_chain_id: 97,
            expected_to: contract_to.into(),
            expected_data: valid_data.clone(),
            expected_sender: expected_owner.clone(),
            expected_value: BigUint::from(0u32),
            expected_nonce: 0,
            expected_gas_limit: 65_000,
            expected_gas_price: BigUint::from(3_000_000_000u64),
        };

        // 1. Valid transaction signing & sender match -> PASS
        let sign_res = validate_and_sign_bsc_tx(&tx, &policy, &secp, &test_privkey);
        assert!(sign_res.is_ok(), "Valid BSC transaction must pass validation and signing");

        // 2. Chain ID mismatch (e.g. prepared for 56 instead of 97) -> REJECT
        let mut tx_wrong_chain = tx.clone();
        tx_wrong_chain.chain_id = 56;
        let err_chain = validate_and_sign_bsc_tx(&tx_wrong_chain, &policy, &secp, &test_privkey);
        assert!(err_chain.is_err(), "Chain ID mismatch must be rejected");

        // 3. Contract address tampered -> REJECT
        let mut tx_wrong_contract = tx.clone();
        tx_wrong_contract.to = "0x1111111111111111111111111111111111111111".into();
        let err_contract = validate_and_sign_bsc_tx(&tx_wrong_contract, &policy, &secp, &test_privkey);
        assert!(err_contract.is_err(), "Contract address tampering must be rejected");

        // 4. Calldata tampered (amount or recipient modified) -> REJECT
        let mut tx_wrong_data = tx.clone();
        tx_wrong_data.data[10] ^= 0x01;
        let err_data = validate_and_sign_bsc_tx(&tx_wrong_data, &policy, &secp, &test_privkey);
        assert!(err_data.is_err(), "Calldata tampering must be rejected");

        // 5. Unintended BNB transfer in token transaction (tx.value > 0) -> REJECT
        let mut tx_with_bnb = tx.clone();
        tx_with_bnb.value = BigUint::from(1u32); // 1 Wei BNB
        let err_value = validate_and_sign_bsc_tx(&tx_with_bnb, &policy, &secp, &test_privkey);
        assert!(err_value.is_err(), "Unintended BNB transfer must be rejected");
        assert!(err_value.unwrap_err().contains("unintended BNB transfer forbidden"));

        // 6. Nonce tampered -> REJECT
        let mut tx_wrong_nonce = tx.clone();
        tx_wrong_nonce.nonce = 1;
        let err_nonce = validate_and_sign_bsc_tx(&tx_wrong_nonce, &policy, &secp, &test_privkey);
        assert!(err_nonce.is_err(), "Nonce tampering must be rejected");

        // 7. Gas price tampered (even within 0.05 BNB cap) -> REJECT
        let mut tx_tampered_gas_price = tx.clone();
        tx_tampered_gas_price.gas_price = BigUint::from(4_000_000_000u64); // 4 Gwei instead of approved 3 Gwei
        let err_gas_price = validate_and_sign_bsc_tx(&tx_tampered_gas_price, &policy, &secp, &test_privkey);
        assert!(err_gas_price.is_err(), "Gas price deviation from approved draft must be rejected");

        // 8. Total fee exceeding 0.05 BNB cap -> REJECT
        let mut tx_expensive = tx.clone();
        tx_expensive.gas_price = BigUint::from(1_000_000_000_000u64); // 1000 Gwei (fee = 0.065 BNB)
        let mut expensive_policy = policy.clone();
        expensive_policy.expected_gas_price = tx_expensive.gas_price.clone();
        let err_fee = validate_and_sign_bsc_tx(&tx_expensive, &expensive_policy, &secp, &test_privkey);
        assert!(err_fee.is_err(), "Fee exceeding 0.05 BNB cap must be rejected");

        // 9. Signing with unauthorized key (sender mismatch) -> REJECT
        let wrong_privkey = SecretKey::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap();
        let err_key = validate_and_sign_bsc_tx(&tx, &policy, &secp, &wrong_privkey);
        assert!(err_key.is_err(), "Signing with non-authorized key must be rejected");
    }

    #[test]
    fn official_eip155_test_vector_verification() {
        // Official EIP-155 test vector:
        // https://eips.ethereum.org/EIPS/eip-155
        let test_privkey_hex = "4646464646464646464646464646464646464646464646464646464646464646";
        let expected_sender = "0x9d8A62f656a8d1615C1294fd71e9CFb3E4855A4F";
        let expected_signing_hash_hex = "daf5a779ae972f972197303d7b574746c7ef83eadac0f2791ad23db92e4c8e53";
        let expected_raw_signed_hex = "f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_str(test_privkey_hex).unwrap();

        let tx = Eip155Transaction {
            nonce: 9,
            gas_price: BigUint::from(20_000_000_000u64), // 20 Gwei
            gas_limit: 21_000,
            to: "0x3535353535353535353535353535353535353535".into(),
            value: BigUint::from(1_000_000_000_000_000_000u64), // 1 ETH / BNB
            data: vec![],
            chain_id: 1,
        };

        // 1. Verify signing hash
        let hash = tx.signing_hash().unwrap();
        assert_eq!(hex::encode(hash), expected_signing_hash_hex, "Must match official EIP-155 signing hash");

        // 2. Sign transaction
        let signed = tx.sign(&secp, &secret_key).unwrap();
        assert_eq!(signed.v, 37, "Must match official EIP-155 v value (1*2 + 35)");
        assert_eq!(hex::encode(&signed.raw_signed_tx_bytes), expected_raw_signed_hex, "Must match official EIP-155 raw serialized transaction");

        // 3. Verify sender recovery
        let recovered = signed.recover_sender(&secp).unwrap();
        assert_eq!(recovered.to_lowercase(), expected_sender.to_lowercase(), "Recovered address must match official test vector sender");
    }

    #[test]
    fn bsc_chain97_signing_and_replay_protection() {
        let secp = Secp256k1::new();
        let test_privkey = SecretKey::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap();

        let tx_bsc_testnet = Eip155Transaction {
            nonce: 5,
            gas_price: BigUint::from(3_000_000_000u64), // 3 Gwei
            gas_limit: 65_000,
            to: "0x9858EfFD232B4033E47d90003D41EC34EcaEda94".into(),
            value: BigUint::from(0u32),
            data: hex::decode("a9059cbb0000000000000000000000009858effd232b4033e47d90003d41ec34ecaeda940000000000000000000000000000000000000000000000000de0b6b3a7640000").unwrap(),
            chain_id: 97, // BSC Testnet
        };

        // 1. Sign for BSC Testnet (Chain ID 97)
        let signed_97 = tx_bsc_testnet.sign(&secp, &test_privkey).unwrap();
        assert_eq!(signed_97.v, 97 * 2 + 35, "v must equal chain_id * 2 + 35 for rec_id=0 (or +36 for rec_id=1)");

        // 2. Sender recovery on Chain ID 97 must succeed
        let sender_97 = signed_97.recover_sender(&secp).unwrap();
        assert!(sender_97.starts_with("0x"));

        // 3. ANTI-REPLAY TEST: Attempting to replay or recover signature on BSC Mainnet (Chain ID 56)
        let mut replayed_on_56 = signed_97.clone();
        replayed_on_56.tx.chain_id = 56; // Change chain ID to mainnet
        let replay_res = replayed_on_56.recover_sender(&secp);
        assert!(
            replay_res.is_err(),
            "Signature created for chain 97 must be rejected on chain 56 (v mismatch)"
        );

        // 4. TAMPER TEST: Tampering with calldata recipient or token amount
        let mut tampered_data = signed_97.clone();
        tampered_data.tx.data[10] ^= 0xff; // Modify calldata byte
        let tampered_sender = tampered_data.recover_sender(&secp).unwrap();
        assert_ne!(
            tampered_sender, sender_97,
            "Tampered calldata must recover a different sender address, invalidating the transaction"
        );
    }

    #[test]
    fn test_bsc_calldata_tamper_rejection_and_canonical_hash_binding() {
        let recipient_addr = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";
        let amount_raw = "1000000000000000000"; // 1.0 USDT
        let amount_wei = BigUint::parse_bytes(amount_raw.as_bytes(), 10).unwrap();
        let valid_calldata = crate::wallet::tx::bsc::calldata::encode_erc20_transfer(
            recipient_addr,
            &amount_wei,
        ).unwrap();

        let valid_payload = crate::wallet::tx::BscCanonicalPayload {
            chain_id: 97,
            token_contract: Some(crate::wallet::blockchain::BSC_TESTNET_USDT_CONTRACT.to_string()),
            recipient_address: recipient_addr.to_string(),
            amount_raw: amount_raw.to_string(),
            nonce: 10,
            gas_limit: 65_000,
            gas_price_wei: "3000000000".to_string(),
            calldata_hex: valid_calldata.clone(),
        };

        // 1. Valid payload must hash deterministically and construct policy
        let hash_valid = valid_payload.canonical_hash();
        let policy_res = BscSigningPolicy::from_canonical_payload(&valid_payload, "0x1111111111111111111111111111111111111111");
        assert!(policy_res.is_ok(), "Valid canonical payload must succeed");

        // 2. Tamper exactly ONE byte in calldata_hex (e.g. modify recipient address within calldata)
        let mut tampered_calldata = valid_calldata.clone();
        // Replace last character from '0' to '1'
        let last_char = tampered_calldata.pop().unwrap();
        let replacement_char = if last_char == '0' { '1' } else { '0' };
        tampered_calldata.push(replacement_char);

        let tampered_payload = crate::wallet::tx::BscCanonicalPayload {
            calldata_hex: tampered_calldata,
            ..valid_payload.clone()
        };

        // Canonical hash MUST be completely different
        let hash_tampered = tampered_payload.canonical_hash();
        assert_ne!(hash_valid, hash_tampered, "Tampering 1 byte of calldata MUST produce a different hash");

        // from_canonical_payload MUST strictly reject tampered calldata
        let tampered_policy_res = BscSigningPolicy::from_canonical_payload(&tampered_payload, "0x1111111111111111111111111111111111111111");
        assert!(tampered_policy_res.is_err(), "Policy construction MUST reject tampered calldata");
        let err_msg = tampered_policy_res.unwrap_err();
        assert!(err_msg.contains("Calldata mismatch"), "Error must report calldata mismatch: {}", err_msg);

        // 3. Chain ID tampering MUST be rejected
        let mut mainnet_payload = valid_payload.clone();
        mainnet_payload.chain_id = 56; // BSC Mainnet
        let mainnet_res = BscSigningPolicy::from_canonical_payload(&mainnet_payload, "0x1111111111111111111111111111111111111111");
        assert!(mainnet_res.is_err());
        assert!(mainnet_res.unwrap_err().contains("Invalid chain ID"));

        // 4. Unauthorized contract MUST be rejected
        let mut wrong_contract_payload = valid_payload.clone();
        wrong_contract_payload.token_contract = Some("0x55d398326f99059fF775485246999027B3197955".to_string()); // Mainnet USDT
        let wrong_contract_res = BscSigningPolicy::from_canonical_payload(&wrong_contract_payload, "0x1111111111111111111111111111111111111111");
        assert!(wrong_contract_res.is_err());
        assert!(wrong_contract_res.unwrap_err().contains("Unauthorized contract"));
    }
}
