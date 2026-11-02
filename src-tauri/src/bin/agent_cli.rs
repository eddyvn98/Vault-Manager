use std::path::PathBuf;
use vault_manager_lib::wallet::{
    bip,
    broadcast::{self, lifecycle::TxLifecycleCoordinator},
    crypto,
    lockout::LockoutManager,
    tx,
    compute_btc_payload_hash, sign_verified_btc_draft, ActiveDraft, SessionState,
};
use zeroize::Zeroizing;

fn app_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME environment variable required");
    PathBuf::from(home).join("Library/Application Support/com.eddyvn98.vaultmanager.wallet.experimental")
}

fn vault_file() -> PathBuf {
    app_dir().join("wallet-v1.enc")
}

fn lifecycle_file() -> PathBuf {
    app_dir().join("tx_lifecycle.json")
}

fn sqlite_db_path() -> PathBuf {
    let d = app_dir().join("bdk_signet_db");
    let _ = std::fs::create_dir_all(&d);
    d.join("bdk_signet.sqlite3")
}

fn read_mnemonic(password: &str) -> Result<Zeroizing<String>, String> {
    let file = vault_file();
    if !file.exists() {
        return Err(format!("Vault file not found at {:?}", file));
    }
    let raw = std::fs::read(&file).map_err(|e| format!("Cannot read vault file: {}", e))?;
    crypto::decrypt(password, &raw).map_err(|e| format!("Failed to decrypt vault: {}", e))
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: agent_cli <subcommand> [options]\n\
             Subcommands:\n  \
               status [--password <pw>]\n  \
               btc-balance [--address <addr>]\n  \
               btc-send --to <addr> --amount <sats> [--fee-rate <rate>] --password <pw> [--broadcast]\n  \
               btc-status --txid <txid>\n  \
               lifecycle-list\n"
        );
        std::process::exit(1);
    }

    let subcmd = args[1].as_str();

    let get_arg = |flag: &str| -> Option<String> {
        args.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
    };
    let has_flag = |flag: &str| -> bool {
        args.iter().any(|a| a == flag)
    };

    match subcmd {
        "status" => {
            let file = vault_file();
            let exists = file.exists();
            let mut out = serde_json::json!({
                "vault_path": file.to_string_lossy(),
                "vault_exists": exists,
                "lifecycle_path": lifecycle_file().to_string_lossy(),
                "lifecycle_exists": lifecycle_file().exists(),
            });

            if let Some(pw) = get_arg("--password") {
                match read_mnemonic(&pw) {
                    Ok(mnemonic) => {
                        let signet_0 = bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 0, 0).unwrap_or_default();
                        let signet_1 = bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 0, 1).unwrap_or_default();
                        let bsc_0 = bip::derive_bsc_address(&mnemonic).unwrap_or_default();
                        out["signet_receive_0"] = serde_json::json!(signet_0);
                        out["signet_receive_1"] = serde_json::json!(signet_1);
                        out["bsc_address_0"] = serde_json::json!(bsc_0);
                        out["unlocked"] = serde_json::json!(true);
                    }
                    Err(e) => {
                        out["unlock_error"] = serde_json::json!(e);
                        out["unlocked"] = serde_json::json!(false);
                    }
                }
            }
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }

        "btc-balance" => {
            let addr = if let Some(a) = get_arg("--address") {
                a
            } else {
                let pw = get_arg("--password").unwrap_or_else(|| "TestnetWalletPass123!@#".to_string());
                let mnemonic = match read_mnemonic(&pw) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": e}));
                        std::process::exit(1);
                    }
                };
                bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 0, 0).unwrap()
            };

            let esplora_url = "https://mempool.space/signet/api";
            let client = reqwest::Client::new();
            let resp = client.get(format!("{}/address/{}", esplora_url, addr)).send().await;
            match resp {
                Ok(r) => {
                    let json: serde_json::Value = r.json().await.unwrap_or_default();
                    let utxos: serde_json::Value = match client.get(format!("{}/address/{}/utxo", esplora_url, addr)).send().await {
                        Ok(u) => u.json().await.unwrap_or_default(),
                        Err(_) => serde_json::Value::Array(vec![]),
                    };

                    let chain_funded = json["chain_stats"]["funded_txo_sum"].as_i64().unwrap_or(0);
                    let chain_spent = json["chain_stats"]["spent_txo_sum"].as_i64().unwrap_or(0);
                    let confirmed_sats = chain_funded - chain_spent;

                    let mempool_funded = json["mempool_stats"]["funded_txo_sum"].as_i64().unwrap_or(0);
                    let mempool_spent = json["mempool_stats"]["spent_txo_sum"].as_i64().unwrap_or(0);
                    let mempool_delta_sats = mempool_funded - mempool_spent;

                    let out = serde_json::json!({
                        "address": addr,
                        "confirmed_sats": confirmed_sats,
                        "mempool_delta_sats": mempool_delta_sats,
                        "total_sats": confirmed_sats + mempool_delta_sats,
                        "utxo_count": utxos.as_array().map(|a| a.len()).unwrap_or(0),
                        "utxos": utxos,
                    });
                    println!("{}", serde_json::to_string_pretty(&out).unwrap());
                }
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("Network request failed: {}", e)}));
                    std::process::exit(1);
                }
            }
        }

        "btc-send" => {
            let to = get_arg("--to").expect("--to <recipient> is required");
            let amount_str = get_arg("--amount").expect("--amount <satoshis> is required");
            let amount_sats: u64 = amount_str.parse().expect("amount must be an integer");
            let fee_rate_str = get_arg("--fee-rate").unwrap_or_else(|| "2".to_string());
            let fee_rate: u64 = fee_rate_str.parse().expect("fee-rate must be an integer");
            let pw = get_arg("--password").expect("--password <pw> is required");
            let should_broadcast = has_flag("--broadcast");

            // 1. Decrypt mnemonic
            let mnemonic = match read_mnemonic(&pw) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": e}));
                    std::process::exit(1);
                }
            };

            // 2. Prepare transaction
            let db_path = sqlite_db_path();
            let (preview, policy) = match tx::btc::prepare::prepare_signet_tx(
                &mnemonic,
                &to,
                amount_sats,
                Some(fee_rate),
                Some(&db_path),
            ).await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("Prepare failed: {}", e)}));
                    std::process::exit(1);
                }
            };

            // 3. Setup SessionState & verify/consume draft
            let mut session = SessionState::default();
            session.mnemonic = Some(mnemonic.clone());
            session.session_epoch = 1;
            let payload_hash = compute_btc_payload_hash(&preview.unsigned_psbt_base64);
            let now_unix = LockoutManager::now_unix();
            session.active_draft = Some(ActiveDraft {
                draft_id: preview.draft_id.clone(),
                session_epoch: 1,
                network: "bitcoin_signet".to_string(),
                payload_hash: payload_hash.clone(),
                expires_at_unix: now_unix + 600,
            });

            // 4. Sign transaction
            let signed = match sign_verified_btc_draft(
                &mut session,
                &preview.draft_id,
                &preview.unsigned_psbt_base64,
                &policy,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("Signing failed: {}", e)}));
                    std::process::exit(1);
                }
            };

            // 5. Preflight dry-run verification
            let esplora_dry = broadcast::btc::EsploraBtcBroadcaster::new(false);
            let (tx, computed_txid, computed_wtxid) = match esplora_dry.dry_run_verify(
                &signed.raw_tx_hex,
                broadcast::types::BroadcastNetwork::BitcoinSignet,
            ) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("Preflight dry_run_verify failed: {}", e)}));
                    std::process::exit(1);
                }
            };

            let approved_prevouts: Vec<broadcast::types::ApprovedPrevout> = policy
                .approved_inputs
                .iter()
                .map(|inp| broadcast::types::ApprovedPrevout {
                    txid: inp.outpoint.txid.to_string(),
                    vout: inp.outpoint.vout,
                    value_satoshis: inp.value_satoshis,
                    script_pubkey_hex: hex::encode(inp.script_pubkey.as_bytes()),
                })
                .collect();

            if let Err(e) = esplora_dry.verify_input_signatures(&tx, &approved_prevouts) {
                eprintln!("{}", serde_json::json!({"error": format!("Preflight verify_input_signatures failed: {}", e)}));
                std::process::exit(1);
            }

            // 6. Record into TxLifecycleCoordinator
            let l_path = lifecycle_file();
            let mut coord = if l_path.exists() {
                TxLifecycleCoordinator::load_from_file(&l_path).unwrap_or_else(|_| {
                    let mut c = TxLifecycleCoordinator::new();
                    c.set_storage_path(l_path.clone());
                    c
                })
            } else {
                let mut c = TxLifecycleCoordinator::new();
                c.set_storage_path(l_path.clone());
                c
            };

            let _ = coord.register_draft(&preview.draft_id, broadcast::types::BroadcastNetwork::BitcoinSignet, now_unix);
            let _ = coord.record_signed_btc(
                &preview.draft_id,
                &signed.payload_hash,
                &signed.raw_tx_hex,
                &signed.txid,
                &signed.wtxid,
                approved_prevouts.clone(),
                now_unix,
            );

            let mut out = serde_json::json!({
                "action": "btc-send",
                "status": "signed_and_preflight_passed",
                "draft_id": preview.draft_id,
                "txid": computed_txid,
                "wtxid": computed_wtxid,
                "recipient": preview.recipient_address,
                "amount_satoshis": preview.amount_satoshis,
                "fee_satoshis": preview.fee_satoshis,
                "fee_rate_sat_vb": preview.fee_rate_sat_per_vb,
                "change_address": preview.change_address,
                "change_satoshis": preview.change_satoshis,
                "inputs_count": preview.selected_utxos.len(),
                "preflight": {
                    "valid": true,
                    "txid_matches": computed_txid == signed.txid,
                    "wtxid_matches": computed_wtxid == signed.wtxid,
                    "signatures_verified": true,
                },
                "broadcast_requested": should_broadcast,
                "broadcasted": false,
            });

            // 7. Broadcast if requested
            if should_broadcast {
                let gate = std::env::var("VAULT_ENABLE_TESTNET_BROADCAST").unwrap_or_default();
                if gate != "1" {
                    eprintln!("{}", serde_json::json!({
                        "error": "VAULT_ENABLE_TESTNET_BROADCAST=1 environment variable is required to execute live testnet broadcast"
                    }));
                    std::process::exit(1);
                }

                let ctx = match coord.begin_broadcast_btc(&preview.draft_id, 1, now_unix) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": format!("Coordinator begin_broadcast failed: {}", e)}));
                        std::process::exit(1);
                    }
                };

                let lease_token = ctx.lease_token;
                let broadcast_res = tokio::task::spawn_blocking(move || {
                    let broadcaster = broadcast::btc::EsploraBtcBroadcaster::new(true);
                    broadcaster.broadcast_verified(
                        &ctx.raw_hex,
                        &ctx.expected_txid,
                        &ctx.expected_wtxid,
                        &ctx.approved_prevouts,
                        ctx.network,
                    )
                }).await.unwrap();

                let outcome = match coord.complete_broadcast_btc(&preview.draft_id, lease_token, broadcast_res, 1, 1, now_unix) {
                    Ok(o) => o,
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": format!("Coordinator complete_broadcast failed: {}", e)}));
                        std::process::exit(1);
                    }
                };

                match outcome {
                    broadcast::lifecycle::BroadcastOutcome::Success(txid) => {
                        out["status"] = serde_json::json!("broadcast_success");
                        out["broadcasted"] = serde_json::json!(true);
                        out["broadcast_txid"] = serde_json::json!(txid);
                        out["explorer_url"] = serde_json::json!(format!("https://mempool.space/signet/tx/{}", txid));
                    }
                    broadcast::lifecycle::BroadcastOutcome::Failed(err) => {
                        out["status"] = serde_json::json!("broadcast_failed");
                        out["broadcast_error"] = serde_json::json!(err);
                    }
                    broadcast::lifecycle::BroadcastOutcome::Retry => {
                        out["status"] = serde_json::json!("broadcast_retry_needed");
                    }
                }
            }

            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }

        "btc-status" => {
            let txid = get_arg("--txid").expect("--txid <txid> is required");
            let esplora_url = "https://mempool.space/signet/api";
            let client = reqwest::Client::new();
            let resp = client.get(format!("{}/tx/{}/status", esplora_url, txid)).send().await;
            match resp {
                Ok(r) => {
                    let json: serde_json::Value = r.json().await.unwrap_or_default();
                    let out = serde_json::json!({
                        "txid": txid,
                        "status": json,
                        "explorer_url": format!("https://mempool.space/signet/tx/{}", txid),
                    });
                    println!("{}", serde_json::to_string_pretty(&out).unwrap());
                }
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("Network request failed: {}", e)}));
                    std::process::exit(1);
                }
            }
        }

        "lifecycle-list" => {
            let l_path = lifecycle_file();
            if !l_path.exists() {
                println!("{}", serde_json::json!({"records": []}));
                return;
            }
            match TxLifecycleCoordinator::load_from_file(&l_path) {
                Ok(coord) => {
                    let records = coord.export_records();
                    println!("{}", serde_json::to_string_pretty(&records).unwrap());
                }
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("Failed to read lifecycle file: {}", e)}));
                    std::process::exit(1);
                }
            }
        }

        other => {
            eprintln!("Unknown subcommand: {}", other);
            std::process::exit(1);
        }
    }
}
