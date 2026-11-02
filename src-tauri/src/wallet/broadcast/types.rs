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
#[serde(tag = "status", content = "details", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TxLifecycleStatus {
    DraftCreated {
        #[serde(alias = "draft_id")]
        draft_id: String,
    },
    Signed {
        #[serde(alias = "draft_id")]
        draft_id: String,
        #[serde(alias = "payload_hash")]
        payload_hash: String,
        #[serde(alias = "raw_tx_hex")]
        raw_tx_hex: String,
    },
    Broadcasting {
        #[serde(alias = "draft_id")]
        draft_id: String,
        attempt: u32,
    },
    Pending {
        #[serde(alias = "tx_hash")]
        tx_hash: String,
        #[serde(alias = "first_seen_unix")]
        first_seen_unix: u64,
    },
    Confirmed {
        #[serde(alias = "tx_hash")]
        tx_hash: String,
        #[serde(alias = "block_height")]
        block_height: u64,
        #[serde(alias = "block_hash")]
        block_hash: String,
        confirmations: u64,
    },
    Failed {
        stage: String,
        #[serde(alias = "error_message")]
        error_message: String,
        permanent: bool,
    },
    Dropped {
        #[serde(alias = "tx_hash")]
        tx_hash: String,
        reason: String,
    },
    Replaced {
        #[serde(alias = "old_tx_hash")]
        old_tx_hash: String,
        #[serde(alias = "new_tx_hash")]
        new_tx_hash: String,
        reason: String,
    },
    ReorgDetected {
        #[serde(alias = "tx_hash")]
        tx_hash: String,
        #[serde(alias = "previous_block_height")]
        previous_block_height: u64,
        #[serde(alias = "previous_block_hash")]
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
            // Broadcasting -> Confirmed (polled after crash/restart and found already mined)
            (TxLifecycleStatus::Broadcasting { .. }, TxLifecycleStatus::Confirmed { .. }) => true,
            // Broadcasting -> Dropped
            (TxLifecycleStatus::Broadcasting { .. }, TxLifecycleStatus::Dropped { .. }) => true,
            // Broadcasting -> Failed
            (TxLifecycleStatus::Broadcasting { .. }, TxLifecycleStatus::Failed { .. }) => true,
            // Failed (transient / non-permanent) -> Broadcasting (retry/resume after crash or timeout)
            (TxLifecycleStatus::Failed { permanent: false, .. }, TxLifecycleStatus::Broadcasting { .. }) => true,
            // Failed (transient / non-permanent) -> Pending (discovered tx reached mempool)
            (TxLifecycleStatus::Failed { permanent: false, .. }, TxLifecycleStatus::Pending { .. }) => true,
            // Failed (transient / non-permanent) -> Confirmed (discovered tx mined)
            (TxLifecycleStatus::Failed { permanent: false, .. }, TxLifecycleStatus::Confirmed { .. }) => true,
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedPrevout {
    pub txid: String,
    pub vout: u32,
    pub value_satoshis: u64,
    pub script_pubkey_hex: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxLifecycleRecord {
    #[serde(alias = "record_id")]
    pub record_id: String,
    pub network: BroadcastNetwork,
    #[serde(alias = "tx_hash")]
    pub tx_hash: Option<String>,
    #[serde(alias = "expected_tx_hash")]
    pub expected_tx_hash: Option<String>,
    #[serde(alias = "raw_tx_hex")]
    pub raw_tx_hex: Option<String>,
    #[serde(alias = "payload_hash")]
    pub payload_hash: Option<String>,
    #[serde(alias = "expected_txid")]
    pub expected_txid: Option<String>,
    #[serde(alias = "expected_wtxid")]
    pub expected_wtxid: Option<String>,
    #[serde(alias = "approved_prevouts")]
    pub approved_prevouts: Option<Vec<ApprovedPrevout>>,
    #[serde(alias = "expected_sender")]
    pub expected_sender: Option<String>,
    #[serde(alias = "expected_recipient")]
    pub expected_recipient: Option<String>,
    #[serde(alias = "expected_amount_raw")]
    pub expected_amount_raw: Option<String>,
    #[serde(alias = "expected_nonce")]
    pub expected_nonce: Option<u64>,
    #[serde(alias = "expected_gas_limit")]
    pub expected_gas_limit: Option<u64>,
    #[serde(alias = "expected_gas_price")]
    pub expected_gas_price: Option<String>,
    pub status: TxLifecycleStatus,
    #[serde(alias = "created_at_unix")]
    pub created_at_unix: u64,
    #[serde(alias = "updated_at_unix")]
    pub updated_at_unix: u64,
    #[serde(alias = "retry_count")]
    pub retry_count: u32,
}

#[allow(dead_code)]
impl TxLifecycleRecord {
    pub fn new(draft_id: String, network: BroadcastNetwork, now_unix: u64) -> Self {
        Self {
            record_id: draft_id.clone(),
            network,
            tx_hash: None,
            expected_tx_hash: None,
            raw_tx_hex: None,
            payload_hash: None,
            expected_txid: None,
            expected_wtxid: None,
            approved_prevouts: None,
            expected_sender: None,
            expected_recipient: None,
            expected_amount_raw: None,
            expected_nonce: None,
            expected_gas_limit: None,
            expected_gas_price: None,
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
