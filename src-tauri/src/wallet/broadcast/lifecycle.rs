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
#[derive(Debug)]
pub struct TxLifecycleCoordinator {
    records: HashMap<String, TxLifecycleRecord>,
    confirmed_block_hashes: HashMap<String, (u64, String)>, // tx_hash -> (block_height, block_hash)
    pub storage_path: Option<std::path::PathBuf>,
}

#[allow(dead_code)]
impl Default for TxLifecycleCoordinator {
    fn default() -> Self {
        Self {
            records: HashMap::new(),
            confirmed_block_hashes: HashMap::new(),
            storage_path: None,
        }
    }
}

#[allow(dead_code)]
impl TxLifecycleCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_storage_path(&mut self, path: std::path::PathBuf) {
        self.storage_path = Some(path);
    }

    pub fn persist_if_configured(&self) -> Result<(), String> {
        if let Some(path) = &self.storage_path {
            self.save_to_file(path)?;
        }
        Ok(())
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
            let mut coord = Self::new();
            coord.storage_path = Some(path.to_path_buf());
            return Ok(coord);
        }
        let data = std::fs::read(path)
            .map_err(|e| format!("Cannot read lifecycle file: {}", e))?;
        let snapshot: LifecycleSnapshot = serde_json::from_slice(&data)
            .map_err(|e| format!("Cannot parse lifecycle file: {}", e))?;

        let mut coordinator = Self::new();
        coordinator.storage_path = Some(path.to_path_buf());
        for r in snapshot.records {
            coordinator.records.insert(r.record_id.clone(), r);
        }
        coordinator.confirmed_block_hashes = snapshot.confirmed_block_hashes;
        Ok(coordinator)
    }

    pub fn transition_record_and_persist(
        &mut self,
        draft_id: &str,
        next_status: TxLifecycleStatus,
        now_unix: u64,
    ) -> Result<(), String> {
        {
            let record = self
                .records
                .get_mut(draft_id)
                .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;
            record.transition_to(next_status, now_unix)?;
        }
        self.persist_if_configured()?;
        Ok(())
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
        let _ = self.persist_if_configured();
        Ok(())
    }

    pub fn record_signed(
        &mut self,
        draft_id: &str,
        payload_hash: &str,
        raw_tx_hex: &str,
        now_unix: u64,
    ) -> Result<(), String> {
        let signed_status = TxLifecycleStatus::Signed {
            draft_id: draft_id.to_string(),
            payload_hash: payload_hash.to_string(),
            raw_tx_hex: raw_tx_hex.to_string(),
        };
        self.transition_record_and_persist(draft_id, signed_status, now_unix)
    }

    pub fn record_signed_btc(
        &mut self,
        draft_id: &str,
        payload_hash: &str,
        raw_tx_hex: &str,
        txid: &str,
        wtxid: &str,
        approved_prevouts: Vec<crate::wallet::broadcast::types::ApprovedPrevout>,
        now_unix: u64,
    ) -> Result<(), String> {
        {
            let record = self
                .records
                .get_mut(draft_id)
                .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;
            record.expected_txid = Some(txid.to_string());
            record.expected_wtxid = Some(wtxid.to_string());
            record.approved_prevouts = Some(approved_prevouts);
        }
        let signed_status = TxLifecycleStatus::Signed {
            draft_id: draft_id.to_string(),
            payload_hash: payload_hash.to_string(),
            raw_tx_hex: raw_tx_hex.to_string(),
        };
        self.transition_record_and_persist(draft_id, signed_status, now_unix)
    }

    pub fn record_signed_bsc(
        &mut self,
        draft_id: &str,
        payload_hash: &str,
        raw_tx_hex: &str,
        sender: &str,
        recipient: &str,
        amount_raw: &str,
        nonce: u64,
        gas_limit: u64,
        gas_price: &str,
        now_unix: u64,
    ) -> Result<(), String> {
        let clean = raw_tx_hex.trim().trim_start_matches("0x").trim_start_matches("0X");
        let raw_bytes = hex::decode(clean).map_err(|e| format!("Invalid raw EVM hex: {}", e))?;
        let mut hasher = tiny_keccak::Keccak::v256();
        let mut hash = [0u8; 32];
        use tiny_keccak::Hasher;
        hasher.update(&raw_bytes);
        hasher.finalize(&mut hash);
        let computed_hash = format!("0x{}", hex::encode(hash));

        {
            let record = self
                .records
                .get_mut(draft_id)
                .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;
            record.expected_sender = Some(sender.to_string());
            record.expected_recipient = Some(recipient.to_string());
            record.expected_amount_raw = Some(amount_raw.to_string());
            record.expected_nonce = Some(nonce);
            record.expected_gas_limit = Some(gas_limit);
            record.expected_gas_price = Some(gas_price.to_string());
            record.expected_tx_hash = Some(computed_hash.clone());
            record.tx_hash = Some(computed_hash);
        }
        let signed_status = TxLifecycleStatus::Signed {
            draft_id: draft_id.to_string(),
            payload_hash: payload_hash.to_string(),
            raw_tx_hex: raw_tx_hex.to_string(),
        };
        self.transition_record_and_persist(draft_id, signed_status, now_unix)
    }

    pub fn execute_broadcast_btc(
        &mut self,
        draft_id: &str,
        broadcaster: &dyn BtcBroadcaster,
        max_retries: u32,
        now_unix: u64,
    ) -> Result<String, String> {
        // Mandatory persistence configuration
        if self.storage_path.is_none() {
            return Err("Mandatory storage path is not configured on TxLifecycleCoordinator".to_string());
        }

        let (raw_hex, expected_txid, expected_wtxid, approved_prevouts, network) = {
            let record = self
                .records
                .get(draft_id)
                .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;

            if record.network != BroadcastNetwork::BitcoinSignet {
                return Err("BTC broadcast requested for non-Bitcoin network".to_string());
            }

            let expected_txid = record.expected_txid.as_ref().ok_or_else(|| {
                "Mandatory draft verification failure: missing approved expected_txid".to_string()
            })?.clone();
            let expected_wtxid = record.expected_wtxid.as_ref().ok_or_else(|| {
                "Mandatory draft verification failure: missing approved expected_wtxid".to_string()
            })?.clone();
            let approved_prevouts = record.approved_prevouts.as_ref().ok_or_else(|| {
                "Mandatory draft verification failure: missing approved_prevouts (zero fallback)".to_string()
            })?.clone();
            if approved_prevouts.is_empty() {
                return Err("Mandatory draft verification failure: approved_prevouts cannot be empty".to_string());
            }

            let raw = match &record.status {
                TxLifecycleStatus::Signed { raw_tx_hex, .. } => raw_tx_hex.clone(),
                TxLifecycleStatus::Broadcasting { .. } => {
                    record.raw_tx_hex.clone().ok_or_else(|| "Missing raw_tx_hex in record".to_string())?
                }
                TxLifecycleStatus::Failed { permanent: false, .. } => {
                    record.raw_tx_hex.clone().ok_or_else(|| "Missing raw_tx_hex in record".to_string())?
                }
                other => {
                    return Err(format!(
                        "Cannot broadcast transaction in state '{:?}', must be Signed, Broadcasting, or non-permanent Failed",
                        other
                    ))
                }
            };
            (raw, expected_txid, expected_wtxid, approved_prevouts, record.network)
        };

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let broadcasting_status = TxLifecycleStatus::Broadcasting {
                draft_id: draft_id.to_string(),
                attempt,
            };
            // AUTO-PERSIST BEFORE NETWORK I/O (P1 Hardening)
            self.transition_record_and_persist(draft_id, broadcasting_status, now_unix)?;

            let broadcast_res = broadcaster.broadcast_verified(
                &raw_hex,
                &expected_txid,
                &expected_wtxid,
                &approved_prevouts,
                network,
            );

            match broadcast_res {
                Ok(txid) => {
                    let pending_status = TxLifecycleStatus::Pending {
                        tx_hash: txid.clone(),
                        first_seen_unix: now_unix,
                    };
                    self.transition_record_and_persist(draft_id, pending_status, now_unix)?;
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
                    self.transition_record_and_persist(draft_id, failed_status, now_unix)?;
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
        // Mandatory persistence configuration
        if self.storage_path.is_none() {
            return Err("Mandatory storage path is not configured on TxLifecycleCoordinator".to_string());
        }

        let (raw_hex, expected_sender, expected_recipient, expected_amount_raw, expected_nonce, expected_gas_limit, expected_gas_price, network) = {
            let record = self
                .records
                .get(draft_id)
                .ok_or_else(|| format!("Draft '{}' not found", draft_id))?;

            if record.network != BroadcastNetwork::BscTestnet {
                return Err("BSC broadcast requested for non-BSC network".to_string());
            }

            let expected_sender = record.expected_sender.as_ref().ok_or_else(|| {
                "Mandatory draft verification failure: missing approved expected_sender".to_string()
            })?.clone();
            let expected_recipient = record.expected_recipient.as_ref().ok_or_else(|| {
                "Mandatory draft verification failure: missing approved expected_recipient".to_string()
            })?.clone();
            let expected_amount_raw = record.expected_amount_raw.as_ref().ok_or_else(|| {
                "Mandatory draft verification failure: missing approved expected_amount_raw".to_string()
            })?.clone();
            let expected_nonce = record.expected_nonce.ok_or_else(|| {
                "Mandatory draft verification failure: missing approved expected_nonce".to_string()
            })?;
            let expected_gas_limit = record.expected_gas_limit.ok_or_else(|| {
                "Mandatory draft verification failure: missing approved expected_gas_limit".to_string()
            })?;
            let expected_gas_price = record.expected_gas_price.as_ref().ok_or_else(|| {
                "Mandatory draft verification failure: missing approved expected_gas_price".to_string()
            })?.clone();

            let raw = match &record.status {
                TxLifecycleStatus::Signed { raw_tx_hex, .. } => raw_tx_hex.clone(),
                TxLifecycleStatus::Broadcasting { .. } => {
                    record.raw_tx_hex.clone().ok_or_else(|| "Missing raw_tx_hex in record".to_string())?
                }
                TxLifecycleStatus::Failed { permanent: false, .. } => {
                    record.raw_tx_hex.clone().ok_or_else(|| "Missing raw_tx_hex in record".to_string())?
                }
                other => {
                    return Err(format!(
                        "Cannot broadcast transaction in state '{:?}', must be Signed, Broadcasting, or non-permanent Failed",
                        other
                    ))
                }
            };
            (
                raw,
                expected_sender,
                expected_recipient,
                expected_amount_raw,
                expected_nonce,
                expected_gas_limit,
                expected_gas_price,
                record.network,
            )
        };

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let broadcasting_status = TxLifecycleStatus::Broadcasting {
                draft_id: draft_id.to_string(),
                attempt,
            };
            // AUTO-PERSIST BEFORE NETWORK I/O (P1 Hardening)
            self.transition_record_and_persist(draft_id, broadcasting_status, now_unix)?;

            let broadcast_res = broadcaster.broadcast_verified(
                &raw_hex,
                &expected_sender,
                &expected_recipient,
                &expected_amount_raw,
                expected_nonce,
                expected_gas_limit,
                &expected_gas_price,
                network,
            );

            match broadcast_res {
                Ok(tx_hash) => {
                    let pending_status = TxLifecycleStatus::Pending {
                        tx_hash: tx_hash.clone(),
                        first_seen_unix: now_unix,
                    };
                    self.transition_record_and_persist(draft_id, pending_status, now_unix)?;
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
                    self.transition_record_and_persist(draft_id, failed_status, now_unix)?;
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
            TxLifecycleStatus::Broadcasting { .. } | TxLifecycleStatus::Failed { permanent: false, .. } => {
                record
                    .tx_hash
                    .clone()
                    .or_else(|| record.expected_txid.clone())
                    .ok_or_else(|| "Cannot poll BTC lifecycle: missing txid in record".to_string())?
            }
            other => {
                return Err(format!(
                    "Cannot poll BTC lifecycle in status '{:?}', must be Pending, Confirmed, Broadcasting, or non-permanent Failed",
                    other
                ))
            }
        };

        let status = broadcaster
            .query_status(&txid, record.network)
            .map_err(|e| format!("Query BTC status error: {}", e))?;

        let res = match status {
            BtcTxQueryStatus::NotFound => {
                // If previously confirmed and now not found -> reorg or dropped!
                if let TxLifecycleStatus::Confirmed { block_height, .. } = record.status {
                    let reorg = TxLifecycleStatus::ReorgDetected {
                        tx_hash: txid.clone(),
                        previous_block_height: block_height,
                        previous_block_hash: "".to_string(),
                    };
                    record.transition_to(reorg.clone(), now_unix)?;
                    reorg
                } else if let TxLifecycleStatus::Pending { first_seen_unix, .. } = record.status {
                    // If pending for > 7200 seconds (2 hours) and still not found, mark dropped
                    if now_unix.saturating_sub(first_seen_unix) > 7200 {
                        let dropped = TxLifecycleStatus::Dropped {
                            tx_hash: txid,
                            reason: "Evicted from mempool after TTL".to_string(),
                        };
                        record.transition_to(dropped.clone(), now_unix)?;
                        dropped
                    } else {
                        record.status.clone()
                    }
                } else {
                    record.status.clone()
                }
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
                record.status.clone()
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
                        self.persist_if_configured()?;
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
                confirmed
            }
            BtcTxQueryStatus::Replaced { replacing_txid } => {
                let replaced = TxLifecycleStatus::Replaced {
                    old_tx_hash: txid,
                    new_tx_hash: replacing_txid,
                    reason: "Replaced via RBF in mempool".to_string(),
                };
                record.transition_to(replaced.clone(), now_unix)?;
                replaced
            }
        };

        self.persist_if_configured()?;
        Ok(res)
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
            TxLifecycleStatus::Broadcasting { .. } | TxLifecycleStatus::Failed { permanent: false, .. } => {
                record
                    .tx_hash
                    .clone()
                    .or_else(|| record.expected_tx_hash.clone())
                    .ok_or_else(|| "Cannot poll BSC lifecycle: missing tx_hash in record".to_string())?
            }
            other => {
                return Err(format!(
                    "Cannot poll BSC lifecycle in status '{:?}', must be Pending, Confirmed, ReorgDetected, Broadcasting, or non-permanent Failed",
                    other
                ))
            }
        };

        let receipt = broadcaster
            .query_receipt(&tx_hash, record.network)
            .map_err(|e| format!("Query BSC receipt error: {}", e))?;

        let res = match receipt {
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
                    reorg
                } else {
                    record.status.clone()
                }
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
                        self.persist_if_configured()?;
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
                confirmed
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
                failed
            }
        };

        self.persist_if_configured()?;
        Ok(res)
    }
}
