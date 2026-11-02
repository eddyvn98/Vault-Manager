use bitcoin::{
    ecdsa::Signature as BtcSignature,
    hashes::Hash,
    secp256k1::{Message, Secp256k1, SecretKey},
    sighash::{EcdsaSighashType, SighashCache},
    Psbt, ScriptBuf, Witness,
};

/// Sign a specific Native SegWit (P2WPKH) input in a PSBT using BIP-143 / BIP-174.
/// 
/// Validates:
/// 1. Input index exists and has `witness_utxo`.
/// 2. `witness_utxo.script_pubkey` matches the P2WPKH script derived from `privkey`.
/// 3. Computes BIP-143 SegWit sighash.
/// 4. Generates canonical low-S ECDSA signature.
/// 5. Populates `final_script_witness` with `[<der_signature_with_sighash_type>, <compressed_pubkey>]`.
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

    let der_sig_bytes = btc_sig.to_vec();

    // Attach witness: [sig, pubkey]
    let mut witness = Witness::new();
    witness.push(der_sig_bytes);
    witness.push(compressed_pubkey.to_vec());

    psbt.inputs[input_idx].final_script_witness = Some(witness);
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
        absolute::LockTime, transaction::Version, Amount, Network, OutPoint, Sequence,
        Transaction, TxIn, TxOut, Txid,
    };
    use std::str::FromStr;

    #[test]
    fn bip143_p2wpkh_signing_and_tamper_verification() {
        let secp = Secp256k1::new();

        // 1. Fixed test private key
        let test_privkey_hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let privkey = SecretKey::from_str(test_privkey_hex).unwrap();
        let pubkey = privkey.public_key(&secp);
        let btc_pubkey = bitcoin::PublicKey::from_slice(&pubkey.serialize()).unwrap();
        let p2wpkh_script = ScriptBuf::new_p2wpkh(&btc_pubkey.wpubkey_hash().unwrap());

        // Derived address should be valid SegWit tb1...
        let addr = bitcoin::Address::from_script(&p2wpkh_script, Network::Signet).unwrap();
        assert!(addr.to_string().starts_with("tb1q"));

        // 2. Build Unsigned Transaction
        let prev_txid = Txid::from_str("0101010101010101010101010101010101010101010101010101010101010101").unwrap();
        let prevout_value = Amount::from_sat(50_000);

        let tx_in = TxIn {
            previous_output: OutPoint::new(prev_txid, 0),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        };

        let recipient_script = ScriptBuf::new_p2wpkh(&btc_pubkey.wpubkey_hash().unwrap());
        let tx_out = TxOut {
            value: Amount::from_sat(48_000),
            script_pubkey: recipient_script,
        };

        let unsigned_tx = Transaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![tx_in],
            output: vec![tx_out],
        };

        // 3. Create PSBT and attach witness_utxo
        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).unwrap();
        psbt.inputs[0].witness_utxo = Some(TxOut {
            value: prevout_value,
            script_pubkey: p2wpkh_script.clone(),
        });

        // 4. Test Sign with Wrong Key -> must fail script_pubkey match check
        let wrong_privkey = SecretKey::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap();
        let wrong_res = sign_p2wpkh_psbt_input(&mut psbt, 0, &secp, &wrong_privkey);
        assert!(wrong_res.is_err(), "Signing with non-matching key must be rejected");

        // 5. Test Sign with Correct Key -> must succeed
        if let Err(e) = sign_p2wpkh_psbt_input(&mut psbt, 0, &secp, &privkey) {
            panic!("Sign error: {}", e);
        }

        // 6. Verify signature on untampered PSBT -> must pass
        assert!(verify_p2wpkh_psbt_input(&psbt, 0, &secp).is_ok());

        // 7. TAMPER TEST A: Tampering with witness_utxo amount
        let mut tampered_psbt_amount = psbt.clone();
        tampered_psbt_amount.inputs[0].witness_utxo.as_mut().unwrap().value = Amount::from_sat(50_001);
        assert!(
            verify_p2wpkh_psbt_input(&tampered_psbt_amount, 0, &secp).is_err(),
            "Tampering with UTXO value must invalidate BIP-143 signature"
        );

        // 8. TAMPER TEST B: Tampering with output recipient or amount
        let mut tampered_psbt_output = psbt.clone();
        tampered_psbt_output.unsigned_tx.output[0].value = Amount::from_sat(40_000);
        assert!(
            verify_p2wpkh_psbt_input(&tampered_psbt_output, 0, &secp).is_err(),
            "Tampering with transaction output must invalidate BIP-143 signature"
        );

        // 9. TAMPER TEST C: Tampering with prevout OutPoint
        let mut tampered_psbt_in = psbt.clone();
        tampered_psbt_in.unsigned_tx.input[0].previous_output.vout = 1;
        assert!(
            verify_p2wpkh_psbt_input(&tampered_psbt_in, 0, &secp).is_err(),
            "Tampering with input prevout index must invalidate BIP-143 signature"
        );
    }
}
