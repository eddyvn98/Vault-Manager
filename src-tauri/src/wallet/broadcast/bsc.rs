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
