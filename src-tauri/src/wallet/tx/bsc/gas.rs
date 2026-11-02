use num_bigint::BigUint;
use num_traits::Num;
use reqwest::Client;
use crate::wallet::blockchain;

pub struct GasNonceInfo {
    pub nonce: u64,
    pub gas_limit: u64,
    #[allow(dead_code)]
    pub gas_price_wei: BigUint,
    pub gas_price_gwei: String,
    pub estimated_fee_bnb: String,
}

pub async fn query_bsc_gas_and_nonce(
    client: &Client,
    rpc_url: &str,
    from_address: &str,
    to_contract: &str,
    calldata: &str,
) -> Result<GasNonceInfo, String> {
    // 1. Query pending nonce
    let nonce_hex = blockchain::call_json_rpc(
        client,
        rpc_url,
        "eth_getTransactionCount",
        serde_json::json!([from_address, "pending"]),
        10,
    )
    .await?;

    let clean_nonce = nonce_hex.trim().trim_start_matches("0x");
    let nonce = if clean_nonce.is_empty() {
        0
    } else {
        u64::from_str_radix(clean_nonce, 16)
            .map_err(|e| format!("Invalid nonce format from RPC ({}): {}", nonce_hex, e))?
    };

    // 2. Query gas price
    let gas_price_hex = blockchain::call_json_rpc(
        client,
        rpc_url,
        "eth_gasPrice",
        serde_json::json!([]),
        11,
    )
    .await?;

    let clean_gp = gas_price_hex.trim().trim_start_matches("0x");
    let gas_price_wei = BigUint::from_str_radix(clean_gp, 16)
        .map_err(|e| format!("Invalid gasPrice format from RPC: {}", e))?;

    // Gas price in Gwei (1 Gwei = 10^9 wei)
    let gwei_divisor = BigUint::from(1_000_000_000u64);
    let gwei_int = &gas_price_wei / &gwei_divisor;
    let gwei_rem = &gas_price_wei % &gwei_divisor;
    let gas_price_gwei = format!("{}.{:02}", gwei_int, (gwei_rem / BigUint::from(10_000_000u64)));

    // 3. Estimate gas via eth_estimateGas (propagate error if execution would revert)
    let estimate_hex = blockchain::call_json_rpc(
        client,
        rpc_url,
        "eth_estimateGas",
        serde_json::json!([{
            "from": from_address,
            "to": to_contract,
            "data": calldata
        }]),
        12,
    )
    .await
    .map_err(|e| format!("Gas estimation failed (transaction may revert, e.g. insufficient token balance): {}", e))?;

    let clean_limit = estimate_hex.trim().trim_start_matches("0x");
    let parsed_gas = u64::from_str_radix(clean_limit, 16)
        .map_err(|e| format!("Invalid estimateGas format from RPC ({}): {}", estimate_hex, e))?;
    // Add 20% safety margin for gas limit
    let gas_limit = (parsed_gas * 120) / 100;

    // 4. Calculate total fee in BNB: gas_limit * gas_price_wei
    let total_fee_wei = BigUint::from(gas_limit) * &gas_price_wei;
    let total_fee_hex = format!("0x{:x}", total_fee_wei);
    let estimated_fee_bnb = blockchain::format_wei_18(&total_fee_hex)?;

    // Safety cap: BSC transaction fee should not exceed 0.05 BNB (50_000_000_000_000_000 wei)
    let max_bsc_fee_wei = BigUint::from(50_000_000_000_000_000u64);
    if total_fee_wei > max_bsc_fee_wei {
        return Err(format!(
            "Estimated BSC fee ({} BNB) exceeds maximum safety limit of 0.05 BNB",
            estimated_fee_bnb
        ));
    }

    Ok(GasNonceInfo {
        nonce,
        gas_limit,
        gas_price_wei,
        gas_price_gwei,
        estimated_fee_bnb,
    })
}
