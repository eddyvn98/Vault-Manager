use std::time::Duration;
use bitcoin::Network;
use num_bigint::BigUint;
use num_traits::Num;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub const BSC_MAINNET_USDT_CONTRACT: &str = "0x55d398326f99059fF775485246999027B3197955";
pub const BSC_TESTNET_USDT_CONTRACT: &str = "0x337610d27c682E347C9cD60BD4b3b107C9d34dDd";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BtcBalance {
    pub address: String,
    pub derivation_path: String,
    pub is_first_receive_address: bool,
    pub confirmed_satoshis: u64,
    pub unconfirmed_satoshis: i64,
    pub total_satoshis: u64,
    pub formatted_btc: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TokenBalance {
    pub symbol: String,
    pub name: String,
    pub contract_address: Option<String>,
    pub raw_hex: String,
    pub formatted: String,
    pub decimals: u8,
    pub available: bool,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BscBalance {
    pub address: String,
    pub derivation_path: String,
    pub chain_id: u64,
    pub network_name: String,
    pub bnb_raw_wei: String,
    pub formatted_bnb: String,
    pub usdt: TokenBalance,
}

#[derive(Deserialize, Debug)]
pub struct EsploraStats {
    pub funded_txo_sum: u64,
    pub spent_txo_sum: u64,
}

#[derive(Deserialize, Debug)]
pub struct EsploraAddressResponse {
    pub chain_stats: EsploraStats,
    pub mempool_stats: EsploraStats,
}

#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    method: &'a str,
    params: serde_json::Value,
    id: u32,
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub id: u32,
    pub result: Option<String>,
    pub error: Option<JsonRpcError>,
}

pub fn format_satoshis(satoshis: u64) -> String {
    let btc = satoshis / 100_000_000;
    let rem = satoshis % 100_000_000;
    format!("{}.{:08}", btc, rem)
}

pub fn format_wei_18(raw_hex_or_dec: &str) -> Result<String, String> {
    let clean = raw_hex_or_dec.trim().trim_start_matches("0x");
    let clean = if clean.is_empty() { "0" } else { clean };

    let value = BigUint::from_str_radix(clean, 16)
        .map_err(|e| format!("Invalid hex value for uint256 wei: {}", e))?;

    let divisor = BigUint::from(10u64).pow(18);
    let int_part = &value / &divisor;
    let rem_part = &value % &divisor;

    let rem_str = format!("{:0>18}", rem_part.to_str_radix(10));
    let dec6 = &rem_str[..6];
    let trimmed = dec6.trim_end_matches('0');
    let final_decimals = if trimmed.is_empty() {
        "0000".to_string()
    } else if trimmed.len() < 4 {
        format!("{:0<4}", trimmed)
    } else {
        trimmed.to_string()
    };

    Ok(format!("{}.{}", int_part.to_str_radix(10), final_decimals))
}

pub fn calculate_esplora_balances(
    chain: &EsploraStats,
    mempool: &EsploraStats,
) -> Result<(u64, i64, u64), String> {
    let funded = chain.funded_txo_sum as i128;
    let spent = chain.spent_txo_sum as i128;
    if spent > funded {
        return Err(format!(
            "Inconsistent Esplora data: spent ({}) > funded ({})",
            spent, funded
        ));
    }
    let confirmed = (funded - spent) as u64;

    let mempool_funded = mempool.funded_txo_sum as i128;
    let mempool_spent = mempool.spent_txo_sum as i128;
    let unconfirmed = (mempool_funded - mempool_spent) as i64;

    let total_i128 = (confirmed as i128) + (unconfirmed as i128);
    let total = if total_i128 < 0 { 0 } else { total_i128 as u64 };

    Ok((confirmed, unconfirmed, total))
}

pub async fn fetch_btc_balance(address: &str, network: Network) -> Result<BtcBalance, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let (url, derivation_path) = match network {
        Network::Bitcoin => (
            format!("https://mempool.space/api/address/{}", address),
            "m/84'/0'/0'/0/0".to_string(),
        ),
        _ => (
            format!("https://mempool.space/signet/api/address/{}", address),
            "m/84'/1'/0'/0/0".to_string(),
        ),
    };

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Network request to Esplora failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Esplora API returned HTTP {}", resp.status()));
    }

    let data: EsploraAddressResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Esplora JSON response: {}", e))?;

    let (confirmed, unconfirmed, total) =
        calculate_esplora_balances(&data.chain_stats, &data.mempool_stats)?;

    Ok(BtcBalance {
        address: address.to_string(),
        derivation_path,
        is_first_receive_address: true,
        confirmed_satoshis: confirmed,
        unconfirmed_satoshis: unconfirmed,
        total_satoshis: total,
        formatted_btc: format_satoshis(total),
    })
}

pub async fn call_json_rpc(
    client: &Client,
    rpc_url: &str,
    method: &str,
    params: serde_json::Value,
    req_id: u32,
) -> Result<String, String> {
    let req = JsonRpcRequest {
        jsonrpc: "2.0",
        method,
        params,
        id: req_id,
    };

    let resp = client
        .post(rpc_url)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("RPC request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("RPC HTTP status error: {}", resp.status()));
    }

    let rpc_data: JsonRpcResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid JSON-RPC format: {}", e))?;

    if let Some(err) = rpc_data.error {
        return Err(format!("JSON-RPC error {}: {}", err.code, err.message));
    }

    if rpc_data.id != req_id {
        return Err(format!(
            "JSON-RPC ID mismatch: expected {}, got {}",
            req_id, rpc_data.id
        ));
    }

    rpc_data
        .result
        .ok_or_else(|| "JSON-RPC response missing both result and error".to_string())
}

pub async fn fetch_bsc_balance(address: &str, testnet: bool) -> Result<BscBalance, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let (rpc_url, expected_chain_id, network_name) = if testnet {
        (
            "https://data-seed-prebsc-1-s1.binance.org:8545",
            97u64,
            "BSC Testnet".to_string(),
        )
    } else {
        (
            "https://bsc-dataseed.binance.org",
            56u64,
            "BSC Mainnet".to_string(),
        )
    };

    // 1. Verify eth_chainId to guarantee we are talking to the expected blockchain network
    let chain_id_hex = call_json_rpc(&client, rpc_url, "eth_chainId", serde_json::json!([]), 1).await?;
    let clean_chain_id = chain_id_hex.trim().trim_start_matches("0x");
    let actual_chain_id = u64::from_str_radix(clean_chain_id, 16)
        .map_err(|e| format!("Invalid chainId returned by RPC ({}): {}", chain_id_hex, e))?;

    if actual_chain_id != expected_chain_id {
        return Err(format!(
            "Chain ID mismatch: Node returned {} (0x{:x}), but expected {} for {}",
            actual_chain_id, actual_chain_id, expected_chain_id, network_name
        ));
    }

    // 2. Fetch native BNB balance (eth_getBalance)
    let raw_bnb_hex = call_json_rpc(
        &client,
        rpc_url,
        "eth_getBalance",
        serde_json::json!([address, "latest"]),
        2,
    )
    .await?;

    let formatted_bnb = format_wei_18(&raw_bnb_hex)?;

    // 3. Fetch USDT Token Balance
    let token_contract = if testnet {
        BSC_TESTNET_USDT_CONTRACT
    } else {
        BSC_MAINNET_USDT_CONTRACT
    };

    let clean_addr = address.trim_start_matches("0x");
    let padded_addr = format!("{:0>64}", clean_addr);
    let call_data = format!("0x70a08231{}", padded_addr);

    let raw_usdt_hex = call_json_rpc(
        &client,
        rpc_url,
        "eth_call",
        serde_json::json!([
            {
                "to": token_contract,
                "data": call_data
            },
            "latest"
        ]),
        3,
    )
    .await?;

    let formatted_usdt = format_wei_18(&raw_usdt_hex)?;

    let usdt_token = TokenBalance {
        symbol: "USDT".to_string(),
        name: if testnet {
            "Binance-Peg BSC-USD (Testnet Mock)".to_string()
        } else {
            "Binance-Peg BSC-USD".to_string()
        },
        contract_address: Some(token_contract.to_string()),
        raw_hex: raw_usdt_hex,
        formatted: formatted_usdt,
        decimals: 18,
        available: true,
        note: if testnet {
            Some("BSC Testnet Mock USDT (Faucet/Test only)".to_string())
        } else {
            None
        },
    };

    Ok(BscBalance {
        address: address.to_string(),
        derivation_path: "m/44'/60'/0'/0/0".to_string(),
        chain_id: actual_chain_id,
        network_name,
        bnb_raw_wei: raw_bnb_hex,
        formatted_bnb,
        usdt: usdt_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_satoshis_test() {
        assert_eq!(format_satoshis(100_000_000), "1.00000000");
        assert_eq!(format_satoshis(50_000_000), "0.50000000");
        assert_eq!(format_satoshis(1), "0.00000001");
        assert_eq!(format_satoshis(0), "0.00000000");
    }

    #[test]
    fn format_wei_18_uint256_and_rejections() {
        // 1.5 BNB (0x14d1120d7b160000 = 1500000000000000000 wei)
        assert_eq!(format_wei_18("0x14d1120d7b160000").unwrap(), "1.5000");
        assert_eq!(format_wei_18("0x0").unwrap(), "0.0000");

        // Small balance: 0.000123 BNB
        // 0.000123 * 10^18 = 123000000000000 = 0x6fe25b018c00
        assert_eq!(format_wei_18("0x6fe25b018c00").unwrap(), "0.000123");

        // Massive Uint256 > u128::MAX (e.g. 2^130 wei)
        // 2^130 in hex = 0x400000000000000000000000000000000
        let massive_hex = "0x400000000000000000000000000000000";
        let formatted = format_wei_18(massive_hex).unwrap();
        assert!(formatted.starts_with("1361129467683753853853"), "Uint256 > u128::MAX parsed accurately");

        // Invalid hex MUST return Err, NEVER silence to 0!
        assert!(format_wei_18("not_a_hex").is_err());
        assert!(format_wei_18("0xGG").is_err());
    }

    #[test]
    fn esplora_stats_calculation_and_mempool_negative() {
        // Normal confirmed balance
        let chain = EsploraStats { funded_txo_sum: 1_000_000, spent_txo_sum: 400_000 };
        let mempool = EsploraStats { funded_txo_sum: 50_000, spent_txo_sum: 0 };
        let (confirmed, unconfirmed, total) = calculate_esplora_balances(&chain, &mempool).unwrap();
        assert_eq!(confirmed, 600_000);
        assert_eq!(unconfirmed, 50_000);
        assert_eq!(total, 650_000);

        // Mempool negative pending (unconfirmed spend)
        let mempool_spend = EsploraStats { funded_txo_sum: 0, spent_txo_sum: 100_000 };
        let (conf2, unconf2, total2) = calculate_esplora_balances(&chain, &mempool_spend).unwrap();
        assert_eq!(conf2, 600_000);
        assert_eq!(unconf2, -100_000);
        assert_eq!(total2, 500_000);

        // Inconsistent chain stats (spent > funded) MUST error
        let bad_chain = EsploraStats { funded_txo_sum: 100, spent_txo_sum: 500 };
        assert!(calculate_esplora_balances(&bad_chain, &mempool).is_err());
    }

    #[test]
    fn json_rpc_error_handling_simulation() {
        let error_json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32000,
                "message": "Node unavailable"
            }
        }"#;

        let parsed: JsonRpcResponse = serde_json::from_str(error_json).unwrap();
        assert!(parsed.error.is_some());
        assert_eq!(parsed.error.as_ref().unwrap().code, -32000);
        assert!(parsed.result.is_none());
    }
}
