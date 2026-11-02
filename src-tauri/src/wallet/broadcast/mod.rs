pub mod bsc;
pub mod btc;
pub mod lifecycle;
pub mod types;

#[cfg(test)]
mod tests {
    use super::{
        bsc::{BscBroadcastError, BscReceiptStatus, MockBscBroadcaster},
        btc::{BtcBroadcastError, BtcTxQueryStatus, MockBtcBroadcaster},
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
}
