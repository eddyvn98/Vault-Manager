use bitcoin::{
    ecdsa::Signature as BtcSignature,
    hashes::Hash,
    secp256k1::{Message, Secp256k1, SecretKey},
    sighash::{EcdsaSighashType, SighashCache},
    Psbt, ScriptBuf, Witness,
};

use crate::wallet::tx::policy;

/// Independently validate PSBT outputs, amounts, recipient, change, and fees before signing.
/// Protects against malicious or tampered PSBTs attempting to divert funds, inflate fees, or tamper with SegWit amounts.
pub fn validate_psbt_before_signing(
    psbt: &Psbt,
    expected_recipient: &ScriptBuf,
    expected_amount_sat: u64,
    expected_change_script: Option<&ScriptBuf>,
) -> Result<u64, String> {
    if psbt.inputs.is_empty() {
        return Err("PSBT has no inputs".into());
    }

    let mut total_in_sat = 0u64;
    for (i, input) in psbt.inputs.iter().enumerate() {
        let utxo = input
            .witness_utxo
            .as_ref()
            .ok_or_else(|| format!("Input {} missing witness_utxo", i))?;
        if utxo.value.to_sat() == 0 {
            return Err(format!("Input {} has zero amount", i));
        }
        total_in_sat = total_in_sat
            .checked_add(utxo.value.to_sat())
            .ok_or_else(|| "Input sum overflow".to_string())?;
    }

    let mut total_out_sat = 0u64;
    let mut recipient_found = false;

    for (i, output) in psbt.unsigned_tx.output.iter().enumerate() {
        let out_val = output.value.to_sat();
        total_out_sat = total_out_sat
            .checked_add(out_val)
            .ok_or_else(|| "Output sum overflow".to_string())?;

        if &output.script_pubkey == expected_recipient {
            if out_val == expected_amount_sat {
                recipient_found = true;
            } else {
                return Err(format!(
                    "Output {} matches recipient script but amount mismatch: expected {}, found {}",
                    i, expected_amount_sat, out_val
                ));
            }
        } else if let Some(change_script) = expected_change_script {
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
    policy::validate_btc_fee(expected_amount_sat, fee_sat)?;

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
        absolute::LockTime, transaction::Version, Amount, OutPoint, Sequence,
        Transaction, TxIn, TxOut, Txid,
    };
    use std::str::FromStr;

    fn build_fixture_psbt() -> (Psbt, SecretKey, ScriptBuf, ScriptBuf, u64, u64) {
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
        let prevout_value = Amount::from_sat(50_000);

        let tx_in = TxIn {
            previous_output: OutPoint::new(prev_txid, 0),
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
            script_pubkey: wallet_script,
        });

        (psbt, privkey, recipient_script, change_script, send_amount, 2_000)
    }

    #[test]
    fn it02_psbt_fixture_input_output_balance_and_signing() {
        let secp = Secp256k1::new();
        let (mut psbt, privkey, recipient_script, change_script, send_amount, expected_fee) =
            build_fixture_psbt();

        // 1. Pre-signing validation: verify balance: sum(input) == sum(output) + fee
        let fee = validate_psbt_before_signing(
            &psbt,
            &recipient_script,
            send_amount,
            Some(&change_script),
        )
        .expect("Pre-signing validation must pass on valid fixture");
        assert_eq!(fee, expected_fee, "Fee must equal 2000 sats (50k - 48k)");

        // 2. BIP-174 Sign input: adds partial_sigs
        sign_p2wpkh_psbt_input(&mut psbt, 0, &secp, &privkey)
            .expect("Must sign P2WPKH input");
        assert_eq!(psbt.inputs[0].partial_sigs.len(), 1);
        assert!(psbt.inputs[0].final_script_witness.is_none());

        // 3. Finalize input: moves partial_sigs to final_script_witness
        finalize_p2wpkh_psbt_input(&mut psbt, 0).expect("Must finalize input");
        assert_eq!(psbt.inputs[0].partial_sigs.len(), 0);
        assert!(psbt.inputs[0].final_script_witness.is_some());

        // 4. Verify signature on untampered transaction
        verify_p2wpkh_psbt_input(&psbt, 0, &secp).expect("Witness must be cryptographically valid");
    }

    #[test]
    fn it03_psbt_pre_sign_tamper_rejections_and_witness_integrity() {
        let secp = Secp256k1::new();
        let (psbt, privkey, recipient_script, change_script, send_amount, _) =
            build_fixture_psbt();

        // 1. TAMPER A: Diverting recipient script
        let mut tampered_recipient = psbt.clone();
        let attacker_script = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::all_zeros());
        tampered_recipient.unsigned_tx.output[0].script_pubkey = attacker_script;
        let res_tamper_rec = validate_psbt_before_signing(
            &tampered_recipient,
            &recipient_script,
            send_amount,
            Some(&change_script),
        );
        assert!(
            res_tamper_rec.is_err(),
            "Signer must reject PSBT when recipient script was altered"
        );

        // 2. TAMPER B: Inflating recipient amount (e.g. 30,000 -> 35,000)
        let mut tampered_amount = psbt.clone();
        tampered_amount.unsigned_tx.output[0].value = Amount::from_sat(35_000);
        let res_tamper_amt = validate_psbt_before_signing(
            &tampered_amount,
            &recipient_script,
            send_amount,
            Some(&change_script),
        );
        assert!(
            res_tamper_amt.is_err(),
            "Signer must reject PSBT when recipient amount was altered"
        );

        // 3. TAMPER C: Diverting change output
        let mut tampered_change = psbt.clone();
        let attacker_change = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::all_zeros());
        tampered_change.unsigned_tx.output[1].script_pubkey = attacker_change;
        let res_tamper_change = validate_psbt_before_signing(
            &tampered_change,
            &recipient_script,
            send_amount,
            Some(&change_script),
        );
        assert!(
            res_tamper_change.is_err(),
            "Signer must reject PSBT when change address was altered"
        );

        // 4. TAMPER D: Inflating UTXO value (Fee spoofing attack: claiming UTXO has 500,000 instead of 50,000)
        let mut tampered_utxo = psbt.clone();
        tampered_utxo.inputs[0].witness_utxo.as_mut().unwrap().value = Amount::from_sat(500_000);
        // This causes fee calculation to jump to 452,000 sats, which violates policy cap!
        let res_tamper_utxo = validate_psbt_before_signing(
            &tampered_utxo,
            &recipient_script,
            send_amount,
            Some(&change_script),
        );
        assert!(
            res_tamper_utxo.is_err(),
            "Signer must reject PSBT when fee policy is violated"
        );

        // 5. TAMPER E: Tampering after signing must be caught by witness verifier
        let mut signed_psbt = psbt.clone();
        sign_p2wpkh_psbt_input(&mut signed_psbt, 0, &secp, &privkey).unwrap();
        finalize_p2wpkh_psbt_input(&mut signed_psbt, 0).unwrap();

        // Altering output value after signing
        let mut tampered_post_sign = signed_psbt.clone();
        tampered_post_sign.unsigned_tx.output[0].value = Amount::from_sat(20_000);
        assert!(
            verify_p2wpkh_psbt_input(&tampered_post_sign, 0, &secp).is_err(),
            "Signature must fail if transaction was altered after signing"
        );
    }
}
