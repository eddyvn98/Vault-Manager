use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UtxoInfo {
    pub txid: String,
    pub vout: u32,
    pub value_satoshis: u64,
    pub address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BtcTxPreview {
    pub draft_id: String,
    pub network: String,
    pub recipient_address: String,
    pub amount_satoshis: u64,
    pub formatted_amount_btc: String,
    pub fee_satoshis: u64,
    pub formatted_fee_btc: String,
    pub fee_rate_sat_per_vb: u64,
    pub selected_utxos: Vec<UtxoInfo>,
    pub change_address: String,
    pub change_satoshis: u64,
    pub unsigned_psbt_base64: String,
    pub signing_enabled: bool,
    pub note: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BscCanonicalPayload {
    pub chain_id: u64,
    pub token_contract: Option<String>,
    pub recipient_address: String,
    pub amount_raw: String,
    pub nonce: u64,
    pub gas_limit: u64,
    pub gas_price_wei: String,
    pub calldata_hex: String,
}

impl BscCanonicalPayload {
    pub fn canonical_hash(&self) -> String {
        let canonical_str = format!(
            "bsc:{}:{}:{}:{}:{}:{}:{}:{}",
            self.chain_id,
            self.token_contract.as_deref().unwrap_or("").to_lowercase(),
            self.recipient_address.to_lowercase(),
            self.amount_raw,
            self.nonce,
            self.gas_limit,
            self.gas_price_wei,
            self.calldata_hex.to_lowercase()
        );
        let mut hasher = tiny_keccak::Keccak::v256();
        let mut hash = [0u8; 32];
        tiny_keccak::Hasher::update(&mut hasher, canonical_str.as_bytes());
        tiny_keccak::Hasher::finalize(hasher, &mut hash);
        format!("0x{}", hex::encode(hash))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BscTxPreview {
    pub draft_id: String,
    pub chain_id: u64,
    pub network_name: String,
    pub is_token_transfer: bool,
    pub token_symbol: String,
    pub token_contract: Option<String>,
    pub from_address: String,
    pub recipient_address: String,
    pub amount_raw: String,
    pub formatted_amount: String,
    pub nonce: u64,
    pub gas_limit: u64,
    pub gas_price_gwei: String,
    pub gas_price_wei: String,
    pub estimated_fee_bnb: String,
    pub calldata_hex: String,
    pub unsigned_tx_summary: String,
    pub signing_enabled: bool,
    pub note: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct PrepareBtcParams {
    pub recipient: String,
    pub amount_satoshis: u64,
    pub fee_rate_sat_vb: Option<u64>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct PrepareBscParams {
    pub recipient: String,
    pub amount_tokens: String,
}
