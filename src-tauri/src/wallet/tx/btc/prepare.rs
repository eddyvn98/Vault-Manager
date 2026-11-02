use std::str::FromStr;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bitcoin::{
    absolute::LockTime,
    transaction::Version,
    Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    psbt::Psbt,
    Network,
};
use reqwest::Client;
use crate::wallet::{
    bip,
    blockchain,
    tx::{
        policy::{self, DUST_LIMIT_SATOSHIS, DEFAULT_SIGNET_FEE_RATE},
        types::{BtcTxPreview, UtxoInfo},
        btc::utxo,
    },
};

pub async fn prepare_signet_tx(
    mnemonic: &str,
    recipient: &str,
    amount_satoshis: u64,
    fee_rate_opt: Option<u64>,
) -> Result<BtcTxPreview, String> {
    // 1. Validate inputs and policies
    let recipient_addr = policy::validate_btc_signet_recipient(recipient)?;
    policy::validate_btc_amount(amount_satoshis)?;

    let fee_rate = fee_rate_opt.unwrap_or(DEFAULT_SIGNET_FEE_RATE).max(1);

    // 2. Derive wallet receive and change addresses
    let from_address = bip::derive_btc_address(mnemonic, Network::Signet)?;
    let change_address = bip::derive_btc_address_at(mnemonic, Network::Signet, 1, 0)?;
    let change_addr = bitcoin::Address::from_str(&change_address)
        .map_err(|e| format!("Invalid change address: {}", e))?
        .require_network(Network::Signet)
        .map_err(|_| "Change address network mismatch".to_string())?;

    // 3. Fetch UTXOs from Esplora
    let client = Client::new();
    let available_utxos = utxo::fetch_signet_utxos(&client, &from_address).await?;

    // 4. Initial fee estimation (approx 141 vB for 1 in, 2 out P2WPKH)
    let initial_est_vbytes = 141u64;
    let initial_est_fee = initial_est_vbytes * fee_rate;

    // 5. Select UTXOs
    let (selected, total_in) = utxo::select_utxos_greedy(&available_utxos, amount_satoshis, initial_est_fee)?;

    // 6. Recalculate precise vBytes: 68 vB per input + 31 vB per output + 11 vB overhead
    let num_inputs = selected.len() as u64;
    let has_change_approx = total_in > amount_satoshis + (num_inputs * 68 + 2 * 31 + 11) * fee_rate;
    let num_outputs = if has_change_approx { 2u64 } else { 1u64 };
    let exact_vbytes = num_inputs * 68 + num_outputs * 31 + 11;
    let exact_fee = exact_vbytes * fee_rate;

    if total_in < amount_satoshis + exact_fee {
        return Err(format!(
            "Insufficient funds after exact fee calculation: available {} sats, need {} sats",
            total_in, amount_satoshis + exact_fee
        ));
    }

    let raw_change = total_in - amount_satoshis - exact_fee;
    let (change_satoshis, final_fee) = if raw_change >= DUST_LIMIT_SATOSHIS {
        (raw_change, exact_fee)
    } else {
        // Dust change is absorbed into fee
        (0, exact_fee + raw_change)
    };

    // 7. Construct Unsigned Transaction
    let mut tx_inputs = Vec::with_capacity(selected.len());
    let mut witness_utxos = Vec::with_capacity(selected.len());
    let from_script = bitcoin::Address::from_str(&from_address)
        .map_err(|e| e.to_string())?
        .require_network(Network::Signet)
        .map_err(|e| e.to_string())?
        .script_pubkey();

    for u in &selected {
        let txid = Txid::from_str(&u.txid).map_err(|e| format!("Invalid txid in UTXO: {}", e))?;
        tx_inputs.push(TxIn {
            previous_output: OutPoint::new(txid, u.vout),
            script_sig: ScriptBuf::new(), // Empty for SegWit native
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: bitcoin::Witness::new(),
        });
        witness_utxos.push(TxOut {
            value: Amount::from_sat(u.value),
            script_pubkey: from_script.clone(),
        });
    }

    let mut tx_outputs = Vec::new();
    tx_outputs.push(TxOut {
        value: Amount::from_sat(amount_satoshis),
        script_pubkey: recipient_addr.script_pubkey(),
    });

    if change_satoshis >= DUST_LIMIT_SATOSHIS {
        tx_outputs.push(TxOut {
            value: Amount::from_sat(change_satoshis),
            script_pubkey: change_addr.script_pubkey(),
        });
    }

    let unsigned_tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: tx_inputs,
        output: tx_outputs,
    };

    // 8. Build PSBT
    let mut psbt = Psbt::from_unsigned_tx(unsigned_tx)
        .map_err(|e| format!("Failed to create PSBT from unsigned tx: {}", e))?;

    for (i, witness_utxo) in witness_utxos.into_iter().enumerate() {
        if let Some(input) = psbt.inputs.get_mut(i) {
            input.witness_utxo = Some(witness_utxo);
        }
    }

    let psbt_bytes = psbt.serialize();
    let psbt_b64 = BASE64.encode(&psbt_bytes);

    let draft_id = format!("draft-btc-{}", &psbt_b64[..12].replace('/', "_"));

    let utxo_infos = selected
        .into_iter()
        .map(|u| UtxoInfo {
            txid: u.txid,
            vout: u.vout,
            value_satoshis: u.value,
            address: from_address.clone(),
        })
        .collect();

    Ok(BtcTxPreview {
        draft_id,
        network: "Bitcoin Signet".into(),
        recipient_address: recipient_addr.to_string(),
        amount_satoshis,
        formatted_amount_btc: blockchain::format_satoshis(amount_satoshis),
        fee_satoshis: final_fee,
        formatted_fee_btc: blockchain::format_satoshis(final_fee),
        fee_rate_sat_per_vb: fee_rate,
        selected_utxos: utxo_infos,
        change_address: if change_satoshis > 0 { change_addr.to_string() } else { "None (absorbed in fee)".into() },
        change_satoshis,
        unsigned_psbt_base64: psbt_b64,
        signing_enabled: false,
        note: "Bitcoin Signet Unsigned PSBT Preview (P2C-1: Signing is not enabled)".into(),
    })
}
