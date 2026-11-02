use bitcoin::{
    ecdsa::Signature as BtcSignature,
    hashes::Hash,
    secp256k1::{Message, Secp256k1, SecretKey},
    sighash::{EcdsaSighashType, SighashCache},
    OutPoint, Psbt, ScriptBuf, Witness,
};

use crate::wallet::tx::policy;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovedPrevout {
    pub outpoint: OutPoint,
    pub value_satoshis: u64,
    pub script_pubkey: ScriptBuf,
    pub change: u32, // 0 for receive branch, 1 for change branch
    pub derivation_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BtcSigningPolicy {
    pub expected_recipient: ScriptBuf,
    pub expected_amount_sat: u64,
    pub expected_change_script: Option<ScriptBuf>,
    pub approved_inputs: Vec<ApprovedPrevout>,
}

/// Independently validate PSBT inputs, outputs, amounts, recipient, change, and fees against approved draft policy.
/// Guarantees:
/// 1. Every input matches an approved OutPoint (txid:vout), EXACT satoshi value, and EXACT script_pubkey.
/// 2. Total input = sum of approved inputs.
/// 3. Recipient output matches exact approved script and exact amount.
/// 4. Any change output matches approved change script. No unrecognized outputs allowed.
/// 5. Fee = total_in - total_out strictly adheres to policy (fee <= 50,000 sats, fee ratio < 25%).
pub fn validate_psbt_before_signing(
    psbt: &Psbt,
    policy: &BtcSigningPolicy,
) -> Result<u64, String> {
    if psbt.inputs.is_empty() {
        return Err("PSBT has no inputs".into());
    }

    if psbt.inputs.len() != policy.approved_inputs.len() {
        return Err(format!(
            "PSBT input count ({}) does not match approved input count ({})",
            psbt.inputs.len(),
            policy.approved_inputs.len()
        ));
    }

    let mut total_in_sat = 0u64;

    // Verify every input against approved inputs list
    for (i, input) in psbt.inputs.iter().enumerate() {
        let utxo = input
            .witness_utxo
            .as_ref()
            .ok_or_else(|| format!("Input {} missing witness_utxo", i))?;

        let txin = &psbt.unsigned_tx.input[i];
        let approved = &policy.approved_inputs[i];

        if txin.previous_output != approved.outpoint {
            return Err(format!(
                "Input {} OutPoint mismatch: expected {}, found {}",
                i, approved.outpoint, txin.previous_output
            ));
        }

        let input_val = utxo.value.to_sat();
        if input_val != approved.value_satoshis {
            return Err(format!(
                "Input {} value mismatch: expected {} sats, found {} sats (tampering detected)",
                i, approved.value_satoshis, input_val
            ));
        }

        if utxo.script_pubkey != approved.script_pubkey {
            return Err(format!(
                "Input {} script_pubkey mismatch: expected {}, found {} (tampering detected)",
                i, approved.script_pubkey, utxo.script_pubkey
            ));
        }

        total_in_sat = total_in_sat
            .checked_add(input_val)
            .ok_or_else(|| "Input sum overflow".to_string())?;
    }

    let mut total_out_sat = 0u64;
    let mut recipient_found = false;

    for (i, output) in psbt.unsigned_tx.output.iter().enumerate() {
        let out_val = output.value.to_sat();
        total_out_sat = total_out_sat
            .checked_add(out_val)
            .ok_or_else(|| "Output sum overflow".to_string())?;

        if output.script_pubkey == policy.expected_recipient {
            if out_val == policy.expected_amount_sat {
                recipient_found = true;
            } else {
                return Err(format!(
                    "Output {} matches recipient script but amount mismatch: expected {}, found {}",
                    i, policy.expected_amount_sat, out_val
                ));
            }
        } else if let Some(ref change_script) = policy.expected_change_script {
            if &output.script_pubkey != change_script {
                return Err(format!(
                    "Output {} has unexpected script (neither approved recipient nor approved change)",
                    i
                ));
            }
        } else {
            return Err(format!(
                "Output {} has unrecognized recipient script when no change is expected",
                i
            ));
        }
    }

    if !recipient_found {
        return Err("Approved recipient output not found in PSBT".into());
    }

    if total_in_sat <= total_out_sat {
        return Err("Total input satoshis must exceed total output satoshis to pay network fee".into());
    }

    let fee_sat = total_in_sat - total_out_sat;
    policy::validate_btc_fee(policy.expected_amount_sat, fee_sat)?;

    Ok(fee_sat)
}

/// Sign a specific Native SegWit (P2WPKH) input in a PSBT using BIP-143 / BIP-174.
/// 
/// Validates:
/// 1. Input index exists and has `witness_utxo`.
/// 2. `witness_utxo.script_pubkey` matches the P2WPKH script derived from `privkey`.
/// 3. Computes BIP-143 SegWit sighash.
/// 4. Generates canonical low-S ECDSA signature.
/// 5. Adds the signature to `psbt.inputs[input_idx].partial_sigs` per BIP-174.
pub fn sign_p2wpkh_psbt_input(
    psbt: &mut Psbt,
    input_idx: usize,
    secp: &Secp256k1<secp256k1::All>,
    privkey: &SecretKey,
) -> Result<(), String> {
    if input_idx >= psbt.inputs.len() {
        return Err(format!("Input index {} out of range", input_idx));
    }

    let input = &psbt.inputs[input_idx];
    let witness_utxo = input
        .witness_utxo
        .as_ref()
        .ok_or_else(|| format!("Input {} is missing witness_utxo", input_idx))?;

    let pubkey = privkey.public_key(secp);
    let compressed_pubkey = pubkey.serialize();
    let btc_pubkey = bitcoin::PublicKey::from_slice(&compressed_pubkey)
        .map_err(|e| format!("Invalid public key slice: {}", e))?;
    let wpubkey_hash = btc_pubkey
        .wpubkey_hash()
        .map_err(|e| format!("Public key is not compressed, invalid for SegWit: {}", e))?;

    // Enforce that witness_utxo script matches the signing key
    let expected_script = ScriptBuf::new_p2wpkh(&wpubkey_hash);
    if witness_utxo.script_pubkey != expected_script {
        return Err(format!(
            "Input {} script_pubkey does not match signing key (expected P2WPKH for {})",
            input_idx, pubkey
        ));
    }

    let sighash_type = EcdsaSighashType::All;

    let mut sighash_cache = SighashCache::new(&psbt.unsigned_tx);
    let sighash = sighash_cache
        .p2wpkh_signature_hash(input_idx, &expected_script, witness_utxo.value, sighash_type)
        .map_err(|e| format!("BIP-143 sighash calculation failed: {}", e))?;

    let msg = Message::from_digest(sighash.to_byte_array());
    let sig = secp.sign_ecdsa(&msg, privkey);
    let btc_sig = BtcSignature {
        signature: sig,
        sighash_type,
    };

    // BIP-174: Add to partial_sigs
    psbt.inputs[input_idx].partial_sigs.insert(btc_pubkey, btc_sig);
    Ok(())
}

/// Finalize a signed P2WPKH PSBT input by constructing the `final_script_witness` from `partial_sigs`.
pub fn finalize_p2wpkh_psbt_input(
    psbt: &mut Psbt,
    input_idx: usize,
) -> Result<(), String> {
    if input_idx >= psbt.inputs.len() {
        return Err(format!("Input index {} out of range", input_idx));
    }

    let input = &mut psbt.inputs[input_idx];
    if input.partial_sigs.is_empty() {
        return Err(format!("Input {} has no partial signatures to finalize", input_idx));
    }

    let (pubkey, sig) = input.partial_sigs.iter().next().unwrap();
    let der_sig_bytes = sig.to_vec();
    let pubkey_bytes = pubkey.to_bytes();

    let mut witness = Witness::new();
    witness.push(der_sig_bytes);
    witness.push(pubkey_bytes);

    input.final_script_witness = Some(witness);
    input.partial_sigs.clear();
    Ok(())
}

/// Secure pre-sign verified workflow for multi-input PSBTs where each input may be
/// derived from a different BIP-84 index (HD derivation).
pub fn sign_and_finalize_verified_psbt_multi(
    psbt: &mut Psbt,
    policy: &BtcSigningPolicy,
    secp: &Secp256k1<secp256k1::All>,
    privkeys: &[SecretKey],
) -> Result<u64, String> {
    if privkeys.len() != psbt.inputs.len() {
        return Err(format!(
            "Key count mismatch: {} keys provided for {} PSBT inputs",
            privkeys.len(),
            psbt.inputs.len()
        ));
    }

    // 1. Mandatory pre-signing validation
    let fee = validate_psbt_before_signing(psbt, policy)?;

    // 2. Sign and finalize every input per BIP-174 using its own derived key
    for (i, key) in privkeys.iter().enumerate() {
        sign_p2wpkh_psbt_input(psbt, i, secp, key)?;
        finalize_p2wpkh_psbt_input(psbt, i)?;
    }

    Ok(fee)
}

/// Secure pre-sign verified workflow: Validates PSBT against policy FIRST,
/// and only proceeds to signing and finalizing all inputs if validation passes.
pub fn sign_and_finalize_verified_psbt(
    psbt: &mut Psbt,
    policy: &BtcSigningPolicy,
    secp: &Secp256k1<secp256k1::All>,
    privkey: &SecretKey,
) -> Result<u64, String> {
    let keys = vec![*privkey; psbt.inputs.len()];
    sign_and_finalize_verified_psbt_multi(psbt, policy, secp, &keys)
}

/// Cryptographically verifies that a P2WPKH input in a PSBT has a valid BIP-143 signature.
pub fn verify_p2wpkh_psbt_input(
    psbt: &Psbt,
    input_idx: usize,
    secp: &Secp256k1<secp256k1::All>,
) -> Result<(), String> {
    if input_idx >= psbt.inputs.len() {
        return Err("Input index out of bounds".into());
    }

    let input = &psbt.inputs[input_idx];
    let witness_utxo = input
        .witness_utxo
        .as_ref()
        .ok_or_else(|| format!("Input {} missing witness_utxo", input_idx))?;

    let witness = input
        .final_script_witness
        .as_ref()
        .ok_or_else(|| format!("Input {} missing final_script_witness", input_idx))?;

    if witness.len() != 2 {
        return Err(format!(
            "Expected 2 witness items for P2WPKH, got {}",
            witness.len()
        ));
    }

    let sig_bytes = witness.nth(0).ok_or("Missing signature in witness")?;
    let pubkey_bytes = witness.nth(1).ok_or("Missing pubkey in witness")?;

    let btc_sig = BtcSignature::from_slice(sig_bytes)
        .map_err(|e| format!("Invalid DER signature in witness: {}", e))?;
    let pubkey = secp256k1::PublicKey::from_slice(pubkey_bytes)
        .map_err(|e| format!("Invalid public key in witness: {}", e))?;

    let btc_pubkey = bitcoin::PublicKey::from_slice(pubkey_bytes)
        .map_err(|e| format!("Invalid Bitcoin pubkey in witness: {}", e))?;
    let wpubkey_hash = btc_pubkey
        .wpubkey_hash()
        .map_err(|e| format!("Public key in witness is not compressed: {}", e))?;
    let expected_script = ScriptBuf::new_p2wpkh(&wpubkey_hash);
    if witness_utxo.script_pubkey != expected_script {
        return Err("Witness pubkey does not match witness_utxo script_pubkey".into());
    }

    let mut sighash_cache = SighashCache::new(&psbt.unsigned_tx);
    let sighash = sighash_cache
        .p2wpkh_signature_hash(input_idx, &witness_utxo.script_pubkey, witness_utxo.value, btc_sig.sighash_type)
        .map_err(|e| format!("Failed to compute sighash: {}", e))?;

    let msg = Message::from_digest(sighash.to_byte_array());
    secp.verify_ecdsa(&msg, &btc_sig.signature, &pubkey)
        .map_err(|e| format!("BIP-143 ECDSA verification failed: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{
        absolute::LockTime, transaction::Version, Amount, Sequence,
        Transaction, TxIn, TxOut, Txid,
    };
    use std::str::FromStr;

    fn build_fixture_psbt() -> (Psbt, SecretKey, BtcSigningPolicy) {
        let secp = Secp256k1::new();
        let privkey = SecretKey::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
        let pubkey = privkey.public_key(&secp);
        let btc_pubkey = bitcoin::PublicKey::from_slice(&pubkey.serialize()).unwrap();
        let wallet_script = ScriptBuf::new_p2wpkh(&btc_pubkey.wpubkey_hash().unwrap());

        // Recipient key & script
        let rec_privkey = SecretKey::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap();
        let rec_pubkey = rec_privkey.public_key(&secp);
        let rec_btc_pubkey = bitcoin::PublicKey::from_slice(&rec_pubkey.serialize()).unwrap();
        let recipient_script = ScriptBuf::new_p2wpkh(&rec_btc_pubkey.wpubkey_hash().unwrap());

        let change_script = wallet_script.clone();

        let prev_txid = Txid::from_str("0101010101010101010101010101010101010101010101010101010101010101").unwrap();
        let prevout_point = OutPoint::new(prev_txid, 0);
        let prevout_value = Amount::from_sat(50_000);

        let tx_in = TxIn {
            previous_output: prevout_point,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };

        let send_amount = 30_000;
        let change_amount = 18_000;
        // Total in = 50,000, Total out = 48,000, Fee = 2,000 sats

        let tx_out_send = TxOut {
            value: Amount::from_sat(send_amount),
            script_pubkey: recipient_script.clone(),
        };
        let tx_out_change = TxOut {
            value: Amount::from_sat(change_amount),
            script_pubkey: change_script.clone(),
        };

        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![tx_in],
            output: vec![tx_out_send, tx_out_change],
        };

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).unwrap();
        psbt.inputs[0].witness_utxo = Some(TxOut {
            value: prevout_value,
            script_pubkey: wallet_script.clone(),
        });

        let policy = BtcSigningPolicy {
            expected_recipient: recipient_script,
            expected_amount_sat: send_amount,
            expected_change_script: Some(change_script),
            approved_inputs: vec![ApprovedPrevout {
                outpoint: prevout_point,
                value_satoshis: 50_000,
                script_pubkey: wallet_script,
                change: 0,
                derivation_index: 0,
            }],
        };

        (psbt, privkey, policy)
    }

    #[test]
    fn it02_psbt_fixture_input_output_balance_and_signing() {
        let secp = Secp256k1::new();
        let (mut psbt, privkey, policy) = build_fixture_psbt();

        // 1. Unified pre-signing validation & BIP-174 signing/finalization
        let fee = sign_and_finalize_verified_psbt(&mut psbt, &policy, &secp, &privkey)
            .expect("Signing verified PSBT must succeed");
        assert_eq!(fee, 2000, "Fee must equal 2000 sats (50k - 48k)");

        // 2. Verify witness is generated
        assert!(psbt.inputs[0].final_script_witness.is_some());
        assert_eq!(psbt.inputs[0].partial_sigs.len(), 0);

        // 3. Verify signature on untampered transaction
        verify_p2wpkh_psbt_input(&psbt, 0, &secp).expect("Witness must be cryptographically valid");
    }

    #[test]
    fn it03_psbt_pre_sign_tamper_rejections_and_sub_satoshi_integrity() {
        let secp = Secp256k1::new();
        let (psbt, privkey, policy) = build_fixture_psbt();

        // 1. TAMPER A: Diverting recipient script
        let mut tampered_recipient = psbt.clone();
        let attacker_script = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::all_zeros());
        tampered_recipient.unsigned_tx.output[0].script_pubkey = attacker_script;
        let res_tamper_rec = validate_psbt_before_signing(&tampered_recipient, &policy);
        assert!(
            res_tamper_rec.is_err(),
            "Signer must reject PSBT when recipient script was altered"
        );

        // 2. TAMPER B: Inflating recipient amount (e.g. 30,000 -> 30,001 or 35,000)
        let mut tampered_amount = psbt.clone();
        tampered_amount.unsigned_tx.output[0].value = Amount::from_sat(30_001);
        let res_tamper_amt = validate_psbt_before_signing(&tampered_amount, &policy);
        assert!(
            res_tamper_amt.is_err(),
            "Signer must reject PSBT even on 1-sat discrepancy in recipient amount"
        );

        // 3. TAMPER C: Diverting change output
        let mut tampered_change = psbt.clone();
        let attacker_change = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::all_zeros());
        tampered_change.unsigned_tx.output[1].script_pubkey = attacker_change;
        let res_tamper_change = validate_psbt_before_signing(&tampered_change, &policy);
        assert!(
            res_tamper_change.is_err(),
            "Signer must reject PSBT when change address was altered"
        );

        // 4. TAMPER D (SUB-SATOSHI / 1-SAT PREVOUT TAMPERING):
        // Attacker claims UTXO has 50,001 sats instead of 50,000 (+1 satoshi discrepancy).
        // Total fee would be 2,001 sats (which by itself is <= 50,000 fee cap).
        // BUT because validator checks against approved_inputs, it MUST REJECT!
        let mut tampered_1sat_utxo = psbt.clone();
        tampered_1sat_utxo.inputs[0].witness_utxo.as_mut().unwrap().value = Amount::from_sat(50_001);
        let res_1sat = validate_psbt_before_signing(&tampered_1sat_utxo, &policy);
        assert!(
            res_1sat.is_err(),
            "Signer must reject PSBT when UTXO amount differs by even 1 satoshi from approved inputs"
        );
        let err_msg = res_1sat.unwrap_err();
        assert!(err_msg.contains("Input 0 value mismatch"), "Error message must specify value mismatch: {}", err_msg);

        // 5. TAMPER E: OutPoint vout tampering (trying to spend vout 1 instead of vout 0)
        let mut tampered_outpoint = psbt.clone();
        tampered_outpoint.unsigned_tx.input[0].previous_output.vout = 1;
        let res_outpoint = validate_psbt_before_signing(&tampered_outpoint, &policy);
        assert!(
            res_outpoint.is_err(),
            "Signer must reject PSBT when OutPoint does not match approved input"
        );

        // 6. MANDATORY PRE-SIGN ENFORCEMENT:
        // Attempting to call sign_and_finalize_verified_psbt on any tampered PSBT fails immediately
        let mut tampered_sign_attempt = tampered_1sat_utxo.clone();
        let sign_attempt_res = sign_and_finalize_verified_psbt(&mut tampered_sign_attempt, &policy, &secp, &privkey);
        assert!(
            sign_attempt_res.is_err(),
            "sign_and_finalize_verified_psbt must refuse to sign when validation fails"
        );

        // 7. TAMPER G: Tampering after signing must be caught by witness verifier
        let mut signed_psbt = psbt.clone();
        sign_and_finalize_verified_psbt(&mut signed_psbt, &policy, &secp, &privkey).unwrap();

        // Altering output value after signing
        let mut tampered_post_sign = signed_psbt.clone();
        tampered_post_sign.unsigned_tx.output[0].value = Amount::from_sat(20_000);
        assert!(
            verify_p2wpkh_psbt_input(&tampered_post_sign, 0, &secp).is_err(),
            "Signature must fail if transaction was altered after signing"
        );
    }

    #[test]
    fn it03_multi_input_distinct_bip84_derivation_signing_and_script_tamper() {
        let secp = Secp256k1::new();

        // 1. Derive two distinct BIP84 private keys (e.g. index 0 and index 1)
        let privkey0 = SecretKey::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
        let privkey1 = SecretKey::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap();

        let pub0 = privkey0.public_key(&secp);
        let btc_pub0 = bitcoin::PublicKey::from_slice(&pub0.serialize()).unwrap();
        let script0 = ScriptBuf::new_p2wpkh(&btc_pub0.wpubkey_hash().unwrap());

        let pub1 = privkey1.public_key(&secp);
        let btc_pub1 = bitcoin::PublicKey::from_slice(&pub1.serialize()).unwrap();
        let script1 = ScriptBuf::new_p2wpkh(&btc_pub1.wpubkey_hash().unwrap());

        assert_ne!(script0, script1, "Scripts must be distinct");

        let outpoint0 = OutPoint::from_str("1111111111111111111111111111111111111111111111111111111111111111:0").unwrap();
        let outpoint1 = OutPoint::from_str("2222222222222222222222222222222222222222222222222222222222222222:1").unwrap();

        let tx_in0 = TxIn {
            previous_output: outpoint0,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };
        let tx_in1 = TxIn {
            previous_output: outpoint1,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };

        let recipient_script = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::all_zeros());
        let tx_out = TxOut {
            value: Amount::from_sat(75_000),
            script_pubkey: recipient_script.clone(),
        };

        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![tx_in0, tx_in1],
            output: vec![tx_out],
        };

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).unwrap();
        // Input 0: 50,000 sats
        psbt.inputs[0].witness_utxo = Some(TxOut {
            value: Amount::from_sat(50_000),
            script_pubkey: script0.clone(),
        });
        // Input 1: 30,000 sats (Total in: 80,000, Total out: 75,000 -> Fee: 5,000)
        psbt.inputs[1].witness_utxo = Some(TxOut {
            value: Amount::from_sat(30_000),
            script_pubkey: script1.clone(),
        });

        let policy = BtcSigningPolicy {
            expected_recipient: recipient_script,
            expected_amount_sat: 75_000,
            expected_change_script: None,
            approved_inputs: vec![
                ApprovedPrevout {
                    outpoint: outpoint0,
                    value_satoshis: 50_000,
                    script_pubkey: script0.clone(),
                    change: 0,
                    derivation_index: 0,
                },
                ApprovedPrevout {
                    outpoint: outpoint1,
                    value_satoshis: 30_000,
                    script_pubkey: script1.clone(),
                    change: 1, // Change keychain branch
                    derivation_index: 1,
                },
            ],
        };

        // 2. Multi-key signing success
        let mut psbt_valid = psbt.clone();
        let fee = sign_and_finalize_verified_psbt_multi(
            &mut psbt_valid,
            &policy,
            &secp,
            &[privkey0, privkey1],
        ).expect("Multi-key signing must succeed");
        assert_eq!(fee, 5000);

        // Verify both inputs cryptographically
        verify_p2wpkh_psbt_input(&psbt_valid, 0, &secp).expect("Input 0 signature must verify");
        verify_p2wpkh_psbt_input(&psbt_valid, 1, &secp).expect("Input 1 signature must verify");

        // 3. TAMPER TEST: Tampering with script_pubkey on Input 1
        let mut psbt_tampered_script = psbt.clone();
        psbt_tampered_script.inputs[1].witness_utxo.as_mut().unwrap().script_pubkey = script0.clone(); // fake script
        let err = validate_psbt_before_signing(&psbt_tampered_script, &policy).unwrap_err();
        assert!(err.contains("Input 1 script_pubkey mismatch"), "Error was: {}", err);
    }
}
