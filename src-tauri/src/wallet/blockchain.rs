use std::time::Duration;
use bitcoin::Network;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BtcBalance {
    pub confirmed_satoshis: u64,
    pub unconfirmed_satoshis: i64,
    pub total_satoshis: u64,
    pub formatted_btc: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BscBalance {
    pub bnb_raw_wei: String,
    pub formatted_bnb: String,
    pub usdt_raw: String,
    pub formatted_usdt: String,
    pub token_contract: &'static str,
}

#[derive(Deserialize)]
struct EsploraStats {
    funded_txo_sum: u64,
    spent_txo_sum: u64,
}

#[derive(Deserialize)]
struct EsploraAddressResponse {
    chain_stats: EsploraStats,
    mempool_stats: EsploraStats,
}

#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    method: &'a str,
    params: serde_json::Value,
    id: u32,
}

#[derive(Deserialize)]
struct JsonRpcResponse {
    result: Option<String>,
    #[allow(dead_code)]
    error: Option<serde_json::Value>,
}

pub fn format_satoshis(satoshis: u64) -> String {
    let btc = satoshis / 100_000_000;
    let rem = satoshis % 100_000_000;
    if rem == 0 {
        format!("{}.00000000", btc)
    } else {
        format!("{}.{:08}", btc, rem)
    }
}

pub fn format_wei_18(raw_hex_or_dec: &str) -> String {
    let clean = raw_hex_or_dec.trim_start_matches("0x");
    let hex_val = if clean.is_empty() { "0" } else { clean };

    // Parse hex into u128 if fits, or format integer
    let val = u128::from_str_radix(hex_val, 16).unwrap_or(0);
    let int_part = val / 1_000_000_000_000_000_000;
    let rem_part = val % 1_000_000_000_000_000_000;
    format!("{}.{:04}", int_part, rem_part / 100_000_000_000_000)
}

pub fn fetch_btc_balance(address: &str, network: Network) -> Result<BtcBalance, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let url = match network {
        Network::Bitcoin => format!("https://mempool.space/api/address/{}", address),
        _ => format!("https://mempool.space/signet/api/address/{}", address),
    };

    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("Network request to Esplora failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Esplora API returned status {}", resp.status()));
    }

    let data: EsploraAddressResponse = resp
        .json()
        .map_err(|e| format!("Failed to parse Esplora response: {}", e))?;

    let confirmed = data
        .chain_stats
        .funded_txo_sum
        .saturating_sub(data.chain_stats.spent_txo_sum);
    let mempool_funded = data.mempool_stats.funded_txo_sum as i64;
    let mempool_spent = data.mempool_stats.spent_txo_sum as i64;
    let unconfirmed = mempool_funded - mempool_spent;

    let total = if unconfirmed < 0 {
        confirmed.saturating_sub((-unconfirmed) as u64)
    } else {
        confirmed.saturating_add(unconfirmed as u64)
    };

    Ok(BtcBalance {
        confirmed_satoshis: confirmed,
        unconfirmed_satoshis: unconfirmed,
        total_satoshis: total,
        formatted_btc: format_satoshis(total),
    })
}

pub fn fetch_bsc_balance(address: &str, testnet: bool) -> Result<BscBalance, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let rpc_url = if testnet {
        "https://data-seed-prebsc-1-s1.binance.org:8545"
    } else {
        "https://bsc-dataseed.binance.org"
    };

    // 1. Fetch native BNB balance
    let bnb_req = JsonRpcRequest {
        jsonrpc: "2.0",
        method: "eth_getBalance",
        params: serde_json::json!([address, "latest"]),
        id: 1,
    };

    let bnb_resp = client
        .post(rpc_url)
        .json(&bnb_req)
        .send()
        .map_err(|e| format!("BSC RPC request failed: {}", e))?;

    let bnb_data: JsonRpcResponse = bnb_resp
        .json()
        .map_err(|e| format!("Invalid JSON-RPC response: {}", e))?;

    let raw_bnb_hex = bnb_data.result.unwrap_or_else(|| "0x0".to_string());
    let formatted_bnb = format_wei_18(&raw_bnb_hex);

    // 2. Fetch Binance-Peg BSC-USD (USDT BEP-20) token balance
    // balanceOf(address) selector: 0x70a08231
    let clean_addr = address.trim_start_matches("0x");
    let padded_addr = format!("{:0>64}", clean_addr);
    let call_data = format!("0x70a08231{}", padded_addr);

    let usdt_req = JsonRpcRequest {
        jsonrpc: "2.0",
        method: "eth_call",
        params: serde_json::json!([
            {
                "to": crate::wallet::bip::USDT_BEP20_CONTRACT,
                "data": call_data
            },
            "latest"
        ]),
        id: 2,
    };

    let usdt_resp = client
        .post(rpc_url)
        .json(&usdt_req)
        .send()
        .map_err(|e| format!("BSC USDT RPC call failed: {}", e))?;

    let usdt_data: JsonRpcResponse = usdt_resp
        .json()
        .map_err(|e| format!("Invalid JSON-RPC USDT response: {}", e))?;

    let raw_usdt_hex = usdt_data.result.unwrap_or_else(|| "0x0".to_string());
    let formatted_usdt = format_wei_18(&raw_usdt_hex);

    Ok(BscBalance {
        bnb_raw_wei: raw_bnb_hex,
        formatted_bnb,
        usdt_raw: raw_usdt_hex,
        formatted_usdt,
        token_contract: crate::wallet::bip::USDT_BEP20_CONTRACT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_satoshis_and_wei_no_float_drift() {
        assert_eq!(format_satoshis(100_000_000), "1.00000000");
        assert_eq!(format_satoshis(50_000_000), "0.50000000");
        assert_eq!(format_satoshis(1), "0.00000001");
        assert_eq!(format_satoshis(0), "0.00000000");

        // 1.5 ETH/BNB (1.5 * 10^18 = 1500000000000000000 = 0x14d1120d7b160000)
        assert_eq!(format_wei_18("0x14d1120d7b160000"), "1.5000");
        assert_eq!(format_wei_18("0x0"), "0.0000");
    }
}
