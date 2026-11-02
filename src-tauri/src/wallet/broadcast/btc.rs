use crate::wallet::broadcast::types::BroadcastNetwork;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use tiny_keccak::{Hasher, Keccak};

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BtcBroadcastError {
    NetworkForbidden(String),
    DoubleSpend { message: String },
    MempoolConflict { message: String },
    FeeTooLow { message: String },
    DustOutput { message: String },
    AlreadyInBlock { txid: String },
    HttpError { status_code: u16, message: String },
    Timeout(String),
    NodeRejected(String),
}

impl std::fmt::Display for BtcBroadcastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetworkForbidden(m) => write!(f, "Network forbidden: {}", m),
            Self::DoubleSpend { message } => write!(f, "Double spend rejected: {}", message),
            Self::MempoolConflict { message } => write!(f, "Mempool conflict: {}", message),
            Self::FeeTooLow { message } => write!(f, "Fee too low: {}", message),
            Self::DustOutput { message } => write!(f, "Dust output rejected: {}", message),
            Self::AlreadyInBlock { txid } => write!(f, "Transaction already in block: {}", txid),
            Self::HttpError { status_code, message } => {
                write!(f, "HTTP error {}: {}", status_code, message)
            }
            Self::Timeout(m) => write!(f, "Request timeout: {}", m),
            Self::NodeRejected(m) => write!(f, "Node rejected: {}", m),
        }
    }
}

impl std::error::Error for BtcBroadcastError {}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BtcTxQueryStatus {
    NotFound,
    InMempool,
    Confirmed {
        block_height: u64,
        block_hash: String,
        confirmations: u64,
    },
    Replaced {
        replacing_txid: String,
    },
}

#[allow(dead_code)]
pub fn parse_esplora_error(status_code: u16, response_body: &str) -> BtcBroadcastError {
    let lower = response_body.to_lowercase();
    if lower.contains("bad-txns-inputs-spent") || lower.contains("txn-mempool-conflict") {
        BtcBroadcastError::DoubleSpend {
            message: response_body.to_string(),
        }
    } else if lower.contains("mempool min fee not met") || lower.contains("min relay fee not met") {
        BtcBroadcastError::FeeTooLow {
            message: response_body.to_string(),
        }
    } else if lower.contains("dust") {
        BtcBroadcastError::DustOutput {
            message: response_body.to_string(),
        }
    } else if lower.contains("transaction already in block chain") {
        BtcBroadcastError::AlreadyInBlock {
            txid: response_body.to_string(),
        }
    } else if status_code >= 500 {
        BtcBroadcastError::HttpError {
            status_code,
            message: response_body.to_string(),
        }
    } else {
        BtcBroadcastError::NodeRejected(response_body.to_string())
    }
}

#[allow(dead_code)]
pub trait BtcBroadcaster: Send + Sync {
    fn broadcast(&self, raw_tx_hex: &str, network: BroadcastNetwork) -> Result<String, BtcBroadcastError>;
    fn query_status(&self, txid: &str, network: BroadcastNetwork) -> Result<BtcTxQueryStatus, BtcBroadcastError>;
}

/// Mock broadcaster simulating network behaviors, failures, double-spends, and reorgs without real node access.
#[allow(dead_code)]
pub struct MockBtcBroadcaster {
    pub call_count: AtomicU32,
    pub next_broadcast_result: Mutex<Option<Result<String, BtcBroadcastError>>>,
    pub query_status_map: Mutex<std::collections::HashMap<String, BtcTxQueryStatus>>,
}

#[allow(dead_code)]
impl Default for MockBtcBroadcaster {
    fn default() -> Self {
        Self {
            call_count: AtomicU32::new(0),
            next_broadcast_result: Mutex::new(None),
            query_status_map: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[allow(dead_code)]
impl MockBtcBroadcaster {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_next_broadcast_result(&self, res: Result<String, BtcBroadcastError>) {
        *self.next_broadcast_result.lock().unwrap() = Some(res);
    }

    pub fn set_tx_status(&self, txid: &str, status: BtcTxQueryStatus) {
        self.query_status_map
            .lock()
            .unwrap()
            .insert(txid.to_string(), status);
    }
}

impl BtcBroadcaster for MockBtcBroadcaster {
    fn broadcast(&self, raw_tx_hex: &str, network: BroadcastNetwork) -> Result<String, BtcBroadcastError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        // Security check: Never permit mainnet
        if network != BroadcastNetwork::BitcoinSignet {
            return Err(BtcBroadcastError::NetworkForbidden(
                "Mock BTC broadcaster only allows Bitcoin Signet".to_string(),
            ));
        }

        if let Some(res) = self.next_broadcast_result.lock().unwrap().take() {
            return res;
        }

        // Default mock success: derive a deterministic mock txid from hash of input
        let mut hasher = Keccak::v256();
        let mut hash = [0u8; 32];
        hasher.update(raw_tx_hex.as_bytes());
        hasher.finalize(&mut hash);
        let txid = format!("mock_btc_txid_{}", hex::encode(&hash[0..8]));
        Ok(txid)
    }

    fn query_status(&self, txid: &str, network: BroadcastNetwork) -> Result<BtcTxQueryStatus, BtcBroadcastError> {
        if network != BroadcastNetwork::BitcoinSignet {
            return Err(BtcBroadcastError::NetworkForbidden(
                "Mock BTC broadcaster only allows Bitcoin Signet".to_string(),
            ));
        }

        let map = self.query_status_map.lock().unwrap();
        Ok(map.get(txid).cloned().unwrap_or(BtcTxQueryStatus::NotFound))
    }
}
