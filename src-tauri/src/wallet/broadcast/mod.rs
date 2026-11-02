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
        // BTC dry-run verification
        let btc_adapter = EsploraBtcBroadcaster::new(false);
        // Valid empty/dummy raw transaction deserialize failure is caught cleanly
        let invalid_raw = btc_adapter.broadcast("deadbeef", BroadcastNetwork::BitcoinSignet);
        assert!(matches!(invalid_raw, Err(BtcBroadcastError::NodeRejected(_))));

        // BSC dry-run verification
        let bsc_adapter = RpcBscBroadcaster::new(false);
        let raw_evm_tx = "f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";
        let dry_run_hash = bsc_adapter.broadcast(raw_evm_tx, BroadcastNetwork::BscTestnet).unwrap();
        assert!(dry_run_hash.starts_with("dry_run_0x"));
    }
}
