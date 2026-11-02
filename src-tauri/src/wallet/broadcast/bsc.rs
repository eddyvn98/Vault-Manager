use crate::wallet::broadcast::types::BroadcastNetwork;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tiny_keccak::{Hasher, Keccak};

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BscBroadcastError {
    NetworkForbidden(String),
    NonceTooLow { message: String },
    ReplacementUnderpriced { message: String },
    AlreadyKnown { tx_hash: String },
    InsufficientFunds { message: String },
    IntrinsicGasTooLow { message: String },
    ExceedsBlockGasLimit { message: String },
    ExecutionReverted { message: String },
    HttpError { status_code: u16, message: String },
    Timeout(String),
    RpcError { code: i64, message: String },
}

impl std::fmt::Display for BscBroadcastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetworkForbidden(m) => write!(f, "Network forbidden: {}", m),
            Self::NonceTooLow { message } => write!(f, "Nonce too low: {}", message),
            Self::ReplacementUnderpriced { message } => {
                write!(f, "Replacement underpriced: {}", message)
            }
            Self::AlreadyKnown { tx_hash } => write!(f, "Already known in mempool: {}", tx_hash),
            Self::InsufficientFunds { message } => {
                write!(f, "Insufficient funds for gas: {}", message)
            }
            Self::IntrinsicGasTooLow { message } => {
                write!(f, "Intrinsic gas too low: {}", message)
            }
            Self::ExceedsBlockGasLimit { message } => {
                write!(f, "Exceeds block gas limit: {}", message)
            }
            Self::ExecutionReverted { message } => write!(f, "Execution reverted: {}", message),
            Self::HttpError { status_code, message } => {
                write!(f, "HTTP error {}: {}", status_code, message)
            }
            Self::Timeout(m) => write!(f, "RPC timeout: {}", m),
            Self::RpcError { code, message } => write!(f, "RPC error {}: {}", code, message),
        }
    }
}

impl std::error::Error for BscBroadcastError {}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BscReceiptStatus {
    Pending,
    Success {
        block_number: u64,
        block_hash: String,
        gas_used: u64,
    },
    Reverted {
        block_number: u64,
        block_hash: String,
        reason: String,
    },
}

#[allow(dead_code)]
pub fn parse_bsc_rpc_error(code: i64, message: &str) -> BscBroadcastError {
    let lower = message.to_lowercase();
    if lower.contains("nonce too low") || lower.contains("nonce too small") {
        BscBroadcastError::NonceTooLow {
            message: message.to_string(),
        }
    } else if lower.contains("replacement transaction underpriced") || lower.contains("transaction underpriced") {
        BscBroadcastError::ReplacementUnderpriced {
            message: message.to_string(),
        }
    } else if lower.contains("already known") {
        BscBroadcastError::AlreadyKnown {
            tx_hash: message.to_string(),
        }
    } else if lower.contains("insufficient funds") {
        BscBroadcastError::InsufficientFunds {
            message: message.to_string(),
        }
    } else if lower.contains("intrinsic gas too low") {
        BscBroadcastError::IntrinsicGasTooLow {
            message: message.to_string(),
        }
    } else if lower.contains("exceeds block gas limit") {
        BscBroadcastError::ExceedsBlockGasLimit {
            message: message.to_string(),
        }
    } else if lower.contains("execution reverted") {
        BscBroadcastError::ExecutionReverted {
            message: message.to_string(),
        }
    } else {
        BscBroadcastError::RpcError {
            code,
            message: message.to_string(),
        }
    }
}

#[allow(dead_code)]
pub trait BscBroadcaster: Send + Sync {
    fn broadcast(&self, raw_signed_hex: &str, network: BroadcastNetwork) -> Result<String, BscBroadcastError>;
    fn query_receipt(&self, tx_hash: &str, network: BroadcastNetwork) -> Result<BscReceiptStatus, BscBroadcastError>;
}

/// Mock BSC broadcaster simulating RPC behaviors, nonce conflicts, gas failures, and receipt statuses without node access.
#[allow(dead_code)]
pub struct MockBscBroadcaster {
    pub call_count: AtomicU32,
    pub next_broadcast_result: Mutex<Option<Result<String, BscBroadcastError>>>,
    pub receipt_map: Mutex<std::collections::HashMap<String, BscReceiptStatus>>,
}

#[allow(dead_code)]
impl Default for MockBscBroadcaster {
    fn default() -> Self {
        Self {
            call_count: AtomicU32::new(0),
            next_broadcast_result: Mutex::new(None),
            receipt_map: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[allow(dead_code)]
impl MockBscBroadcaster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_next_broadcast_result(&self, res: Result<String, BscBroadcastError>) {
        *self.next_broadcast_result.lock().unwrap() = Some(res);
    }

    pub fn set_receipt(&self, tx_hash: &str, status: BscReceiptStatus) {
        self.receipt_map
            .lock()
            .unwrap()
            .insert(tx_hash.to_string(), status);
    }
}

impl BscBroadcaster for MockBscBroadcaster {
    fn broadcast(&self, raw_signed_hex: &str, network: BroadcastNetwork) -> Result<String, BscBroadcastError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        // Security check: Only allow BSC Testnet
        if network != BroadcastNetwork::BscTestnet {
            return Err(BscBroadcastError::NetworkForbidden(
                "Mock BSC broadcaster only allows BSC Testnet (Chain ID 97)".to_string(),
            ));
        }

        if let Some(res) = self.next_broadcast_result.lock().unwrap().take() {
            return res;
        }

        // Default mock success: deterministic tx hash (0x + keccak256)
        let clean = raw_signed_hex.trim_start_matches("0x");
        let bytes = hex::decode(clean).unwrap_or_else(|_| raw_signed_hex.as_bytes().to_vec());
        let mut hasher = Keccak::v256();
        let mut hash = [0u8; 32];
        hasher.update(&bytes);
        hasher.finalize(&mut hash);
        Ok(format!("0x{}", hex::encode(hash)))
    }

    fn query_receipt(&self, tx_hash: &str, network: BroadcastNetwork) -> Result<BscReceiptStatus, BscBroadcastError> {
        if network != BroadcastNetwork::BscTestnet {
            return Err(BscBroadcastError::NetworkForbidden(
                "Mock BSC broadcaster only allows BSC Testnet (Chain ID 97)".to_string(),
            ));
        }

        let map = self.receipt_map.lock().unwrap();
        Ok(map.get(tx_hash).cloned().unwrap_or(BscReceiptStatus::Pending))
    }
}

/// Minimal safe RLP list parser for legacy EIP-155 transactions.
/// Returns a vector of raw item bytes contained in the RLP list.
pub fn parse_rlp_list(payload: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if payload.is_empty() {
        return Err("Empty RLP payload".into());
    }

    let first = payload[0];
    let (list_payload, _consumed) = if (0xc0..=0xf7).contains(&first) {
        let len = (first - 0xc0) as usize;
        if payload.len() < 1 + len {
            return Err("RLP list payload truncated".into());
        }
        (&payload[1..1 + len], 1 + len)
    } else if first >= 0xf8 {
        let len_of_len = (first - 0xf7) as usize;
        if payload.len() < 1 + len_of_len {
            return Err("RLP list length header truncated".into());
        }
        let mut len = 0usize;
        for &b in &payload[1..1 + len_of_len] {
            len = (len << 8) | (b as usize);
        }
        if payload.len() < 1 + len_of_len + len {
            return Err("RLP list payload truncated".into());
        }
        (&payload[1 + len_of_len..1 + len_of_len + len], 1 + len_of_len + len)
    } else {
        return Err("Expected RLP list header (0xc0..0xff)".into());
    };

    let mut items = Vec::new();
    let mut cursor = 0;
    while cursor < list_payload.len() {
        let b = list_payload[cursor];
        if b <= 0x7f {
            items.push(vec![b]);
            cursor += 1;
        } else if (0x80..=0xb7).contains(&b) {
            let item_len = (b - 0x80) as usize;
            cursor += 1;
            if cursor + item_len > list_payload.len() {
                return Err("RLP string item truncated".into());
            }
            items.push(list_payload[cursor..cursor + item_len].to_vec());
            cursor += item_len;
        } else if (0xb8..=0xbf).contains(&b) {
            let len_of_len = (b - 0xb7) as usize;
            cursor += 1;
            if cursor + len_of_len > list_payload.len() {
                return Err("RLP string length truncated".into());
            }
            let mut item_len = 0usize;
            for &lb in &list_payload[cursor..cursor + len_of_len] {
                item_len = (item_len << 8) | (lb as usize);
            }
            cursor += len_of_len;
            if cursor + item_len > list_payload.len() {
                return Err("RLP string item truncated".into());
            }
            items.push(list_payload[cursor..cursor + item_len].to_vec());
            cursor += item_len;
        } else {
            return Err("Nested lists are not expected inside legacy EIP-155 transaction".into());
        }
    }

    Ok(items)
}

/// Real network adapter for BSC Testnet using JSON-RPC.
/// Strictly enforces testnet boundaries and feature-locking (G5A/G5B).
#[allow(dead_code)]
pub struct RpcBscBroadcaster {
    pub rpc_url: String,
    pub broadcast_enabled: bool,
    client: reqwest::blocking::Client,
}

#[allow(dead_code)]
impl RpcBscBroadcaster {
    pub fn new(broadcast_enabled: bool) -> Self {
        Self::with_endpoint("https://data-seed-prebsc-1-s1.binance.org:8545", broadcast_enabled)
    }

    pub fn with_endpoint(endpoint: &str, broadcast_enabled: bool) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());

        Self {
            rpc_url: endpoint.to_string(),
            broadcast_enabled,
            client,
        }
    }

    /// Dry-run verification (G5A): Decodes RLP, parses EIP-155 signature fields,
    /// verifies chain ID 97, recipient contract, zero value, and ERC-20 selector,
    /// and calculates Keccak-256 hash without sending eth_sendRawTransaction.
    pub fn dry_run_verify(&self, raw_tx_hex: &str, network: BroadcastNetwork) -> Result<String, BscBroadcastError> {
        if network != BroadcastNetwork::BscTestnet {
            return Err(BscBroadcastError::NetworkForbidden(
                format!("Network {:?} is strictly forbidden. Only BSC Testnet (Chain ID 97) is permitted.", network)
            ));
        }

        let clean_hex = raw_tx_hex.trim().trim_start_matches("0x").trim_start_matches("0X");
        let raw_bytes = hex::decode(clean_hex)
            .map_err(|e| BscBroadcastError::RpcError { code: -32602, message: format!("Invalid raw EVM tx hex: {}", e) })?;

        if raw_bytes.is_empty() {
            return Err(BscBroadcastError::RpcError { code: -32602, message: "Raw transaction bytes are empty".to_string() });
        }

        // Parse RLP items
        let items = parse_rlp_list(&raw_bytes)
            .map_err(|e| BscBroadcastError::RpcError { code: -32602, message: format!("Failed to parse EIP-155 RLP list: {}", e) })?;

        if items.len() != 9 {
            return Err(BscBroadcastError::RpcError {
                code: -32602,
                message: format!("Expected 9 RLP items for EIP-155 transaction, got {}", items.len()),
            });
        }

        // Item 6: v (must decode to chain_id = 97)
        let mut v = 0u64;
        for &b in &items[6] {
            v = (v << 8) | (b as u64);
        }
        if v < 35 {
            return Err(BscBroadcastError::NetworkForbidden(
                "Legacy transaction lacks EIP-155 replay protection (v < 35)".to_string(),
            ));
        }
        let chain_id = (v - 35) / 2;
        if chain_id != 97 {
            return Err(BscBroadcastError::NetworkForbidden(
                format!("EIP-155 chain ID mismatch in raw transaction: expected 97 (BSC Testnet), found {}", chain_id),
            ));
        }

        // Item 3: to address (must match testnet USDT contract)
        let to_hex = format!("0x{}", hex::encode(&items[3]));
        if to_hex.to_lowercase() != crate::wallet::blockchain::BSC_TESTNET_USDT_CONTRACT.to_lowercase() {
            return Err(BscBroadcastError::NetworkForbidden(
                format!("Unauthorized recipient contract: expected {}, got {}", crate::wallet::blockchain::BSC_TESTNET_USDT_CONTRACT, to_hex),
            ));
        }

        // Item 4: value (must be strictly 0 for token transfers)
        if !items[4].is_empty() && items[4].iter().any(|&b| b != 0) {
            return Err(BscBroadcastError::RpcError {
                code: -32602,
                message: "Transaction value must be 0 for token transfers".to_string(),
            });
        }

        // Item 5: data (must start with ERC20 transfer selector 0xa9059cbb)
        if items[5].len() < 4 || items[5][0..4] != [0xa9, 0x05, 0x9c, 0xbb] {
            return Err(BscBroadcastError::RpcError {
                code: -32602,
                message: "Transaction calldata does not match ERC-20 transfer selector (0xa9059cbb)".to_string(),
            });
        }

        let mut hasher = Keccak::v256();
        let mut hash = [0u8; 32];
        hasher.update(&raw_bytes);
        hasher.finalize(&mut hash);
        let computed_hash = format!("0x{}", hex::encode(hash));
        Ok(computed_hash)
    }

    /// Dry-run query: strictly parses nonce and balance via JSON-RPC without unwrap_or(0) defaults
    pub fn dry_run_query_account(&self, address: &str) -> Result<(u64, String), BscBroadcastError> {
        let nonce_val = self.call_rpc("eth_getTransactionCount", serde_json::json!([address, "latest"]))?;
        let clean_nonce = nonce_val.trim().trim_start_matches("0x").trim_start_matches("0X");
        if clean_nonce.is_empty() {
            return Err(BscBroadcastError::RpcError { code: -32603, message: "RPC returned empty nonce".to_string() });
        }
        let nonce = u64::from_str_radix(clean_nonce, 16)
            .map_err(|e| BscBroadcastError::RpcError { code: -32603, message: format!("Invalid nonce hex from RPC: {}", e) })?;

        let balance_hex = self.call_rpc("eth_getBalance", serde_json::json!([address, "latest"]))?;
        let clean_balance = balance_hex.trim();
        if clean_balance.is_empty() || clean_balance == "null" || !clean_balance.starts_with("0x") {
            return Err(BscBroadcastError::RpcError { code: -32603, message: format!("RPC returned invalid balance: {}", balance_hex) });
        }
        Ok((nonce, balance_hex))
    }

    /// Queries mempool and receipt status, distinguishing Pending in mempool from unknown/not-found
    pub fn query_mempool_or_receipt(&self, tx_hash: &str) -> Result<BscReceiptStatus, BscBroadcastError> {
        let formatted_hash = if tx_hash.starts_with("0x") {
            tx_hash.to_string()
        } else {
            format!("0x{}", tx_hash)
        };

        // 1. First check if a receipt already exists (mined block)
        let receipt_str = self.call_rpc("eth_getTransactionReceipt", serde_json::json!([formatted_hash]))?;
        if receipt_str != "null" && !receipt_str.trim().is_empty() {
            let receipt: serde_json::Value = serde_json::from_str(&receipt_str)
                .map_err(|e| BscBroadcastError::RpcError { code: -32700, message: format!("Cannot parse receipt JSON: {}", e) })?;

            let status_hex = receipt.get("status").and_then(|v| v.as_str()).unwrap_or("0x0");
            let block_number_hex = receipt.get("blockNumber").and_then(|v| v.as_str()).unwrap_or("0x0");
            let block_number = u64::from_str_radix(block_number_hex.trim_start_matches("0x"), 16).unwrap_or(0);
            let block_hash = receipt.get("blockHash").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let gas_used_hex = receipt.get("gasUsed").and_then(|v| v.as_str()).unwrap_or("0x0");
            let gas_used = u64::from_str_radix(gas_used_hex.trim_start_matches("0x"), 16).unwrap_or(0);

            if status_hex == "0x1" {
                return Ok(BscReceiptStatus::Success {
                    block_number,
                    block_hash,
                    gas_used,
                });
            } else {
                return Ok(BscReceiptStatus::Reverted {
                    block_number,
                    block_hash,
                    reason: "EVM execution reverted (status 0x0)".to_string(),
                });
            }
        }

        // 2. Receipt is null -> Query eth_getTransactionByHash to see if node has it in mempool
        let tx_str = self.call_rpc("eth_getTransactionByHash", serde_json::json!([formatted_hash]))?;
        if tx_str != "null" && !tx_str.trim().is_empty() {
            return Ok(BscReceiptStatus::Pending);
        }

        // 3. Not found in receipt AND not found in mempool -> Unknown!
        Err(BscBroadcastError::Timeout(format!(
            "Transaction '{}' was not found in mempool or block receipts. Status unknown.",
            tx_hash
        )))
    }

    fn call_rpc(&self, method: &str, params: serde_json::Value) -> Result<String, BscBroadcastError> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let resp = self.client.post(&self.rpc_url).json(&req).send()
            .map_err(|e| BscBroadcastError::Timeout(format!("RPC call {} timed out: {}", method, e)))?;

        let status_code = resp.status().as_u16();
        if !resp.status().is_success() {
            return Err(BscBroadcastError::HttpError { status_code, message: format!("HTTP status {}", status_code) });
        }

        let json: serde_json::Value = resp.json()
            .map_err(|e| BscBroadcastError::RpcError { code: -32700, message: format!("Failed to parse RPC JSON: {}", e) })?;

        if let Some(err) = json.get("error") {
            let code = err.get("code").and_then(|v| v.as_i64()).unwrap_or(-32000);
            let message = err.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown RPC error");
            return Err(parse_bsc_rpc_error(code, message));
        }

        let result = json.get("result")
            .ok_or_else(|| BscBroadcastError::RpcError { code: -32603, message: "Missing result in RPC response".to_string() })?;

        if result.is_string() {
            Ok(result.as_str().unwrap().to_string())
        } else {
            Ok(result.to_string())
        }
    }
}

impl BscBroadcaster for RpcBscBroadcaster {
    fn broadcast(&self, raw_tx_hex: &str, network: BroadcastNetwork) -> Result<String, BscBroadcastError> {
        let computed_hash = self.dry_run_verify(raw_tx_hex, network)?;

        // If broadcast is locked (feature-gated / default mode): return dry-run computed hash
        if !self.broadcast_enabled {
            return Ok(format!("dry_run_{}", computed_hash));
        }

        let formatted_hex = if raw_tx_hex.starts_with("0x") || raw_tx_hex.starts_with("0X") {
            raw_tx_hex.to_string()
        } else {
            format!("0x{}", raw_tx_hex)
        };

        // Live broadcast (G5B locked mode):
        let res = match self.call_rpc("eth_sendRawTransaction", serde_json::json!([formatted_hex])) {
            Ok(h) => h,
            Err(e) => {
                // If timeout or failure occurred, DO NOT immediately retry or duplicate:
                // Check if node actually has the transaction in mempool or block
                match self.query_mempool_or_receipt(&computed_hash) {
                    Ok(BscReceiptStatus::Success { .. } | BscReceiptStatus::Pending) => computed_hash.clone(),
                    _ => return Err(e),
                }
            }
        };

        if res.to_lowercase() != computed_hash.to_lowercase() {
            return Err(BscBroadcastError::RpcError {
                code: -32000,
                message: format!("RPC returned hash '{}' does not match locally calculated Keccak-256 hash '{}'", res, computed_hash),
            });
        }

        Ok(computed_hash)
    }

    fn query_receipt(&self, tx_hash: &str, network: BroadcastNetwork) -> Result<BscReceiptStatus, BscBroadcastError> {
        if network != BroadcastNetwork::BscTestnet {
            return Err(BscBroadcastError::NetworkForbidden(
                format!("Network {:?} is strictly forbidden. Only BSC Testnet (Chain ID 97) is permitted.", network)
            ));
        }

        let formatted_hash = if tx_hash.starts_with("0x") {
            tx_hash.to_string()
        } else {
            format!("0x{}", tx_hash)
        };

        let result_str = self.call_rpc("eth_getTransactionReceipt", serde_json::json!([formatted_hash]))?;
        if result_str == "null" || result_str.trim().is_empty() {
            return Ok(BscReceiptStatus::Pending);
        }

        let receipt: serde_json::Value = serde_json::from_str(&result_str)
            .map_err(|e| BscBroadcastError::RpcError { code: -32700, message: format!("Cannot parse receipt JSON: {}", e) })?;

        let status_hex = receipt.get("status").and_then(|v| v.as_str()).unwrap_or("0x0");
        let block_number_hex = receipt.get("blockNumber").and_then(|v| v.as_str()).unwrap_or("0x0");
        let block_number = u64::from_str_radix(block_number_hex.trim_start_matches("0x"), 16).unwrap_or(0);
        let block_hash = receipt.get("blockHash").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let gas_used_hex = receipt.get("gasUsed").and_then(|v| v.as_str()).unwrap_or("0x0");
        let gas_used = u64::from_str_radix(gas_used_hex.trim_start_matches("0x"), 16).unwrap_or(0);

        if status_hex == "0x1" {
            Ok(BscReceiptStatus::Success {
                block_number,
                block_hash,
                gas_used,
            })
        } else {
            Ok(BscReceiptStatus::Reverted {
                block_number,
                block_hash,
                reason: "EVM execution reverted (status 0x0)".to_string(),
            })
        }
    }
}
