pub mod bsc;
pub mod btc;
pub mod lifecycle;
pub mod types;

#[cfg(test)]
mod tests {
    use super::{
        bsc::{BscBroadcastError, BscBroadcaster, BscReceiptStatus, MockBscBroadcaster, RpcBscBroadcaster},
        btc::{BtcBroadcastError, BtcBroadcaster, BtcTxQueryStatus, EsploraBtcBroadcaster, MockBtcBroadcaster},
        lifecycle::TxLifecycleCoordinator,
        types::{BroadcastNetwork, TxLifecycleStatus},
    };
    use crate::wallet::bip;
    use bitcoin::hashes::Hash;
    use std::str::FromStr;

    #[test]
    fn test_mainnet_broadcast_strictly_forbidden() {
        assert!(BroadcastNetwork::assert_testnet_only("bitcoin_mainnet").is_err());
        assert!(BroadcastNetwork::assert_testnet_only("mainnet").is_err());
        assert!(BroadcastNetwork::assert_testnet_only("bsc_mainnet").is_err());
        assert!(BroadcastNetwork::assert_testnet_only("56").is_err());

        assert_eq!(
            BroadcastNetwork::assert_testnet_only("bitcoin_signet").unwrap(),
            BroadcastNetwork::BitcoinSignet
        );
        assert_eq!(
            BroadcastNetwork::assert_testnet_only("bsc_testnet").unwrap(),
            BroadcastNetwork::BscTestnet
        );
    }

    #[test]
    fn test_btc_lifecycle_happy_path() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_01";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();

        coordinator
            .record_signed(draft_id, "mock_hash", "02000000000101deadbeef", 1005)
            .unwrap();

        let txid = coordinator
            .execute_broadcast_btc(draft_id, &broadcaster, 3, 1010)
            .unwrap();
        assert!(txid.starts_with("mock_btc_txid_"));

        let rec = coordinator.get_record(draft_id).unwrap();
        assert!(matches!(rec.status, TxLifecycleStatus::Pending { .. }));

        // Now simulate block confirmation
        broadcaster.set_tx_status(
            &txid,
            BtcTxQueryStatus::Confirmed {
                block_height: 180000,
                block_hash: "0000000000000001abcd".to_string(),
                confirmations: 1,
            },
        );

        let polled = coordinator.poll_btc_lifecycle(draft_id, &broadcaster, 1020).unwrap();
        match polled {
            TxLifecycleStatus::Confirmed {
                block_height,
                confirmations,
                ..
            } => {
                assert_eq!(block_height, 180000);
                assert_eq!(confirmations, 1);
            }
            other => panic!("Expected Confirmed, got {:?}", other),
        }
    }

    #[test]
    fn test_btc_broadcast_double_spend_rejection() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_double_spend";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();
        coordinator
            .record_signed(draft_id, "mock_hash", "02000000000101badinputs", 1005)
            .unwrap();

        // Configure broadcaster to reject with DoubleSpend
        broadcaster.set_next_broadcast_result(Err(BtcBroadcastError::DoubleSpend {
            message: "txn-mempool-conflict: input already spent".to_string(),
        }));

        let err = coordinator
            .execute_broadcast_btc(draft_id, &broadcaster, 3, 1010)
            .unwrap_err();
        assert!(err.contains("Double spend rejected"));

        let rec = coordinator.get_record(draft_id).unwrap();
        match &rec.status {
            TxLifecycleStatus::Failed { stage, permanent, error_message } => {
                assert_eq!(stage, "broadcast");
                assert!(*permanent, "Double spend must be permanent failure");
                assert!(error_message.contains("Double spend"));
            }
            other => panic!("Expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn test_btc_broadcast_timeout_retry_then_success() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_retry";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();
        coordinator
            .record_signed(draft_id, "mock_hash", "02000000000101retrytx", 1005)
            .unwrap();

        // First attempt returns Timeout, subsequent succeeds
        broadcaster.set_next_broadcast_result(Err(BtcBroadcastError::Timeout(
            "Node response timed out".to_string(),
        )));

        let txid = coordinator
            .execute_broadcast_btc(draft_id, &broadcaster, 3, 1010)
            .unwrap();
        assert!(txid.starts_with("mock_btc_txid_"));

        let rec = coordinator.get_record(draft_id).unwrap();
        assert_eq!(rec.retry_count, 2, "Expected 2 broadcast attempts");
        assert!(matches!(rec.status, TxLifecycleStatus::Pending { .. }));
    }

    #[test]
    fn test_btc_lifecycle_rbf_replacement_detection() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_rbf";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();
        coordinator
            .record_signed(draft_id, "mock_hash", "02000000000101rbftx", 1005)
            .unwrap();

        let txid = coordinator
            .execute_broadcast_btc(draft_id, &broadcaster, 3, 1010)
            .unwrap();

        // Node reports tx was replaced by a higher fee tx
        broadcaster.set_tx_status(
            &txid,
            BtcTxQueryStatus::Replaced {
                replacing_txid: "replacing_txid_9999".to_string(),
            },
        );

        let polled = coordinator.poll_btc_lifecycle(draft_id, &broadcaster, 1020).unwrap();
        match polled {
            TxLifecycleStatus::Replaced {
                old_tx_hash,
                new_tx_hash,
                reason,
            } => {
                assert_eq!(old_tx_hash, txid);
                assert_eq!(new_tx_hash, "replacing_txid_9999");
                assert!(reason.contains("RBF"));
            }
            other => panic!("Expected Replaced, got {:?}", other),
        }
    }

    #[test]
    fn test_btc_lifecycle_reorg_detection() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_reorg";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();
        coordinator
            .record_signed(draft_id, "mock_hash", "02000000000101reorgtx", 1005)
            .unwrap();

        let txid = coordinator
            .execute_broadcast_btc(draft_id, &broadcaster, 3, 1010)
            .unwrap();

        // 1. Confirmed in block 100 with hash A
        broadcaster.set_tx_status(
            &txid,
            BtcTxQueryStatus::Confirmed {
                block_height: 100,
                block_hash: "block_hash_AAAA".to_string(),
                confirmations: 1,
            },
        );
        coordinator.poll_btc_lifecycle(draft_id, &broadcaster, 1020).unwrap();

        // 2. Reorg: same height now has block hash B
        broadcaster.set_tx_status(
            &txid,
            BtcTxQueryStatus::Confirmed {
                block_height: 100,
                block_hash: "block_hash_BBBB_fork".to_string(),
                confirmations: 1,
            },
        );

        let polled = coordinator.poll_btc_lifecycle(draft_id, &broadcaster, 1030).unwrap();
        match polled {
            TxLifecycleStatus::ReorgDetected {
                previous_block_height,
                previous_block_hash,
                ..
            } => {
                assert_eq!(previous_block_height, 100);
                assert_eq!(previous_block_hash, "block_hash_AAAA");
            }
            other => panic!("Expected ReorgDetected, got {:?}", other),
        }
    }

    #[test]
    fn test_bsc_lifecycle_happy_path() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_01";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed(draft_id, "mock_hash", "0xf86c018504a817c80082520894...", 1005)
            .unwrap();

        let tx_hash = coordinator
            .execute_broadcast_bsc(draft_id, &broadcaster, 3, 1010)
            .unwrap();
        assert!(tx_hash.starts_with("0x"));

        // Simulate mined receipt (status 0x1)
        broadcaster.set_receipt(
            &tx_hash,
            BscReceiptStatus::Success {
                block_number: 35000000,
                block_hash: "0xblock_hash_bsc_123".to_string(),
                gas_used: 21000,
            },
        );

        let polled = coordinator.poll_bsc_lifecycle(draft_id, &broadcaster, 1020).unwrap();
        match polled {
            TxLifecycleStatus::Confirmed {
                block_height,
                block_hash,
                ..
            } => {
                assert_eq!(block_height, 35000000);
                assert_eq!(block_hash, "0xblock_hash_bsc_123");
            }
            other => panic!("Expected Confirmed, got {:?}", other),
        }
    }

    #[test]
    fn test_bsc_broadcast_nonce_too_low_conflict() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_nonce_err";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed(draft_id, "mock_hash", "0xf86c...", 1005)
            .unwrap();

        broadcaster.set_next_broadcast_result(Err(BscBroadcastError::NonceTooLow {
            message: "nonce too low: next nonce 5, tx nonce 4".to_string(),
        }));

        let err = coordinator
            .execute_broadcast_bsc(draft_id, &broadcaster, 3, 1010)
            .unwrap_err();
        assert!(err.contains("Nonce too low"));

        let rec = coordinator.get_record(draft_id).unwrap();
        match &rec.status {
            TxLifecycleStatus::Failed { stage, permanent, error_message } => {
                assert_eq!(stage, "broadcast");
                assert!(*permanent, "Nonce conflict is a permanent broadcast error");
                assert!(error_message.contains("Nonce too low"));
            }
            other => panic!("Expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn test_bsc_broadcast_replacement_underpriced() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_underpriced";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed(draft_id, "mock_hash", "0xf86c...", 1005)
            .unwrap();

        broadcaster.set_next_broadcast_result(Err(BscBroadcastError::ReplacementUnderpriced {
            message: "replacement transaction underpriced: gas price must be >= 10% higher".to_string(),
        }));

        let err = coordinator
            .execute_broadcast_bsc(draft_id, &broadcaster, 3, 1010)
            .unwrap_err();
        assert!(err.contains("Replacement underpriced"));

        let rec = coordinator.get_record(draft_id).unwrap();
        assert!(matches!(rec.status, TxLifecycleStatus::Failed { permanent: true, .. }));
    }

    #[test]
    fn test_bsc_lifecycle_execution_reverted_status_0x0() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_revert";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed(draft_id, "mock_hash", "0xf86c...", 1005)
            .unwrap();

        let tx_hash = coordinator
            .execute_broadcast_bsc(draft_id, &broadcaster, 3, 1010)
            .unwrap();

        // Simulate execution reverted on-chain (status 0x0)
        broadcaster.set_receipt(
            &tx_hash,
            BscReceiptStatus::Reverted {
                block_number: 35000005,
                block_hash: "0xblock_revert".to_string(),
                reason: "BEP20: transfer amount exceeds balance".to_string(),
            },
        );

        let polled = coordinator.poll_bsc_lifecycle(draft_id, &broadcaster, 1020).unwrap();
        match polled {
            TxLifecycleStatus::Failed { stage, permanent, error_message } => {
                assert_eq!(stage, "execution");
                assert!(permanent);
                assert!(error_message.contains("EVM execution reverted (status 0x0)"));
                assert!(error_message.contains("transfer amount exceeds balance"));
            }
            other => panic!("Expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn test_bsc_lifecycle_reorg_detection() {
        let mut coordinator = TxLifecycleCoordinator::new();
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_reorg";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed(draft_id, "mock_hash", "0xf86c...", 1005)
            .unwrap();

        let tx_hash = coordinator
            .execute_broadcast_bsc(draft_id, &broadcaster, 3, 1010)
            .unwrap();

        // Mined at block 35000000 with hash A
        broadcaster.set_receipt(
            &tx_hash,
            BscReceiptStatus::Success {
                block_number: 35000000,
                block_hash: "0xblock_canonical_A".to_string(),
                gas_used: 21000,
            },
        );
        coordinator.poll_bsc_lifecycle(draft_id, &broadcaster, 1020).unwrap();

        // Fork occurred, block 35000000 now has hash B
        broadcaster.set_receipt(
            &tx_hash,
            BscReceiptStatus::Success {
                block_number: 35000000,
                block_hash: "0xblock_fork_B".to_string(),
                gas_used: 21000,
            },
        );

        let polled = coordinator.poll_bsc_lifecycle(draft_id, &broadcaster, 1030).unwrap();
        match polled {
            TxLifecycleStatus::ReorgDetected {
                previous_block_height,
                previous_block_hash,
                ..
            } => {
                assert_eq!(previous_block_height, 35000000);
                assert_eq!(previous_block_hash, "0xblock_canonical_A");
            }
            other => panic!("Expected ReorgDetected, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_esplora_and_bsc_errors() {
        use super::{bsc::parse_bsc_rpc_error, btc::parse_esplora_error};

        // Esplora double-spend
        let err = parse_esplora_error(400, "sendrawtransaction RPC error: txn-mempool-conflict");
        assert!(matches!(err, BtcBroadcastError::DoubleSpend { .. }));

        // Esplora fee too low
        let err = parse_esplora_error(400, "sendrawtransaction RPC error: min relay fee not met");
        assert!(matches!(err, BtcBroadcastError::FeeTooLow { .. }));

        // Esplora HTTP 500
        let err = parse_esplora_error(500, "Internal Server Error");
        assert!(matches!(err, BtcBroadcastError::HttpError { status_code: 500, .. }));

        // BSC Nonce too low
        let err = parse_bsc_rpc_error(-32000, "nonce too low");
        assert!(matches!(err, BscBroadcastError::NonceTooLow { .. }));

        // BSC Replacement underpriced
        let err = parse_bsc_rpc_error(-32000, "replacement transaction underpriced");
        assert!(matches!(err, BscBroadcastError::ReplacementUnderpriced { .. }));

        // BSC Insufficient funds
        let err = parse_bsc_rpc_error(-32000, "insufficient funds for gas * price + value");
        assert!(matches!(err, BscBroadcastError::InsufficientFunds { .. }));
    }

    #[test]
    fn test_lifecycle_coordinator_persistence_across_restart() {
        let temp_dir = tempfile::tempdir().unwrap();
        let persist_file = temp_dir.path().join("lifecycle_records.json");

        // 1. First session: coordinator records transactions and confirms one
        let mut coordinator1 = TxLifecycleCoordinator::new();
        let btc_broadcaster = MockBtcBroadcaster::new();
        let bsc_broadcaster = MockBscBroadcaster::new();

        let btc_draft = "draft_btc_persist_01";
        coordinator1.register_draft(btc_draft, BroadcastNetwork::BitcoinSignet, 1000).unwrap();
        coordinator1.record_signed(btc_draft, "hash_btc_1", "0200000001dead", 1005).unwrap();
        let btc_txid = coordinator1.execute_broadcast_btc(btc_draft, &btc_broadcaster, 3, 1010).unwrap();

        btc_broadcaster.set_tx_status(&btc_txid, BtcTxQueryStatus::Confirmed {
            block_height: 150_000,
            block_hash: "00000000block_signet_abc".to_string(),
            confirmations: 2,
        });
        coordinator1.poll_btc_lifecycle(btc_draft, &btc_broadcaster, 1020).unwrap();

        let bsc_draft = "draft_bsc_persist_01";
        coordinator1.register_draft(bsc_draft, BroadcastNetwork::BscTestnet, 1030).unwrap();
        coordinator1.record_signed(bsc_draft, "hash_bsc_1", "f86c...", 1035).unwrap();
        let _bsc_tx_hash = coordinator1.execute_broadcast_bsc(bsc_draft, &bsc_broadcaster, 3, 1040).unwrap();

        // Save coordinator 1 state to disk
        coordinator1.save_to_file(&persist_file).expect("Must save lifecycle records to disk");

        // 2. Simulate complete application crash or restart by dropping coordinator1
        drop(coordinator1);

        // 3. Second session: reload coordinator from disk into a fresh instance
        let mut coordinator2 = TxLifecycleCoordinator::load_from_file(&persist_file)
            .expect("Must load lifecycle records from disk");

        // Verify both transactions were restored with accurate state
        let btc_record = coordinator2.get_record(btc_draft).expect("BTC record must be restored");
        assert!(matches!(btc_record.status, TxLifecycleStatus::Confirmed { block_height: 150_000, .. }));

        let bsc_record = coordinator2.get_record(bsc_draft).expect("BSC record must be restored");
        assert!(matches!(bsc_record.status, TxLifecycleStatus::Pending { .. }));

        // 4. Continue lifecycle tracking seamlessly on reloaded coordinator
        bsc_broadcaster.set_receipt(&_bsc_tx_hash, BscReceiptStatus::Success {
            block_number: 35_000_100,
            block_hash: "0xblock_bsc_canonical_1".to_string(),
            gas_used: 65_000,
        });
        let polled_bsc = coordinator2.poll_bsc_lifecycle(bsc_draft, &bsc_broadcaster, 1050).unwrap();
        assert!(matches!(polled_bsc, TxLifecycleStatus::Confirmed { block_height: 35_000_100, .. }));

        // Save again and reload to verify second transition persisted
        coordinator2.save_to_file(&persist_file).expect("Must re-save");
        let coordinator3 = TxLifecycleCoordinator::load_from_file(&persist_file).unwrap();
        let bsc_record_v3 = coordinator3.get_record(bsc_draft).unwrap();
        assert!(matches!(bsc_record_v3.status, TxLifecycleStatus::Confirmed { block_height: 35_000_100, .. }));
    }

    #[test]
    fn test_adapters_reject_wrong_network_before_network_io() {
        // BTC adapter MUST reject BscTestnet or Mainnet networks before attempting any network call
        let btc_adapter = EsploraBtcBroadcaster::new(false);
        let btc_res = btc_adapter.broadcast("02000000000101", BroadcastNetwork::BscTestnet);
        assert!(matches!(btc_res, Err(BtcBroadcastError::NetworkForbidden(_))));

        let btc_query_res = btc_adapter.query_status("txid", BroadcastNetwork::BscTestnet);
        assert!(matches!(btc_query_res, Err(BtcBroadcastError::NetworkForbidden(_))));

        // BSC adapter MUST reject BitcoinSignet or Mainnet networks before attempting any network call
        let bsc_adapter = RpcBscBroadcaster::new(false);
        let bsc_res = bsc_adapter.broadcast("0xf86c...", BroadcastNetwork::BitcoinSignet);
        assert!(matches!(bsc_res, Err(BscBroadcastError::NetworkForbidden(_))));

        let bsc_query_res = bsc_adapter.query_receipt("0x1234", BroadcastNetwork::BitcoinSignet);
        assert!(matches!(bsc_query_res, Err(BscBroadcastError::NetworkForbidden(_))));
    }

    #[test]
    fn test_adapters_dry_run_calculate_hashes_accurately_without_post() {
        // BTC dry-run verification with validly structured signet raw tx
        let btc_adapter = EsploraBtcBroadcaster::new(false);
        let invalid_raw = btc_adapter.broadcast("deadbeef", BroadcastNetwork::BitcoinSignet);
        assert!(matches!(invalid_raw, Err(BtcBroadcastError::NodeRejected(_))));

        // BSC dry-run verification with valid BSC Testnet (Chain ID 97) transaction
        let bsc_adapter = RpcBscBroadcaster::new(false);

        let secp = secp256k1::Secp256k1::new();
        let privkey = secp256k1::SecretKey::from_slice(&[1u8; 32]).unwrap();
        let calldata = crate::wallet::tx::bsc::calldata::encode_erc20_transfer(
            "0x9858EfFD232B4033E47d90003D41EC34EcaEda94",
            &num_bigint::BigUint::from(1000u32),
        ).unwrap();
        let tx = crate::wallet::signer::bsc::Eip155Transaction {
            nonce: 1,
            gas_price: num_bigint::BigUint::from(3_000_000_000u64),
            gas_limit: 65_000,
            to: crate::wallet::blockchain::BSC_TESTNET_USDT_CONTRACT.to_string(),
            value: num_bigint::BigUint::from(0u32),
            data: hex::decode(calldata.trim_start_matches("0x")).unwrap(),
            chain_id: 97,
        };
        let signed = tx.sign(&secp, &privkey).unwrap();
        let raw_bsc_testnet_hex = hex::encode(&signed.raw_signed_tx_bytes);

        let dry_run_hash = bsc_adapter.broadcast(&raw_bsc_testnet_hex, BroadcastNetwork::BscTestnet).unwrap();
        assert!(dry_run_hash.starts_with("dry_run_0x"));
    }

    #[test]
    fn test_g5_t1_bsc_adapter_rejects_chain1_and_chain56_eip155() {
        let bsc_adapter = RpcBscBroadcaster::new(false);
        let secp = secp256k1::Secp256k1::new();
        let privkey = secp256k1::SecretKey::from_slice(&[1u8; 32]).unwrap();
        let calldata = crate::wallet::tx::bsc::calldata::encode_erc20_transfer(
            "0x9858EfFD232B4033E47d90003D41EC34EcaEda94",
            &num_bigint::BigUint::from(1000u32),
        ).unwrap();

        // 1. Transaction signed for Ethereum Mainnet (Chain ID 1, v = 37)
        let tx_eth_mainnet = crate::wallet::signer::bsc::Eip155Transaction {
            nonce: 1,
            gas_price: num_bigint::BigUint::from(20_000_000_000u64),
            gas_limit: 65_000,
            to: crate::wallet::blockchain::BSC_TESTNET_USDT_CONTRACT.to_string(),
            value: num_bigint::BigUint::from(0u32),
            data: hex::decode(calldata.trim_start_matches("0x")).unwrap(),
            chain_id: 1, // Mainnet
        };
        let signed_eth = tx_eth_mainnet.sign(&secp, &privkey).unwrap();
        let raw_eth_hex = hex::encode(&signed_eth.raw_signed_tx_bytes);

        let res_eth = bsc_adapter.dry_run_verify(&raw_eth_hex, BroadcastNetwork::BscTestnet);
        assert!(res_eth.is_err(), "Adapter MUST reject Chain ID 1 raw transaction");
        let err_eth = res_eth.unwrap_err().to_string();
        assert!(err_eth.contains("chain ID mismatch"), "Error must report chain ID mismatch: {}", err_eth);

        // 2. Transaction signed for BSC Mainnet (Chain ID 56)
        let tx_bsc_mainnet = crate::wallet::signer::bsc::Eip155Transaction {
            chain_id: 56, // BSC Mainnet
            ..tx_eth_mainnet
        };
        let signed_bsc_mainnet = tx_bsc_mainnet.sign(&secp, &privkey).unwrap();
        let raw_bsc_mainnet_hex = hex::encode(&signed_bsc_mainnet.raw_signed_tx_bytes);

        let res_bsc_mainnet = bsc_adapter.dry_run_verify(&raw_bsc_mainnet_hex, BroadcastNetwork::BscTestnet);
        assert!(res_bsc_mainnet.is_err(), "Adapter MUST reject BSC Mainnet Chain ID 56 transaction");
        let err_bsc = res_bsc_mainnet.unwrap_err().to_string();
        assert!(err_bsc.contains("chain ID mismatch"), "Error must report chain ID mismatch: {}", err_bsc);
    }

    #[test]
    fn test_g5_t4_lifecycle_crash_between_states_retains_raw_tx_and_hash() {
        let temp_dir = tempfile::tempdir().unwrap();
        let persist_file = temp_dir.path().join("crash_recovery_records.json");

        let mut coordinator1 = TxLifecycleCoordinator::new();
        let draft_id = "draft_crash_test_01";
        coordinator1.register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000).unwrap();

        let raw_tx = "0200000001deadbeef0000000000";
        let payload_hash = "0xhash1234";
        coordinator1.record_signed(draft_id, payload_hash, raw_tx, 1005).unwrap();

        // Transition into Broadcasting
        let broadcaster = MockBtcBroadcaster::new();
        // Set mock to fail once with timeout
        broadcaster.next_broadcast_result.lock().unwrap().replace(Err(BtcBroadcastError::Timeout("Connection timeout".into())));
        let broadcast_res = coordinator1.execute_broadcast_btc(draft_id, &broadcaster, 1, 1010);
        assert!(broadcast_res.is_err());

        // Check that raw_tx_hex was NOT lost during transition into Broadcasting
        let rec = coordinator1.get_record(draft_id).unwrap();
        assert_eq!(rec.raw_tx_hex.as_deref(), Some(raw_tx));
        assert_eq!(rec.payload_hash.as_deref(), Some(payload_hash));

        // Save state, simulate crash by dropping coordinator
        coordinator1.save_to_file(&persist_file).unwrap();
        drop(coordinator1);

        // Reload coordinator from disk
        let coordinator2 = TxLifecycleCoordinator::load_from_file(&persist_file).unwrap();
        let restored = coordinator2.get_record(draft_id).unwrap();
        assert_eq!(restored.raw_tx_hex.as_deref(), Some(raw_tx));
        assert_eq!(restored.payload_hash.as_deref(), Some(payload_hash));
    }

    #[test]
    fn test_g5_t7_btc_rejects_raw_tx_different_from_approved_draft() {
        let btc_adapter = EsploraBtcBroadcaster::new(false);
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let privkey = bip::derive_bip84_privkey_at(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            bitcoin::Network::Signet,
            0,
            0,
        ).unwrap();
        let script = bitcoin::ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::hash(&privkey.public_key(&secp).serialize()));

        let outpoint = bitcoin::OutPoint {
            txid: bitcoin::Txid::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
            vout: 0,
        };

        let tx = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: outpoint,
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(45_000),
                script_pubkey: script,
            }],
        };

        let raw_hex = hex::encode(bitcoin::consensus::serialize(&tx));
        let actual_txid = tx.compute_txid().to_string();

        // 1. With exact matching txid -> succeeds in dry run
        let valid_res = btc_adapter.broadcast_verified(&raw_hex, &actual_txid, BroadcastNetwork::BitcoinSignet);
        assert!(valid_res.is_ok());

        // 2. With altered / unexpected txid -> REJECTED
        let forged_txid = "000000000000000000000000000000000000000000000000000000000000dead";
        let invalid_res = btc_adapter.broadcast_verified(&raw_hex, forged_txid, BroadcastNetwork::BitcoinSignet);
        assert!(invalid_res.is_err(), "Must reject raw tx if txid does not match approved draft");
        assert!(invalid_res.unwrap_err().to_string().contains("does not match approved transaction txid"));
    }
}
