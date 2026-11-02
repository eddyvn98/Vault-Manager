use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bdk_esplora::{esplora_client::Builder as EsploraBuilder, EsploraAsyncExt};
use bitcoin::{Address, Amount, FeeRate, Network};
use rand::RngCore;

use crate::wallet::{
    blockchain,
    tx::{
        btc::{descriptors, store},
        policy,
        types::{BtcTxPreview, UtxoInfo},
    },
};

pub async fn prepare_signet_tx(
    mnemonic: &str,
    recipient: &str,
    amount_satoshis: u64,
    fee_rate_opt: Option<u64>,
    db_path: Option<&std::path::Path>,
) -> Result<BtcTxPreview, String> {
    // 1. Validate inputs and policies
    let recipient_addr = policy::validate_btc_signet_recipient(recipient)?;
    policy::validate_btc_amount(amount_satoshis)?;

    let fee_rate_val = fee_rate_opt.unwrap_or(policy::DEFAULT_SIGNET_FEE_RATE).max(1);
    if fee_rate_val > 1_000 {
        return Err(format!("Fee rate ({} sat/vB) exceeds maximum limit of 1000 sat/vB", fee_rate_val));
    }
    let fee_rate = FeeRate::from_sat_per_vb(fee_rate_val)
        .ok_or_else(|| format!("Invalid fee rate: {} sat/vB", fee_rate_val))?;

    // 2. Derive BIP84 watch-only descriptors (no private keys in descriptor)
    let desc = descriptors::derive_signet_bip84_descriptors(mnemonic)?;

    // 3. Initialize BDK Wallet with SQLite persistence
    let (mut wallet, mut conn) = store::open_or_init_wallet(db_path, &desc, Network::Signet)?;

    // 4. Sync chain state and UTXOs using bdk_esplora
    let esplora_client = EsploraBuilder::new("https://mempool.space/signet/api")
        .build_async()
        .map_err(|e| format!("Failed to create Esplora client: {}", e))?;

    let full_scan_req = wallet.start_full_scan();
    let scan_update = esplora_client
        .full_scan(full_scan_req, 5, 3)
        .await
        .map_err(|e| format!("Esplora full scan failed: {}", e))?;

    wallet
        .apply_update(scan_update)
        .map_err(|e| format!("Failed to apply blockchain update to BDK wallet: {}", e))?;

    wallet
        .persist(&mut conn)
        .map_err(|e| format!("Failed to persist BDK wallet after scan: {}", e))?;

    // 5. Build transaction using BDK's native transaction builder
    let mut builder = wallet.build_tx();
    builder
        .add_recipient(recipient_addr.script_pubkey(), Amount::from_sat(amount_satoshis))
        .fee_rate(fee_rate);

    let psbt = builder
        .finish()
        .map_err(|e| format!("BDK transaction build failed: {}", e))?;

    wallet
        .persist(&mut conn)
        .map_err(|e| format!("Failed to persist BDK wallet after tx build: {}", e))?;

    // 6. Verify Fee Policies
    let fee = wallet
        .calculate_fee(&psbt.unsigned_tx)
        .map_err(|e| format!("Failed to calculate PSBT fee: {}", e))?;
    let fee_sats = fee.to_sat();

    policy::validate_btc_fee(amount_satoshis, fee_sats)?;

    // 7. Extract input UTXO information and change output
    let mut selected_utxos = Vec::new();
    for (i, input) in psbt.inputs.iter().enumerate() {
        if let Some(prevout) = &input.witness_utxo {
            let txin = &psbt.unsigned_tx.input[i];
            let utxo_address = Address::from_script(&prevout.script_pubkey, Network::Signet)
                .map(|a| a.to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            selected_utxos.push(UtxoInfo {
                txid: txin.previous_output.txid.to_string(),
                vout: txin.previous_output.vout,
                value_satoshis: prevout.value.to_sat(),
                address: utxo_address,
            });
        }
    }

    let change_output = psbt
        .unsigned_tx
        .output
        .iter()
        .find(|o| o.script_pubkey != recipient_addr.script_pubkey());

    let (change_satoshis, change_address) = if let Some(change_out) = change_output {
        let addr = bitcoin::Address::from_script(&change_out.script_pubkey, Network::Signet)
            .map(|a| a.to_string())
            .unwrap_or_else(|_| {
                wallet
                    .peek_address(bdk_wallet::KeychainKind::Internal, 0)
                    .address
                    .to_string()
            });
        (change_out.value.to_sat(), addr)
    } else {
        (0, "None (absorbed in fee)".into())
    };

    // 8. Cryptographically secure random draft ID (32 hex characters)
    let mut rand_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut rand_bytes);
    let draft_id = format!("draft-btc-{}", hex::encode(rand_bytes));

    let psbt_bytes = psbt.serialize();
    let psbt_b64 = BASE64.encode(&psbt_bytes);

    Ok(BtcTxPreview {
        draft_id,
        network: "Bitcoin Signet".into(),
        recipient_address: recipient_addr.to_string(),
        amount_satoshis,
        formatted_amount_btc: blockchain::format_satoshis(amount_satoshis),
        fee_satoshis: fee_sats,
        formatted_fee_btc: blockchain::format_satoshis(fee_sats),
        fee_rate_sat_per_vb: fee_rate_val,
        selected_utxos,
        change_address,
        change_satoshis,
        unsigned_psbt_base64: psbt_b64,
        signing_enabled: false,
        note: "Bitcoin Signet Unsigned PSBT Preview (P2C-1: Built with BDK 3.2.0)".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bdk_wallet_initialization_with_derived_descriptors() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let desc = descriptors::derive_signet_bip84_descriptors(mnemonic).unwrap();

        let wallet = bdk_wallet::Wallet::create(desc.receive_descriptor, desc.change_descriptor)
            .network(Network::Signet)
            .create_wallet_no_persist();

        assert!(wallet.is_ok(), "BDK Wallet must initialize cleanly with derived watch-only descriptors");
    }

    #[tokio::test]
    async fn prepare_validates_signet_policies() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        
        // Invalid address (mainnet address rejected)
        let res_mainnet = prepare_signet_tx(
            mnemonic,
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu",
            10_000,
            Some(2),
            None,
        ).await;
        assert!(res_mainnet.is_err());

        // Amount below dust limit rejected
        let res_dust = prepare_signet_tx(
            mnemonic,
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            100,
            Some(2),
            None,
        ).await;
        assert!(res_dust.is_err());
    }
}
