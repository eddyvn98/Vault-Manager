use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BroadcastNetwork {
    BitcoinSignet,
    BscTestnet,
}

impl BroadcastNetwork {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            BroadcastNetwork::BitcoinSignet => "bitcoin_signet",
            BroadcastNetwork::BscTestnet => "bsc_testnet",
        }
    }

    /// Hard security boundary: Only Signet and BSC Testnet are permitted in this build.
    #[allow(dead_code)]
    pub fn assert_testnet_only(network_str: &str) -> Result<Self, String> {
        match network_str {
            "bitcoin_signet" | "signet" => Ok(BroadcastNetwork::BitcoinSignet),
            "bsc_testnet" | "bsc-testnet" | "97" => Ok(BroadcastNetwork::BscTestnet),
            "bitcoin_mainnet" | "mainnet" | "bsc_mainnet" | "56" => {
                Err("SECURITY ERROR: Mainnet broadcast is strictly forbidden in this testnet-only build.".to_string())
            }
            other => Err(format!("Unsupported or unrecognized network: '{}'", other)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "details", rename_all = "camelCase")]
pub enum TxLifecycleStatus {
    DraftCreated {
        draft_id: String,
    },
    Signed {
        draft_id: String,
        payload_hash: String,
        raw_tx_hex: String,
    },
    Broadcasting {
        draft_id: String,
        attempt: u32,
    },
    Pending {
        tx_hash: String,
        first_seen_unix: u64,
    },
    Confirmed {
        tx_hash: String,
        block_height: u64,
        block_hash: String,
        confirmations: u64,
    },
    Failed {
        stage: String,
        error_message: String,
        permanent: bool,
    },
    Dropped {
        tx_hash: String,
        reason: String,
    },
    Replaced {
        old_tx_hash: String,
        new_tx_hash: String,
        reason: String,
    },
    ReorgDetected {
        tx_hash: String,
        previous_block_height: u64,
        previous_block_hash: String,
    },
}

impl TxLifecycleStatus {
    #[allow(dead_code)]
    pub fn is_terminal(&self) -> bool {
        match self {
            TxLifecycleStatus::Confirmed { confirmations, .. } => *confirmations >= 6,
            TxLifecycleStatus::Failed { permanent, .. } => *permanent,
            TxLifecycleStatus::Dropped { .. } => true,
            TxLifecycleStatus::Replaced { .. } => true,
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn can_transition_to(&self, next: &TxLifecycleStatus) -> bool {
        match (self, next) {
            // Draft -> Signed
            (TxLifecycleStatus::DraftCreated { draft_id: d1 }, TxLifecycleStatus::Signed { draft_id: d2, .. }) => {
                d1 == d2
            }
            // Signed -> Broadcasting
            (TxLifecycleStatus::Signed { draft_id: d1, .. }, TxLifecycleStatus::Broadcasting { draft_id: d2, .. }) => {
                d1 == d2
            }
            // Broadcasting -> Broadcasting (retry)
            (TxLifecycleStatus::Broadcasting { draft_id: d1, .. }, TxLifecycleStatus::Broadcasting { draft_id: d2, .. }) => {
                d1 == d2
            }
            // Broadcasting -> Pending (tx accepted in mempool)
            (TxLifecycleStatus::Broadcasting { .. }, TxLifecycleStatus::Pending { .. }) => true,
            // Broadcasting -> Failed
            (TxLifecycleStatus::Broadcasting { .. }, TxLifecycleStatus::Failed { .. }) => true,
            // Pending -> Confirmed
            (TxLifecycleStatus::Pending { tx_hash: h1, .. }, TxLifecycleStatus::Confirmed { tx_hash: h2, .. }) => {
                h1 == h2
            }
            // Pending -> Pending (e.g. still in mempool)
            (TxLifecycleStatus::Pending { tx_hash: h1, .. }, TxLifecycleStatus::Pending { tx_hash: h2, .. }) => {
                h1 == h2
            }
            // Pending -> Dropped (evicted from mempool)
            (TxLifecycleStatus::Pending { tx_hash: h1, .. }, TxLifecycleStatus::Dropped { tx_hash: h2, .. }) => {
                h1 == h2
            }
            // Pending -> Replaced (RBF or higher gas replacement mined)
            (TxLifecycleStatus::Pending { tx_hash: h1, .. }, TxLifecycleStatus::Replaced { old_tx_hash: h2, .. }) => {
                h1 == h2
            }
            // Pending -> Failed (e.g. EVM receipt reverted with 0x0 status)
            (TxLifecycleStatus::Pending { .. }, TxLifecycleStatus::Failed { .. }) => true,
            // Confirmed -> Confirmed (more confirmations)
            (
                TxLifecycleStatus::Confirmed { tx_hash: h1, block_height: bh1, .. },
                TxLifecycleStatus::Confirmed { tx_hash: h2, block_height: bh2, .. },
            ) => h1 == h2 && bh1 == bh2,
            // Confirmed -> ReorgDetected (block hash mismatch or unmined)
            (TxLifecycleStatus::Confirmed { tx_hash: h1, .. }, TxLifecycleStatus::ReorgDetected { tx_hash: h2, .. }) => {
                h1 == h2
            }
            // ReorgDetected -> Pending (tx back in mempool), Confirmed (re-mined), or Dropped
            (TxLifecycleStatus::ReorgDetected { tx_hash: h1, .. }, TxLifecycleStatus::Pending { tx_hash: h2, .. }) => {
                h1 == h2
            }
            (TxLifecycleStatus::ReorgDetected { tx_hash: h1, .. }, TxLifecycleStatus::Confirmed { tx_hash: h2, .. }) => {
                h1 == h2
            }
            (TxLifecycleStatus::ReorgDetected { tx_hash: h1, .. }, TxLifecycleStatus::Dropped { tx_hash: h2, .. }) => {
                h1 == h2
            }
            _ => false,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxLifecycleRecord {
    pub record_id: String,
    pub network: BroadcastNetwork,
    pub tx_hash: Option<String>,
    pub raw_tx_hex: Option<String>,
    pub payload_hash: Option<String>,
    pub expected_txid: Option<String>,
    pub expected_wtxid: Option<String>,
    pub expected_sender: Option<String>,
    pub expected_recipient: Option<String>,
    pub expected_amount_raw: Option<String>,
    pub status: TxLifecycleStatus,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
    pub retry_count: u32,
}

#[allow(dead_code)]
impl TxLifecycleRecord {
    pub fn new(draft_id: String, network: BroadcastNetwork, now_unix: u64) -> Self {
        Self {
            record_id: draft_id.clone(),
            network,
            tx_hash: None,
            raw_tx_hex: None,
            payload_hash: None,
            expected_txid: None,
            expected_wtxid: None,
            expected_sender: None,
            expected_recipient: None,
            expected_amount_raw: None,
            status: TxLifecycleStatus::DraftCreated { draft_id },
            created_at_unix: now_unix,
            updated_at_unix: now_unix,
            retry_count: 0,
        }
    }

    pub fn transition_to(&mut self, next_status: TxLifecycleStatus, now_unix: u64) -> Result<(), String> {
        if !self.status.can_transition_to(&next_status) {
            return Err(format!(
                "Illegal lifecycle transition from {:?} to {:?}",
                self.status, next_status
            ));
        }

        // Track critical payload and tx identifiers across transitions
        match &next_status {
            TxLifecycleStatus::Signed { payload_hash, raw_tx_hex, .. } => {
                self.payload_hash = Some(payload_hash.clone());
                self.raw_tx_hex = Some(raw_tx_hex.clone());
            }
            TxLifecycleStatus::Pending { tx_hash, .. } => {
                self.tx_hash = Some(tx_hash.clone());
            }
            TxLifecycleStatus::Confirmed { tx_hash, .. } => {
                self.tx_hash = Some(tx_hash.clone());
            }
            TxLifecycleStatus::Broadcasting { .. } => {
                self.retry_count += 1;
            }
            _ => {}
        }

        self.status = next_status;
        self.updated_at_unix = now_unix;
        Ok(())
    }
}
