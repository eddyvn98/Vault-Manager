use crate::wallet::broadcast::{
    bsc::{BscBroadcastError, BscBroadcaster, BscReceiptStatus},
    btc::{BtcBroadcastError, BtcBroadcaster, BtcTxQueryStatus},
    types::{BroadcastNetwork, TxLifecycleRecord, TxLifecycleStatus},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
struct LifecycleSnapshot {
    records: Vec<TxLifecycleRecord>,
    confirmed_block_hashes: HashMap<String, (u64, String)>,
}

#[allow(dead_code)]
pub struct TxLifecycleCoordinator {
    records: HashMap<String, TxLifecycleRecord>,
    confirmed_block_hashes: HashMap<String, (u64, String)>, // tx_hash -> (block_height, block_hash)
}

#[allow(dead_code)]
impl Default for TxLifecycleCoordinator {
    fn default() -> Self {
        Self {
            records: HashMap::new(),
            confirmed_block_hashes: HashMap::new(),
        }
    }
}

#[allow(dead_code)]
impl TxLifecycleCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_record(&self, id: &str) -> Option<&TxLifecycleRecord> {
        self.records.get(id)
    }

    #[allow(dead_code)]
    pub fn export_records(&self) -> Vec<TxLifecycleRecord> {
        self.records.values().cloned().collect()
    }

    #[allow(dead_code)]
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        let snapshot = LifecycleSnapshot {
            records: self.records.values().cloned().collect(),
            confirmed_block_hashes: self.confirmed_block_hashes.clone(),
        };
        let json_bytes = serde_json::to_vec_pretty(&snapshot)
            .map_err(|e| format!("Failed to serialize lifecycle records: {}", e))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create parent directory: {}", e))?;
        }

        let temp_path = path.with_extension(format!("tmp.{}", rand::random::<u32>()));
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&temp_path)
                .map_err(|e| format!("Cannot create temporary lifecycle file: {}", e))?;
            file.write_all(&json_bytes)
                .map_err(|e| format!("Cannot write temporary lifecycle file: {}", e))?;
            file.sync_all()
                .map_err(|e| format!("Cannot sync lifecycle file: {}", e))?;
        }

        std::fs::rename(&temp_path, path)
            .map_err(|e| format!("Cannot atomically save lifecycle file: {}", e))?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = std::fs::read(path)
            .map_err(|e| format!("Cannot read lifecycle file: {}", e))?;
        let snapshot: LifecycleSnapshot = serde_json::from_slice(&data)
            .map_err(|e| format!("Cannot parse lifecycle file: {}", e))?;

        let mut coordinator = Self::new();
        for r in snapshot.records {
            coordinator.records.insert(r.record_id.clone(), r);
        }
        coordinator.confirmed_block_hashes = snapshot.confirmed_block_hashes;
        Ok(coordinator)
    }

    pub fn register_draft(
        &mut self,
        draft_id: &str,
        network: BroadcastNetwork,
        now_unix: u64,
    ) -> Result<(), String> {
        if self.records.contains_key(draft_id) {
            return Err(format!("Record with draft_id '{}' already exists", draft_id));
        }
        let record = TxLifecycleRecord::new(draft_id.to_string(), network, now_unix);
        self.records.insert(draft_id.to_string(), record);
        Ok(())
    }

    pub fn record_signed(
        &mut self,
        draft_id: &str,
        payload_hash: &str,
        raw_tx_hex: &str,
        now_unix: u64,
    ) -> Result<(), String> {
        let record = self
            .records
            .get_mut(draft_id)
            .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;

        let signed_status = TxLifecycleStatus::Signed {
            draft_id: draft_id.to_string(),
            payload_hash: payload_hash.to_string(),
            raw_tx_hex: raw_tx_hex.to_string(),
        };

        record.transition_to(signed_status, now_unix)
    }

    pub fn execute_broadcast_btc(
        &mut self,
        draft_id: &str,
        broadcaster: &dyn BtcBroadcaster,
        max_retries: u32,
        now_unix: u64,
    ) -> Result<String, String> {
        let record = self
            .records
            .get_mut(draft_id)
            .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;

        if record.network != BroadcastNetwork::BitcoinSignet {
            return Err("BTC broadcast requested for non-Bitcoin network".to_string());
        }

        let raw_hex = match &record.status {
            TxLifecycleStatus::Signed { raw_tx_hex, .. } => raw_tx_hex.clone(),
            TxLifecycleStatus::Broadcasting { .. } => {
                record.raw_tx_hex.clone().ok_or_else(|| "Missing raw_tx_hex in record".to_string())?
            }
            other => {
                return Err(format!(
                    "Cannot broadcast transaction in state '{:?}', must be Signed or Broadcasting",
                    other
                ))
            }
        };

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let broadcasting_status = TxLifecycleStatus::Broadcasting {
                draft_id: draft_id.to_string(),
                attempt,
            };
            record.transition_to(broadcasting_status, now_unix)?;

            match broadcaster.broadcast(&raw_hex, record.network) {
                Ok(txid) => {
                    let pending_status = TxLifecycleStatus::Pending {
                        tx_hash: txid.clone(),
                        first_seen_unix: now_unix,
                    };
                    record.transition_to(pending_status, now_unix)?;
                    return Ok(txid);
                }
                Err(err) => {
                    let is_transient = matches!(
                        err,
                        BtcBroadcastError::Timeout(_) | BtcBroadcastError::HttpError { status_code: 500..=599, .. }
                    );

                    if is_transient && attempt < max_retries {
                        // Will retry in next loop iteration
                        continue;
                    }

                    let permanent = !is_transient;
                    let failed_status = TxLifecycleStatus::Failed {
                        stage: "broadcast".to_string(),
                        error_message: err.to_string(),
                        permanent,
                    };
                    record.transition_to(failed_status, now_unix)?;
                    return Err(format!("BTC broadcast failed: {}", err));
                }
            }
        }
    }

    pub fn execute_broadcast_bsc(
        &mut self,
        draft_id: &str,
        broadcaster: &dyn BscBroadcaster,
        max_retries: u32,
        now_unix: u64,
    ) -> Result<String, String> {
        let record = self
            .records
            .get_mut(draft_id)
            .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;

        if record.network != BroadcastNetwork::BscTestnet {
            return Err("BSC broadcast requested for non-BSC network".to_string());
        }

        let raw_hex = match &record.status {
            TxLifecycleStatus::Signed { raw_tx_hex, .. } => raw_tx_hex.clone(),
            TxLifecycleStatus::Broadcasting { .. } => {
                record.raw_tx_hex.clone().ok_or_else(|| "Missing raw_tx_hex in record".to_string())?
            }
            other => {
                return Err(format!(
                    "Cannot broadcast transaction in state '{:?}', must be Signed or Broadcasting",
                    other
                ))
            }
        };

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let broadcasting_status = TxLifecycleStatus::Broadcasting {
                draft_id: draft_id.to_string(),
                attempt,
            };
            record.transition_to(broadcasting_status, now_unix)?;

            match broadcaster.broadcast(&raw_hex, record.network) {
                Ok(tx_hash) => {
                    let pending_status = TxLifecycleStatus::Pending {
                        tx_hash: tx_hash.clone(),
                        first_seen_unix: now_unix,
                    };
                    record.transition_to(pending_status, now_unix)?;
                    return Ok(tx_hash);
                }
                Err(err) => {
                    let is_transient = matches!(
                        err,
                        BscBroadcastError::Timeout(_) | BscBroadcastError::HttpError { status_code: 500..=599, .. }
                    );

                    if is_transient && attempt < max_retries {
                        continue;
                    }

                    let permanent = !is_transient;
                    let failed_status = TxLifecycleStatus::Failed {
                        stage: "broadcast".to_string(),
                        error_message: err.to_string(),
                        permanent,
                    };
                    record.transition_to(failed_status, now_unix)?;
                    return Err(format!("BSC broadcast failed: {}", err));
                }
            }
        }
    }

    pub fn poll_btc_lifecycle(
        &mut self,
        draft_id: &str,
        broadcaster: &dyn BtcBroadcaster,
        now_unix: u64,
    ) -> Result<TxLifecycleStatus, String> {
        let record = self
            .records
            .get_mut(draft_id)
            .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;

        let txid = match &record.status {
            TxLifecycleStatus::Pending { tx_hash, .. } => tx_hash.clone(),
            TxLifecycleStatus::Confirmed { tx_hash, .. } => tx_hash.clone(),
            other => {
                return Err(format!(
                    "Cannot poll BTC lifecycle in status '{:?}', must be Pending or Confirmed",
                    other
                ))
            }
        };

        let status = broadcaster
            .query_status(&txid, record.network)
            .map_err(|e| format!("Query BTC status error: {}", e))?;

        match status {
            BtcTxQueryStatus::NotFound => {
                // If previously confirmed and now not found -> reorg or dropped!
                if let TxLifecycleStatus::Confirmed { block_height, .. } = record.status {
                    let reorg = TxLifecycleStatus::ReorgDetected {
                        tx_hash: txid.clone(),
                        previous_block_height: block_height,
                        previous_block_hash: "".to_string(),
                    };
                    record.transition_to(reorg.clone(), now_unix)?;
                    return Ok(reorg);
                }
                // If pending for > 7200 seconds (2 hours) and still not found, mark dropped
                if let TxLifecycleStatus::Pending { first_seen_unix, .. } = record.status {
                    if now_unix.saturating_sub(first_seen_unix) > 7200 {
                        let dropped = TxLifecycleStatus::Dropped {
                            tx_hash: txid,
                            reason: "Evicted from mempool after TTL".to_string(),
                        };
                        record.transition_to(dropped.clone(), now_unix)?;
                        return Ok(dropped);
                    }
                }
                Ok(record.status.clone())
            }
            BtcTxQueryStatus::InMempool => {
                let pending = TxLifecycleStatus::Pending {
                    tx_hash: txid,
                    first_seen_unix: now_unix,
                };
                // Only transition if not already Pending
                if !matches!(record.status, TxLifecycleStatus::Pending { .. }) {
                    record.transition_to(pending.clone(), now_unix)?;
                }
                Ok(record.status.clone())
            }
            BtcTxQueryStatus::Confirmed {
                block_height,
                block_hash,
                confirmations,
            } => {
                // Check if reorg occurred (block_hash mismatch at same height)
                if let Some((prev_height, prev_hash)) = self.confirmed_block_hashes.get(&txid) {
                    if *prev_height == block_height && *prev_hash != block_hash {
                        let reorg = TxLifecycleStatus::ReorgDetected {
                            tx_hash: txid.clone(),
                            previous_block_height: *prev_height,
                            previous_block_hash: prev_hash.clone(),
                        };
                        record.transition_to(reorg.clone(), now_unix)?;
                        return Ok(reorg);
                    }
                }

                self.confirmed_block_hashes
                    .insert(txid.clone(), (block_height, block_hash.clone()));

                let confirmed = TxLifecycleStatus::Confirmed {
                    tx_hash: txid,
                    block_height,
                    block_hash,
                    confirmations,
                };
                record.transition_to(confirmed.clone(), now_unix)?;
                Ok(confirmed)
            }
            BtcTxQueryStatus::Replaced { replacing_txid } => {
                let replaced = TxLifecycleStatus::Replaced {
                    old_tx_hash: txid,
                    new_tx_hash: replacing_txid,
                    reason: "Replaced via RBF in mempool".to_string(),
                };
                record.transition_to(replaced.clone(), now_unix)?;
                Ok(replaced)
            }
        }
    }

    pub fn poll_bsc_lifecycle(
        &mut self,
        draft_id: &str,
        broadcaster: &dyn BscBroadcaster,
        now_unix: u64,
    ) -> Result<TxLifecycleStatus, String> {
        let record = self
            .records
            .get_mut(draft_id)
            .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;

        let tx_hash = match &record.status {
            TxLifecycleStatus::Pending { tx_hash, .. } => tx_hash.clone(),
            TxLifecycleStatus::Confirmed { tx_hash, .. } => tx_hash.clone(),
            TxLifecycleStatus::ReorgDetected { tx_hash, .. } => tx_hash.clone(),
            other => {
                return Err(format!(
                    "Cannot poll BSC lifecycle in status '{:?}', must be Pending, Confirmed, or ReorgDetected",
                    other
                ))
            }
        };

        let receipt = broadcaster
            .query_receipt(&tx_hash, record.network)
            .map_err(|e| format!("Query BSC receipt error: {}", e))?;

        match receipt {
            BscReceiptStatus::Pending => {
                // If the transaction was previously Confirmed and now receipt returned Pending or disappeared,
                // this indicates a chain reorganization fork has dropped or invalidated the receipt!
                if let TxLifecycleStatus::Confirmed { block_height, block_hash, .. } = &record.status {
                    let reorg = TxLifecycleStatus::ReorgDetected {
                        tx_hash: tx_hash.clone(),
                        previous_block_height: *block_height,
                        previous_block_hash: block_hash.clone(),
                    };
                    record.transition_to(reorg.clone(), now_unix)?;
                    return Ok(reorg);
                }
                Ok(record.status.clone())
            }
            BscReceiptStatus::Success {
                block_number,
                block_hash,
                ..
            } => {
                // Check if reorg occurred
                if let Some((prev_height, prev_hash)) = self.confirmed_block_hashes.get(&tx_hash) {
                    if *prev_height == block_number && *prev_hash != block_hash {
                        let reorg = TxLifecycleStatus::ReorgDetected {
                            tx_hash: tx_hash.clone(),
                            previous_block_height: *prev_height,
                            previous_block_hash: prev_hash.clone(),
                        };
                        record.transition_to(reorg.clone(), now_unix)?;
                        return Ok(reorg);
                    }
                }

                self.confirmed_block_hashes
                    .insert(tx_hash.clone(), (block_number, block_hash.clone()));

                let confirmed = TxLifecycleStatus::Confirmed {
                    tx_hash,
                    block_height: block_number,
                    block_hash,
                    confirmations: 1,
                };
                record.transition_to(confirmed.clone(), now_unix)?;
                Ok(confirmed)
            }
            BscReceiptStatus::Reverted {
                reason, ..
            } => {
                let failed = TxLifecycleStatus::Failed {
                    stage: "execution".to_string(),
                    error_message: format!("EVM execution reverted (status 0x0): {}", reason),
                    permanent: true,
                };
                record.transition_to(failed.clone(), now_unix)?;
                Ok(failed)
            }
        }
    }
}
