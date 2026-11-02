use std::time::Duration;
use num_bigint::BigUint;
use num_traits::Num;
use rand::RngCore;
use reqwest::Client;
use crate::wallet::{
    bip,
    blockchain::{self, BSC_TESTNET_USDT_CONTRACT},
    tx::{
        policy::{self, BSC_TESTNET_CHAIN_ID},
        types::BscTxPreview,
        bsc::{calldata, gas},
    },
};

pub fn parse_decimal_to_wei_18(amount_str: &str) -> Result<BigUint, String> {
    let clean = amount_str.trim();
    if clean.is_empty() {
        return Err("Amount cannot be empty".into());
    }

    let parts: Vec<&str> = clean.split('.').collect();
    if parts.len() > 2 {
        return Err("Invalid decimal format".into());
    }

    let int_part_str = if parts[0].is_empty() { "0" } else { parts[0] };
    let int_part = BigUint::from_str_radix(int_part_str, 10)
        .map_err(|e| format!("Invalid integer amount: {}", e))?;

    let base = BigUint::from(10u64).pow(18);
    let mut total = int_part * &base;

    if parts.len() == 2 {
        let dec_str = parts[1];
        if dec_str.len() > 18 {
            return Err("Amount has more than 18 decimal places".into());
        }
        let padded_dec = format!("{:0<18}", dec_str);
        let dec_part = BigUint::from_str_radix(&padded_dec, 10)
            .map_err(|e| format!("Invalid fractional amount: {}", e))?;
        total += dec_part;
    }

    if total == BigUint::from(0u32) {
        return Err("Amount must be greater than zero".into());
    }

    Ok(total)
}

pub async fn prepare_bsc_testnet_tx(
    mnemonic: &str,
    recipient: &str,
    amount_str: &str,
) -> Result<BscTxPreview, String> {
    // 1. Validate recipient and amount
    let valid_recipient = policy::validate_evm_recipient(recipient)?;
    let amount_wei = parse_decimal_to_wei_18(amount_str)?;

    // 2. Derive sender address from mnemonic
    let from_address = bip::derive_bsc_address(mnemonic)?;

    // 3. Encode ERC-20 transfer calldata
    let calldata_hex = calldata::encode_erc20_transfer(&valid_recipient, &amount_wei)?;

    // 4. Connect to BSC Testnet RPC and verify chain ID
    let rpc_url = "https://data-seed-prebsc-1-s1.binance.org:8545";
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let chain_id_hex = blockchain::call_json_rpc(
        &client,
        rpc_url,
        "eth_chainId",
        serde_json::json!([]),
        1,
    )
    .await?;

    blockchain::verify_chain_id(&chain_id_hex, BSC_TESTNET_CHAIN_ID, "BSC Testnet")?;

    // 5. Query nonce and gas estimation
    let gas_info = gas::query_bsc_gas_and_nonce(
        &client,
        rpc_url,
        &from_address,
        BSC_TESTNET_USDT_CONTRACT,
        &calldata_hex,
    )
    .await?;

    let raw_hex = format!("0x{:x}", amount_wei);
    let formatted_amount = blockchain::format_wei_18(&raw_hex)?;

    let mut rand_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut rand_bytes);
    let draft_id = format!("draft-bsc-{}", hex::encode(rand_bytes));

    let unsigned_summary = format!(
        "EVM Tx [ChainId: {}, From: {}, To Contract: {}, Nonce: {}, GasLimit: {}, GasPrice: {} Gwei, Data: {}]",
        BSC_TESTNET_CHAIN_ID,
        from_address,
        BSC_TESTNET_USDT_CONTRACT,
        gas_info.nonce,
        gas_info.gas_limit,
        gas_info.gas_price_gwei,
        calldata_hex
    );

    Ok(BscTxPreview {
        draft_id,
        chain_id: BSC_TESTNET_CHAIN_ID,
        network_name: "BSC Testnet".into(),
        is_token_transfer: true,
        token_symbol: "USDT".into(),
        token_contract: Some(BSC_TESTNET_USDT_CONTRACT.into()),
        from_address,
        recipient_address: valid_recipient,
        amount_raw: amount_wei.to_str_radix(10),
        formatted_amount,
        nonce: gas_info.nonce,
        gas_limit: gas_info.gas_limit,
        gas_price_gwei: gas_info.gas_price_gwei,
        gas_price_wei: gas_info.gas_price_wei.to_string(),
        estimated_fee_bnb: gas_info.estimated_fee_bnb,
        calldata_hex,
        unsigned_tx_summary: unsigned_summary,
        signing_enabled: false,
        note: "BSC Testnet Unsigned Transaction Preview (P2C-1: Signing is not enabled)".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decimal_to_wei() {
        assert_eq!(
            parse_decimal_to_wei_18("1").unwrap(),
            BigUint::from(1_000_000_000_000_000_000u64)
        );
        assert_eq!(
            parse_decimal_to_wei_18("1.5").unwrap(),
            BigUint::from(1_500_000_000_000_000_000u64)
        );
        assert_eq!(
            parse_decimal_to_wei_18("0.0001").unwrap(),
            BigUint::from(100_000_000_000_000u64)
        );

        assert!(parse_decimal_to_wei_18("0").is_err());
        assert!(parse_decimal_to_wei_18("abc").is_err());
        assert!(parse_decimal_to_wei_18("1.1.1").is_err());
    }
}
