use crate::wallet::broadcast::types::BroadcastNetwork;
use bitcoin::hashes::Hash;
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
    fn broadcast_verified(
        &self,
        raw_tx_hex: &str,
        expected_txid: &str,
        expected_wtxid: &str,
        expected_prevouts: &[crate::wallet::broadcast::types::ApprovedPrevout],
        network: BroadcastNetwork,
    ) -> Result<String, BtcBroadcastError>;
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

    fn broadcast_verified(
        &self,
        raw_tx_hex: &str,
        _expected_txid: &str,
        _expected_wtxid: &str,
        _expected_prevouts: &[crate::wallet::broadcast::types::ApprovedPrevout],
        network: BroadcastNetwork,
    ) -> Result<String, BtcBroadcastError> {
        self.broadcast(raw_tx_hex, network)
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

/// Real network adapter for Bitcoin Signet using Esplora API (mempool.space).
/// Designed with strict testnet-only boundaries and feature-locking (G5A/G5B).
#[allow(dead_code)]
pub struct EsploraBtcBroadcaster {
    pub esplora_url: String,
    pub broadcast_enabled: bool,
    client: Option<reqwest::blocking::Client>,
}

#[allow(dead_code)]
impl EsploraBtcBroadcaster {
    pub fn new(broadcast_enabled: bool) -> Self {
        Self::with_endpoint("https://mempool.space/signet/api", broadcast_enabled)
    }

    pub fn with_endpoint(endpoint: &str, broadcast_enabled: bool) -> Self {
        let client = if broadcast_enabled {
            reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .ok()
        } else {
            None
        };

        Self {
            esplora_url: endpoint.to_string(),
            broadcast_enabled,
            client,
        }
    }

    /// Dry-run verification (G5A): Deserializes raw tx, validates inputs/outputs,
    /// checks SegWit witnesses, checks testnet network, and calculates txid and wtxid locally without any network POST.
    pub fn dry_run_verify(&self, raw_tx_hex: &str, network: BroadcastNetwork) -> Result<(bitcoin::Transaction, String, String), BtcBroadcastError> {
        if network != BroadcastNetwork::BitcoinSignet {
            return Err(BtcBroadcastError::NetworkForbidden(
                format!("Network {:?} is strictly forbidden. Only Bitcoin Signet is permitted.", network)
            ));
        }

        let clean_hex = raw_tx_hex.trim();
        let raw_bytes = hex::decode(clean_hex)
            .map_err(|e| BtcBroadcastError::NodeRejected(format!("Invalid raw tx hex: {}", e)))?;

        let tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&raw_bytes)
            .map_err(|e| BtcBroadcastError::NodeRejected(format!("Cannot deserialize raw BTC transaction: {}", e)))?;

        if tx.input.is_empty() {
            return Err(BtcBroadcastError::NodeRejected("Transaction has no inputs".to_string()));
        }
        if tx.output.is_empty() {
            return Err(BtcBroadcastError::NodeRejected("Transaction has no outputs".to_string()));
        }

        // BIP-141 SegWit Witness Check:
        // Every input of a native SegWit (BIP84) transaction must contain non-empty witness data
        for (i, input) in tx.input.iter().enumerate() {
            if input.witness.is_empty() {
                return Err(BtcBroadcastError::NodeRejected(format!(
                    "Input {} lacks SegWit witness: raw signed transaction must contain witness data",
                    i
                )));
            }
            if input.witness.len() != 2 {
                return Err(BtcBroadcastError::NodeRejected(format!(
                    "Input {} has invalid witness element count: expected 2 elements [sig, pubkey] for BIP84 P2WPKH, got {}",
                    i, input.witness.len()
                )));
            }
            let sig_bytes = input.witness.nth(0).unwrap();
            let pubkey_bytes = input.witness.nth(1).unwrap();

            if pubkey_bytes.len() != 33 {
                return Err(BtcBroadcastError::NodeRejected(format!(
                    "Input {} has invalid pubkey length: expected 33 bytes compressed pubkey, got {}",
                    i, pubkey_bytes.len()
                )));
            }
            secp256k1::PublicKey::from_slice(pubkey_bytes).map_err(|e| {
                BtcBroadcastError::NodeRejected(format!("Input {} has invalid compressed public key: {}", i, e))
            })?;

            if sig_bytes.is_empty() {
                return Err(BtcBroadcastError::NodeRejected(format!(
                    "Input {} has empty signature in witness", i
                )));
            }
            let sighash_type = sig_bytes[sig_bytes.len() - 1];
            if sighash_type != 0x01 {
                return Err(BtcBroadcastError::NodeRejected(format!(
                    "Input {} has unsupported sighash type 0x{:02x}, only SIGHASH_ALL (0x01) supported",
                    i, sighash_type
                )));
            }
            let der_bytes = &sig_bytes[..sig_bytes.len() - 1];
            secp256k1::ecdsa::Signature::from_der(der_bytes).map_err(|e| {
                BtcBroadcastError::NodeRejected(format!("Input {} has invalid DER signature: {}", i, e))
            })?;
        }

        let computed_txid = tx.compute_txid().to_string();
        let computed_wtxid = tx.compute_wtxid().to_string();
        Ok((tx, computed_txid, computed_wtxid))
    }

    /// Cryptographically verifies ECDSA DER signatures against compressed pubkeys and BIP-143 sighash,
    /// and enforces exact match of prevout outpoints, values, and script_pubkeys.
    pub fn verify_input_signatures(
        &self,
        tx: &bitcoin::Transaction,
        prevouts: &[crate::wallet::broadcast::types::ApprovedPrevout],
    ) -> Result<(), BtcBroadcastError> {
        if prevouts.is_empty() {
            return Err(BtcBroadcastError::NodeRejected(
                "Mandatory verification failure: approved_prevouts cannot be empty".to_string(),
            ));
        }
        if prevouts.len() != tx.input.len() {
            return Err(BtcBroadcastError::NodeRejected(format!(
                "Approved prevouts count ({}) does not match transaction input count ({})",
                prevouts.len(),
                tx.input.len()
            )));
        }

        let mut sighash_cache = bitcoin::sighash::SighashCache::new(tx);
        let secp = secp256k1::Secp256k1::verification_only();

        for (i, prevout) in prevouts.iter().enumerate() {
            // 1. Strict Outpoint binding: verify input txid and vout match approved prevout
            let input_txid = tx.input[i].previous_output.txid.to_string();
            let input_vout = tx.input[i].previous_output.vout;
            if input_txid != prevout.txid || input_vout != prevout.vout {
                return Err(BtcBroadcastError::NodeRejected(format!(
                    "Input {} outpoint '{}:{}' does not match approved prevout '{}:{}'",
                    i, input_txid, input_vout, prevout.txid, prevout.vout
                )));
            }

            let witness = &tx.input[i].witness;
            let sig_bytes = witness.nth(0).ok_or_else(|| {
                BtcBroadcastError::NodeRejected(format!("Input {} missing signature witness", i))
            })?;
            let pubkey_bytes = witness.nth(1).ok_or_else(|| {
                BtcBroadcastError::NodeRejected(format!("Input {} missing pubkey witness", i))
            })?;

            let pubkey = secp256k1::PublicKey::from_slice(pubkey_bytes).map_err(|e| {
                BtcBroadcastError::NodeRejected(format!("Input {} invalid pubkey: {}", i, e))
            })?;

            // 2. Strict ScriptPubkey binding: verify pubkey derives to approved prevout script_pubkey
            let expected_script_bytes = hex::decode(prevout.script_pubkey_hex.trim_start_matches("0x"))
                .map_err(|e| BtcBroadcastError::NodeRejected(format!("Invalid prevout script_pubkey hex: {}", e)))?;
            let expected_script = bitcoin::ScriptBuf::from_bytes(expected_script_bytes);
            let derived_script = bitcoin::ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::hash(pubkey_bytes));
            if derived_script != expected_script {
                return Err(BtcBroadcastError::NodeRejected(format!(
                    "Input {} witness pubkey script '{}' does not match approved prevout script_pubkey '{}'",
                    i, derived_script.to_hex_string(), prevout.script_pubkey_hex
                )));
            }

            // 3. Strict DER Signature parsing
            let der_bytes = &sig_bytes[..sig_bytes.len() - 1];
            let sig = secp256k1::ecdsa::Signature::from_der(der_bytes).map_err(|e| {
                BtcBroadcastError::NodeRejected(format!("Input {} invalid DER signature: {}", i, e))
            })?;

            // 4. BIP-143 Sighash Calculation with approved satoshis
            let sighash = sighash_cache.p2wpkh_signature_hash(
                i,
                &expected_script,
                bitcoin::Amount::from_sat(prevout.value_satoshis),
                bitcoin::sighash::EcdsaSighashType::All,
            ).map_err(|e| BtcBroadcastError::NodeRejected(format!("Failed to calculate BIP-143 sighash for input {}: {}", i, e)))?;

            let msg = secp256k1::Message::from_digest(sighash.to_byte_array());
            secp.verify_ecdsa(&msg, &sig, &pubkey).map_err(|e| {
                BtcBroadcastError::NodeRejected(format!("Cryptographic signature verification failed for input {}: {}", i, e))
            })?;
        }

        Ok(())
    }

    /// Verifies that the raw transaction txid AND wtxid strictly match the approved draft,
    /// and cryptographically verifies ECDSA signatures against approved prevouts before broadcasting (Zero Fallback).
    pub fn broadcast_verified(
        &self,
        raw_tx_hex: &str,
        expected_txid: &str,
        expected_wtxid: &str,
        expected_prevouts: &[crate::wallet::broadcast::types::ApprovedPrevout],
        network: BroadcastNetwork,
    ) -> Result<String, BtcBroadcastError> {
        let (tx, computed_txid, computed_wtxid) = self.dry_run_verify(raw_tx_hex, network)?;
        if computed_txid != expected_txid {
            return Err(BtcBroadcastError::NodeRejected(format!(
                "Raw transaction txid '{}' does not match approved transaction txid '{}'",
                computed_txid, expected_txid
            )));
        }
        if computed_wtxid != expected_wtxid {
            return Err(BtcBroadcastError::NodeRejected(format!(
                "Raw transaction wtxid '{}' does not match approved transaction wtxid '{}' (witness mismatch)",
                computed_wtxid, expected_wtxid
            )));
        }
        // MANDATORY: Always verify cryptographic signatures against approved prevouts
        self.verify_input_signatures(&tx, expected_prevouts)?;
        self.broadcast(raw_tx_hex, network)
    }
}

impl BtcBroadcaster for EsploraBtcBroadcaster {
    fn broadcast(&self, raw_tx_hex: &str, network: BroadcastNetwork) -> Result<String, BtcBroadcastError> {
        let (_tx, computed_txid, _computed_wtxid) = self.dry_run_verify(raw_tx_hex, network)?;

        // If broadcast is locked (feature-gated / default mode): return dry-run computed txid
        if !self.broadcast_enabled {
            return Ok(format!("dry_run_{}", computed_txid));
        }

        // Live broadcast (G5B locked mode):
        let http_client = self.client.as_ref().ok_or_else(|| {
            BtcBroadcastError::NodeRejected("Broadcaster HTTP client not initialized".into())
        })?;
        let post_url = format!("{}/tx", self.esplora_url);
        let resp = match http_client.post(&post_url).body(raw_tx_hex.trim().to_string()).send() {
            Ok(r) => r,
            Err(e) => {
                // If timeout occurred, DO NOT immediately retry or create a competing transaction:
                // First query whether the transaction reached the mempool!
                if let Ok(BtcTxQueryStatus::InMempool | BtcTxQueryStatus::Confirmed { .. }) = self.query_status(&computed_txid, network) {
                    return Ok(computed_txid);
                }
                return Err(BtcBroadcastError::Timeout(format!("Esplora POST /tx timed out: {}", e)));
            }
        };

        let status_code = resp.status().as_u16();
        let body = resp.text().unwrap_or_default();

        if status_code == 200 {
            let returned_txid = body.trim().to_string();
            if returned_txid != computed_txid {
                return Err(BtcBroadcastError::NodeRejected(format!(
                    "Returned txid '{}' does not match locally calculated txid '{}'",
                    returned_txid, computed_txid
                )));
            }
            Ok(computed_txid)
        } else {
            Err(parse_esplora_error(status_code, &body))
        }
    }

    fn broadcast_verified(
        &self,
        raw_tx_hex: &str,
        expected_txid: &str,
        expected_wtxid: &str,
        expected_prevouts: &[crate::wallet::broadcast::types::ApprovedPrevout],
        network: BroadcastNetwork,
    ) -> Result<String, BtcBroadcastError> {
        self.broadcast_verified(raw_tx_hex, expected_txid, expected_wtxid, expected_prevouts, network)
    }

    fn query_status(&self, txid: &str, network: BroadcastNetwork) -> Result<BtcTxQueryStatus, BtcBroadcastError> {
        if network != BroadcastNetwork::BitcoinSignet {
            return Err(BtcBroadcastError::NetworkForbidden(
                format!("Network {:?} is strictly forbidden. Only Bitcoin Signet is permitted.", network)
            ));
        }

        let http_client = self.client.as_ref().ok_or_else(|| {
            BtcBroadcastError::NodeRejected("Broadcaster HTTP client not initialized".into())
        })?;
        let url = format!("{}/tx/{}/status", self.esplora_url, txid);
        let resp = http_client.get(&url).send()
            .map_err(|e| BtcBroadcastError::Timeout(format!("Esplora query_status timed out: {}", e)))?;

        let status_code = resp.status().as_u16();
        if status_code == 404 {
            return Ok(BtcTxQueryStatus::NotFound);
        }
        if !resp.status().is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(parse_esplora_error(status_code, &body));
        }

        let json: serde_json::Value = resp.json()
            .map_err(|e| BtcBroadcastError::NodeRejected(format!("Failed to parse status JSON: {}", e)))?;

        let confirmed = json.get("confirmed").and_then(|v| v.as_bool()).unwrap_or(false);
        if confirmed {
            let block_height = json.get("block_height").and_then(|v| v.as_u64()).unwrap_or(0);
            let block_hash = json.get("block_hash").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(BtcTxQueryStatus::Confirmed {
                block_height,
                block_hash,
                confirmations: 1,
            })
        } else {
            Ok(BtcTxQueryStatus::InMempool)
        }
    }
}
