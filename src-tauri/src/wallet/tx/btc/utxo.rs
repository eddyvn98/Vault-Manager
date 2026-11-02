use std::time::Duration;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct EsploraUtxoStatus {
    pub confirmed: bool,
    #[allow(dead_code)]
    pub block_height: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EsploraUtxo {
    pub txid: String,
    pub vout: u32,
    pub status: EsploraUtxoStatus,
    pub value: u64,
}

pub async fn fetch_signet_utxos(
    client: &Client,
    address: &str,
) -> Result<Vec<EsploraUtxo>, String> {
    let url = format!("https://mempool.space/signet/api/address/{}/utxo", address);
    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Esplora UTXO endpoint: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Esplora UTXO query returned HTTP {}", resp.status()));
    }

    let utxos: Vec<EsploraUtxo> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Esplora UTXO response: {}", e))?;

    Ok(utxos)
}

/// Greedy largest-first coin selection for unsigned transaction preparation.
/// Returns (selected_utxos, total_selected_satoshis).
pub fn select_utxos_greedy(
    available_utxos: &[EsploraUtxo],
    target_amount_sats: u64,
    estimated_fee_sats: u64,
) -> Result<(Vec<EsploraUtxo>, u64), String> {
    let needed = target_amount_sats.saturating_add(estimated_fee_sats);
    let mut confirmed: Vec<EsploraUtxo> = available_utxos
        .iter()
        .filter(|u| u.status.confirmed)
        .cloned()
        .collect();

    // Sort descending by value (largest first)
    confirmed.sort_by(|a, b| b.value.cmp(&a.value));

    let mut selected = Vec::new();
    let mut total_sats: u64 = 0;

    for utxo in confirmed {
        selected.push(utxo.clone());
        total_sats = total_sats.saturating_add(utxo.value);
        if total_sats >= needed {
            break;
        }
    }

    if total_sats < needed {
        return Err(format!(
            "Insufficient funds: available {} sats, but need {} sats (including {} sats fee)",
            total_sats, needed, estimated_fee_sats
        ));
    }

    Ok((selected, total_sats))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coin_selection() {
        let utxos = vec![
            EsploraUtxo {
                txid: "1111111111111111111111111111111111111111111111111111111111111111".into(),
                vout: 0,
                status: EsploraUtxoStatus { confirmed: true, block_height: Some(100) },
                value: 10_000,
            },
            EsploraUtxo {
                txid: "2222222222222222222222222222222222222222222222222222222222222222".into(),
                vout: 1,
                status: EsploraUtxoStatus { confirmed: true, block_height: Some(101) },
                value: 50_000,
            },
            EsploraUtxo {
                txid: "3333333333333333333333333333333333333333333333333333333333333333".into(),
                vout: 0,
                status: EsploraUtxoStatus { confirmed: false, block_height: None }, // unconfirmed
                value: 100_000,
            },
        ];

        // Need 40,000 + 1,000 fee = 41,000
        // Largest confirmed is 50,000, which satisfies it alone
        let (selected, total) = select_utxos_greedy(&utxos, 40_000, 1_000).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].value, 50_000);
        assert_eq!(total, 50_000);

        // Insufficient funds
        assert!(select_utxos_greedy(&utxos, 70_000, 1_000).is_err());
    }
}
