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

    /// Dry-run verification (G5A): calculates Keccak-256 transaction hash locally,
    /// verifies network is BSC Testnet, without sending eth_sendRawTransaction.
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

        let mut hasher = Keccak::v256();
        let mut hash = [0u8; 32];
        hasher.update(&raw_bytes);
        hasher.finalize(&mut hash);
        let computed_hash = format!("0x{}", hex::encode(hash));
        Ok(computed_hash)
    }

    /// Dry-run query: queries nonce and balance via JSON-RPC
    pub fn dry_run_query_account(&self, address: &str) -> Result<(u64, String), BscBroadcastError> {
        let nonce_val = self.call_rpc("eth_getTransactionCount", serde_json::json!([address, "latest"]))?;
        let nonce = u64::from_str_radix(nonce_val.trim_start_matches("0x"), 16).unwrap_or(0);
        let balance_hex = self.call_rpc("eth_getBalance", serde_json::json!([address, "latest"]))?;
        Ok((nonce, balance_hex))
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
                // If timeout occurred, DO NOT immediately retry or duplicate:
                // First query receipt/mempool for the expected transaction hash!
                if let Ok(BscReceiptStatus::Success { .. } | BscReceiptStatus::Pending) = self.query_receipt(&computed_hash, network) {
                    return Ok(computed_hash);
                }
                return Err(e);
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
