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

const BSC_TESTNET_USDT: &str = "0x337610d27c682E347C9cD60BD4b3b107C9d34dDd";
const BSC_TESTNET_RPC: &str = "https://data-seed-prebsc-1-s1.binance.org:8545";

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

fn get_password(
    get_arg: impl Fn(&str) -> Option<String>,
    has_flag: impl Fn(&str) -> bool,
) -> Result<String, String> {
    if has_flag("--password-stdin") {
        let mut buffer = String::new();
        std::io::stdin()
            .read_line(&mut buffer)
            .map_err(|e| format!("Failed to read password from stdin: {}", e))?;
        let trimmed = buffer.trim_end_matches(&['\r', '\n'][..]);
        if trimmed.is_empty() {
            return Err("Password read from stdin was empty".to_string());
        }
        return Ok(trimmed.to_string());
    }

    if let Ok(env_pw) = std::env::var("VAULT_PASSWORD") {
        if !env_pw.is_empty() {
            return Ok(env_pw);
        }
    }

    if let Some(cli_pw) = get_arg("--password") {
        eprintln!(
            "[SECURITY WARNING] Passing master password via --password CLI argument can leak through process table and shell history. Prefer VAULT_PASSWORD environment variable or --password-stdin."
        );
        return Ok(cli_pw);
    }

    Err(
        "Master password required. Provide via VAULT_PASSWORD environment variable, --password-stdin, or --password <pw>."
            .to_string(),
    )
}

fn load_coordinator_fail_closed(l_path: &PathBuf) -> Result<TxLifecycleCoordinator, String> {
    if l_path.exists() {
        TxLifecycleCoordinator::load_from_file(l_path).map_err(|e| {
            format!(
                "FAIL-CLOSED: Persistent coordinator file exists at {:?} but could not be loaded: {}. Halting execution to prevent transaction history loss or corruption.",
                l_path, e
            )
        })
    } else {
        let mut c = TxLifecycleCoordinator::new();
        c.set_storage_path(l_path.clone());
        Ok(c)
    }
}

async fn bsc_rpc_call(
    client: &reqwest::Client,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1,
    });
    let resp = client
        .post(BSC_TESTNET_RPC)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Network request failed for {}: {}", method, e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} error for RPC {}", resp.status(), method));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON response for {}: {}", method, e))?;

    if let Some(err) = json.get("error") {
        return Err(format!("RPC error returned by {}: {}", method, err));
    }

    match json.get("result") {
        Some(res) if !res.is_null() => Ok(res.clone()),
        _ => Err(format!("Missing or null 'result' field in RPC response for {}", method)),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: agent_cli <subcommand> [options]\n\
             Subcommands:\n  \
               status [--password <pw> | --password-stdin]\n  \
               btc-balance [--address <addr>] [--all]\n  \
               btc-send --to <addr> --amount <sats> [--fee-rate <rate>] [--broadcast]\n  \
               btc-status --txid <txid>\n  \
               bsc-balance [--address <addr>]\n  \
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

            if let Ok(pw) = get_password(&get_arg, &has_flag) {
                match read_mnemonic(&pw) {
                    Ok(mnemonic) => {
                        let signet_0 = bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 0, 0).unwrap_or_default();
                        let signet_1 = bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 0, 1).unwrap_or_default();
                        let signet_change_0 = bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 1, 0).unwrap_or_default();
                        let bsc_0 = bip::derive_bsc_address_at(&mnemonic, 0).unwrap_or_default();
                        let bsc_1 = bip::derive_bsc_address_at(&mnemonic, 1).unwrap_or_default();

                        out["signet_receive_0"] = serde_json::json!(signet_0);
                        out["signet_receive_1"] = serde_json::json!(signet_1);
                        out["signet_change_0"] = serde_json::json!(signet_change_0);
                        out["bsc_address_0"] = serde_json::json!(bsc_0);
                        out["bsc_address_1"] = serde_json::json!(bsc_1);
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
            let client = reqwest::Client::new();
            let esplora_url = "https://mempool.space/signet/api";

            let addresses = if let Some(a) = get_arg("--address") {
                vec![a]
            } else if has_flag("--known-addresses") || has_flag("--all") {
                let pw = match get_password(&get_arg, &has_flag) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": e}));
                        std::process::exit(1);
                    }
                };
                let mnemonic = match read_mnemonic(&pw) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": e}));
                        std::process::exit(1);
                    }
                };
                vec![
                    bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 0, 0).unwrap(),
                    bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 0, 1).unwrap(),
                    bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 1, 0).unwrap(),
                ]
            } else {
                let pw = match get_password(&get_arg, &has_flag) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": e}));
                        std::process::exit(1);
                    }
                };
                let mnemonic = match read_mnemonic(&pw) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": e}));
                        std::process::exit(1);
                    }
                };
                vec![bip::derive_btc_address_at(&mnemonic, bitcoin::Network::Signet, 0, 0).unwrap()]
            };

            let mut results = Vec::new();
            let mut grand_confirmed: i64 = 0;
            let mut grand_mempool: i64 = 0;
            let mut grand_utxos: usize = 0;

            for addr in addresses {
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

                        let count = utxos.as_array().map(|a| a.len()).unwrap_or(0);
                        grand_confirmed += confirmed_sats;
                        grand_mempool += mempool_delta_sats;
                        grand_utxos += count;

                        results.push(serde_json::json!({
                            "address": addr,
                            "confirmed_sats": confirmed_sats,
                            "mempool_delta_sats": mempool_delta_sats,
                            "total_sats": confirmed_sats + mempool_delta_sats,
                            "utxo_count": count,
                            "utxos": utxos,
                        }));
                    }
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": format!("Network request failed: {}", e)}));
                        std::process::exit(1);
                    }
                }
            }

            if results.len() == 1 {
                println!("{}", serde_json::to_string_pretty(&results[0]).unwrap());
            } else {
                let summary = serde_json::json!({
                    "addresses": results,
                    "grand_total": {
                        "confirmed_sats": grand_confirmed,
                        "mempool_delta_sats": grand_mempool,
                        "total_sats": grand_confirmed + grand_mempool,
                        "total_utxo_count": grand_utxos,
                    }
                });
                println!("{}", serde_json::to_string_pretty(&summary).unwrap());
            }
        }

        "btc-send" => {
            let to = get_arg("--to").expect("--to <recipient> is required");
            let amount_str = get_arg("--amount").expect("--amount <satoshis> is required");
            let amount_sats: u64 = amount_str.parse().expect("amount must be an integer");
            let fee_rate_str = get_arg("--fee-rate").unwrap_or_else(|| "2".to_string());
            let fee_rate: u64 = fee_rate_str.parse().expect("fee-rate must be an integer");
            let should_broadcast = has_flag("--broadcast");

            let pw = match get_password(&get_arg, &has_flag) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": e}));
                    std::process::exit(1);
                }
            };

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

            // 6. Fail-closed record into TxLifecycleCoordinator
            let l_path = lifecycle_file();
            let mut coord = match load_coordinator_fail_closed(&l_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": e}));
                    std::process::exit(1);
                }
            };

            if let Err(e) = coord.register_draft(&preview.draft_id, broadcast::types::BroadcastNetwork::BitcoinSignet, now_unix) {
                eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: coordinator.register_draft failed: {}", e)}));
                std::process::exit(1);
            }

            if let Err(e) = coord.record_signed_btc(
                &preview.draft_id,
                &signed.payload_hash,
                &signed.raw_tx_hex,
                &signed.txid,
                &signed.wtxid,
                approved_prevouts.clone(),
                now_unix,
            ) {
                eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: coordinator.record_signed_btc failed: {}", e)}));
                std::process::exit(1);
            }

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
                        println!("{}", serde_json::to_string_pretty(&out).unwrap());
                        std::process::exit(0);
                    }
                    broadcast::lifecycle::BroadcastOutcome::Failed(err) => {
                        out["status"] = serde_json::json!("broadcast_failed");
                        out["broadcast_error"] = serde_json::json!(err);
                        eprintln!("{}", serde_json::to_string_pretty(&out).unwrap());
                        std::process::exit(1);
                    }
                    broadcast::lifecycle::BroadcastOutcome::Retry => {
                        out["status"] = serde_json::json!("broadcast_retry_needed");
                        eprintln!("{}", serde_json::to_string_pretty(&out).unwrap());
                        std::process::exit(2);
                    }
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            }
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

        "bsc-balance" => {
            let addr = if let Some(a) = get_arg("--address") {
                a
            } else {
                let pw = match get_password(&get_arg, &has_flag) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": e}));
                        std::process::exit(1);
                    }
                };
                let mnemonic = match read_mnemonic(&pw) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("{}", serde_json::json!({"error": e}));
                        std::process::exit(1);
                    }
                };
                bip::derive_bsc_address_at(&mnemonic, 0).unwrap()
            };

            let client = reqwest::Client::new();

            // 1. eth_chainId
            let chain_id_val = match bsc_rpc_call(&client, "eth_chainId", serde_json::json!([])).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: RPC eth_chainId failed: {}", e)}));
                    std::process::exit(1);
                }
            };
            let chain_id_hex = chain_id_val.as_str().unwrap_or_default();
            let chain_id = match u64::from_str_radix(chain_id_hex.trim_start_matches("0x"), 16) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: Invalid eth_chainId hex '{}': {}", chain_id_hex, e)}));
                    std::process::exit(1);
                }
            };
            if chain_id != 97 {
                eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: Connected RPC returned chain ID {} instead of BSC Testnet (97)", chain_id)}));
                std::process::exit(1);
            }

            // 2. eth_getBalance (tBNB)
            let balance_val = match bsc_rpc_call(&client, "eth_getBalance", serde_json::json!([&addr, "latest"])).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: RPC eth_getBalance failed: {}", e)}));
                    std::process::exit(1);
                }
            };
            let tbnb_wei_hex = balance_val.as_str().unwrap_or_default();
            let tbnb_wei = match u128::from_str_radix(tbnb_wei_hex.trim_start_matches("0x"), 16) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: Invalid balance hex '{}': {}", tbnb_wei_hex, e)}));
                    std::process::exit(1);
                }
            };

            // 3. eth_getTransactionCount (nonce)
            let nonce_val = match bsc_rpc_call(&client, "eth_getTransactionCount", serde_json::json!([&addr, "latest"])).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: RPC eth_getTransactionCount failed: {}", e)}));
                    std::process::exit(1);
                }
            };
            let nonce_hex = nonce_val.as_str().unwrap_or_default();
            let nonce = match u64::from_str_radix(nonce_hex.trim_start_matches("0x"), 16) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: Invalid nonce hex '{}': {}", nonce_hex, e)}));
                    std::process::exit(1);
                }
            };

            // 4. eth_getCode (USDT contract bytecode check)
            let code_val = match bsc_rpc_call(&client, "eth_getCode", serde_json::json!([BSC_TESTNET_USDT, "latest"])).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: RPC eth_getCode failed: {}", e)}));
                    std::process::exit(1);
                }
            };
            let code_hex = code_val.as_str().unwrap_or_default();
            if code_hex.len() <= 10 || code_hex == "0x" {
                eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: No contract bytecode found at USDT address {}", BSC_TESTNET_USDT)}));
                std::process::exit(1);
            }

            // 5. decimals() of USDT: 0x313ce567
            let decimals_val = match bsc_rpc_call(&client, "eth_call", serde_json::json!([{
                "to": BSC_TESTNET_USDT,
                "data": "0x313ce567"
            }, "latest"])).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: RPC decimals() call failed: {}", e)}));
                    std::process::exit(1);
                }
            };
            let decimals_hex = decimals_val.as_str().unwrap_or_default();
            let decimals = match u32::from_str_radix(decimals_hex.trim_start_matches("0x"), 16) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: Invalid decimals hex '{}': {}", decimals_hex, e)}));
                    std::process::exit(1);
                }
            };
            if decimals != 18 {
                eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: USDT contract decimals returned {} instead of expected 18", decimals)}));
                std::process::exit(1);
            }

            // 6. balanceOf(address) of USDT: 0x70a08231 + 32-byte padded address
            let clean_addr = addr.trim_start_matches("0x");
            let call_data = format!("0x70a08231000000000000000000000000{}", clean_addr);
            let token_val = match bsc_rpc_call(&client, "eth_call", serde_json::json!([{
                "to": BSC_TESTNET_USDT,
                "data": call_data
            }, "latest"])).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: RPC balanceOf() call failed: {}", e)}));
                    std::process::exit(1);
                }
            };
            let usdt_units_hex = token_val.as_str().unwrap_or_default();
            let usdt_units = match u128::from_str_radix(usdt_units_hex.trim_start_matches("0x"), 16) {
                Ok(u) => u,
                Err(e) => {
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: Invalid balanceOf hex '{}': {}", usdt_units_hex, e)}));
                    std::process::exit(1);
                }
            };

            let whole_tbnb = tbnb_wei / 1_000_000_000_000_000_000;
            let frac_tbnb = tbnb_wei % 1_000_000_000_000_000_000;
            let whole_usdt = usdt_units / 1_000_000_000_000_000_000;
            let frac_usdt = usdt_units % 1_000_000_000_000_000_000;

            let out = serde_json::json!({
                "address": addr,
                "chain_id": chain_id,
                "chain_id_hex": chain_id_hex,
                "nonce": nonce,
                "tbnb_wei": tbnb_wei.to_string(),
                "tbnb_formatted": format!("{}.{:018}", whole_tbnb, frac_tbnb),
                "usdt_contract": BSC_TESTNET_USDT,
                "usdt_contract_verified": true,
                "usdt_decimals": decimals,
                "usdt_raw_units": usdt_units.to_string(),
                "usdt_formatted": format!("{}.{:018}", whole_usdt, frac_usdt),
                "explorer_url": format!("https://testnet.bscscan.com/address/{}", addr),
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
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
                    eprintln!("{}", serde_json::json!({"error": format!("FAIL-CLOSED: Failed to read lifecycle file: {}", e)}));
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
