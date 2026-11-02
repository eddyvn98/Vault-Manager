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
    use tiny_keccak::Hasher;

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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("happy_path.json"));
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_01";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();

        coordinator
            .record_signed_btc(draft_id, "mock_hash", "02000000000101deadbeef", "mock_txid", "mock_wtxid", None, 1005)
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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("ds.json"));
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_double_spend";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();
        coordinator
            .record_signed_btc(draft_id, "mock_hash", "02000000000101badinputs", "mock_txid", "mock_wtxid", None, 1005)
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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("retry.json"));
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_retry";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();
        coordinator
            .record_signed_btc(draft_id, "mock_hash", "02000000000101retrytx", "mock_txid", "mock_wtxid", None, 1005)
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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("rbf.json"));
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_rbf";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();
        coordinator
            .record_signed_btc(draft_id, "mock_hash", "02000000000101rbftx", "mock_txid", "mock_wtxid", None, 1005)
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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("reorg.json"));
        let broadcaster = MockBtcBroadcaster::new();

        let draft_id = "draft_btc_reorg";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 1000)
            .unwrap();
        coordinator
            .record_signed_btc(draft_id, "mock_hash", "02000000000101reorgtx", "mock_txid", "mock_wtxid", None, 1005)
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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("bsc_happy.json"));
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_01";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed_bsc(
                draft_id,
                "mock_hash",
                "0xf86c018504a817c80082520894...",
                "0x1111111111111111111111111111111111111111",
                "0x2222222222222222222222222222222222222222",
                "1000",
                None,
                None,
                None,
                1005,
            )
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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("bsc_nonce.json"));
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_nonce_err";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed_bsc(
                draft_id,
                "mock_hash",
                "0xf86c...",
                "0x1111111111111111111111111111111111111111",
                "0x2222222222222222222222222222222222222222",
                "1000",
                None,
                None,
                None,
                1005,
            )
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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("bsc_underpriced.json"));
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_underpriced";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed_bsc(
                draft_id,
                "mock_hash",
                "0xf86c...",
                "0x1111111111111111111111111111111111111111",
                "0x2222222222222222222222222222222222222222",
                "1000",
                None,
                None,
                None,
                1005,
            )
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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("bsc_revert.json"));
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_revert";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed_bsc(
                draft_id,
                "mock_hash",
                "0xf86c...",
                "0x1111111111111111111111111111111111111111",
                "0x2222222222222222222222222222222222222222",
                "1000",
                None,
                None,
                None,
                1005,
            )
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
        let temp_dir = tempfile::tempdir().unwrap();
        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(temp_dir.path().join("bsc_reorg.json"));
        let broadcaster = MockBscBroadcaster::new();

        let draft_id = "draft_bsc_reorg";
        coordinator
            .register_draft(draft_id, BroadcastNetwork::BscTestnet, 1000)
            .unwrap();
        coordinator
            .record_signed_bsc(
                draft_id,
                "mock_hash",
                "0xf86c...",
                "0x1111111111111111111111111111111111111111",
                "0x2222222222222222222222222222222222222222",
                "1000",
                None,
                None,
                None,
                1005,
            )
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
        coordinator1.set_storage_path(persist_file.clone());
        let btc_broadcaster = MockBtcBroadcaster::new();
        let bsc_broadcaster = MockBscBroadcaster::new();

        let btc_draft = "draft_btc_persist_01";
        coordinator1.register_draft(btc_draft, BroadcastNetwork::BitcoinSignet, 1000).unwrap();
        coordinator1.record_signed_btc(btc_draft, "hash_btc_1", "0200000001dead", "txid_btc_mock", "wtxid_btc_mock", None, 1005).unwrap();
        let btc_txid = coordinator1.execute_broadcast_btc(btc_draft, &btc_broadcaster, 3, 1010).unwrap();

        btc_broadcaster.set_tx_status(&btc_txid, BtcTxQueryStatus::Confirmed {
            block_height: 150_000,
            block_hash: "00000000block_signet_abc".to_string(),
            confirmations: 2,
        });
        coordinator1.poll_btc_lifecycle(btc_draft, &btc_broadcaster, 1020).unwrap();

        let bsc_draft = "draft_bsc_persist_01";
        coordinator1.register_draft(bsc_draft, BroadcastNetwork::BscTestnet, 1030).unwrap();
        coordinator1.record_signed_bsc(
            bsc_draft,
            "hash_bsc_1",
            "f86c...",
            "0x1111111111111111111111111111111111111111",
            "0x2222222222222222222222222222222222222222",
            "1000",
            None,
            None,
            None,
            1035,
        ).unwrap();
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

        let dummy_sig = secp.sign_ecdsa(&secp256k1::Message::from_digest([1u8; 32]), &privkey);
        let mut sig_bytes = dummy_sig.serialize_der().to_vec();
        sig_bytes.push(0x01); // SIGHASH_ALL

        let mut witness = bitcoin::Witness::new();
        witness.push(sig_bytes);
        witness.push(privkey.public_key(&secp).serialize());

        let tx = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: outpoint,
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness,
            }],
            output: vec![bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(45_000),
                script_pubkey: script,
            }],
        };

        let raw_hex = hex::encode(bitcoin::consensus::serialize(&tx));
        let actual_txid = tx.compute_txid().to_string();
        let actual_wtxid = tx.compute_wtxid().to_string();

        // 1. With exact matching txid and wtxid -> succeeds in dry run
        let valid_res = btc_adapter.broadcast_verified(&raw_hex, &actual_txid, &actual_wtxid, None, BroadcastNetwork::BitcoinSignet);
        assert!(valid_res.is_ok());

        // 2. With altered / unexpected txid -> REJECTED
        let forged_txid = "000000000000000000000000000000000000000000000000000000000000dead";
        let invalid_res = btc_adapter.broadcast_verified(&raw_hex, forged_txid, &actual_wtxid, None, BroadcastNetwork::BitcoinSignet);
        assert!(invalid_res.is_err(), "Must reject raw tx if txid does not match approved draft");
        assert!(invalid_res.unwrap_err().to_string().contains("does not match approved transaction txid"));

        // 3. With altered / forged wtxid (witness tampering) -> REJECTED
        let forged_wtxid = "000000000000000000000000000000000000000000000000000000000000beef";
        let invalid_wtxid_res = btc_adapter.broadcast_verified(&raw_hex, &actual_txid, forged_wtxid, None, BroadcastNetwork::BitcoinSignet);
        assert!(invalid_wtxid_res.is_err(), "Must reject raw tx if wtxid does not match approved draft");
        assert!(invalid_wtxid_res.unwrap_err().to_string().contains("witness mismatch"));

        // 4. Raw tx with empty witness -> REJECTED by SegWit rule
        let mut unsigned_tx = tx.clone();
        unsigned_tx.input[0].witness = bitcoin::Witness::new();
        let unsigned_hex = hex::encode(bitcoin::consensus::serialize(&unsigned_tx));
        let unsigned_res = btc_adapter.broadcast_verified(&unsigned_hex, &actual_txid, &actual_wtxid, None, BroadcastNetwork::BitcoinSignet);
        assert!(unsigned_res.is_err());
        assert!(unsigned_res.unwrap_err().to_string().contains("lacks SegWit witness"));
    }

    #[test]
    fn test_network_preflight_raw_verification() {
        // BTC Preflight Verification:
        let btc_adapter = EsploraBtcBroadcaster::new(false);
        let secp_btc = bitcoin::secp256k1::Secp256k1::new();
        let privkey_btc = bip::derive_bip84_privkey_at(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            bitcoin::Network::Signet,
            0,
            0,
        ).unwrap();
        let script_btc = bitcoin::ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::hash(&privkey_btc.public_key(&secp_btc).serialize()));

        let outpoint = bitcoin::OutPoint {
            txid: bitcoin::Txid::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap(),
            vout: 0,
        };

        // Create a syntactically valid DER signature for witness preflight check
        let dummy_sig = secp_btc.sign_ecdsa(&secp256k1::Message::from_digest([1u8; 32]), &privkey_btc);
        let mut sig_bytes = dummy_sig.serialize_der().to_vec();
        sig_bytes.push(0x01); // SIGHASH_ALL

        let mut witness = bitcoin::Witness::new();
        witness.push(sig_bytes);
        witness.push(privkey_btc.public_key(&secp_btc).serialize());

        let valid_btc_tx = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: outpoint,
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness,
            }],
            output: vec![bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(50_000),
                script_pubkey: script_btc,
            }],
        };
        let btc_raw_hex = hex::encode(bitcoin::consensus::serialize(&valid_btc_tx));
        let expected_txid = valid_btc_tx.compute_txid().to_string();
        let expected_wtxid = valid_btc_tx.compute_wtxid().to_string();

        // BTC: Valid match succeeds
        assert!(btc_adapter.broadcast_verified(&btc_raw_hex, &expected_txid, &expected_wtxid, None, BroadcastNetwork::BitcoinSignet).is_ok());
        // BTC: Wrong network rejected
        assert!(btc_adapter.broadcast_verified(&btc_raw_hex, &expected_txid, &expected_wtxid, None, BroadcastNetwork::BscTestnet).is_err());

        // BSC Preflight Verification:
        let bsc_adapter = RpcBscBroadcaster::new(false);
        let secp_bsc = secp256k1::Secp256k1::new();
        let privkey_bsc = secp256k1::SecretKey::from_slice(&[7u8; 32]).unwrap();
        let pubkey_bsc = secp256k1::PublicKey::from_secret_key(&secp_bsc, &privkey_bsc);
        let uncompressed = pubkey_bsc.serialize_uncompressed();
        let mut hasher = tiny_keccak::Keccak::v256();
        let mut addr_hash = [0u8; 32];
        hasher.update(&uncompressed[1..]);
        hasher.finalize(&mut addr_hash);
        let expected_sender = crate::wallet::tx::policy::to_eip55_checksum(&hex::encode(&addr_hash[12..32]));

        let recipient_str = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";
        let amount_raw_str = "2500000000000000000"; // 2.5 USDT
        let calldata = crate::wallet::tx::bsc::calldata::encode_erc20_transfer(
            recipient_str,
            &num_bigint::BigUint::from(2500000000000000000u64),
        ).unwrap();

        let bsc_tx = crate::wallet::signer::bsc::Eip155Transaction {
            nonce: 5,
            gas_price: num_bigint::BigUint::from(3_000_000_000u64),
            gas_limit: 65_000,
            to: crate::wallet::blockchain::BSC_TESTNET_USDT_CONTRACT.to_string(),
            value: num_bigint::BigUint::from(0u32),
            data: hex::decode(calldata.trim_start_matches("0x")).unwrap(),
            chain_id: 97, // BSC Testnet
        };
        let signed_bsc = bsc_tx.sign(&secp_bsc, &privkey_bsc).unwrap();
        let bsc_raw_hex = hex::encode(&signed_bsc.raw_signed_tx_bytes);

        // BSC 1: Valid signed payload matches sender, recipient, amount, nonce, gas -> PASS
        let verify_ok = bsc_adapter.dry_run_verify_against_draft(
            &bsc_raw_hex,
            Some(&expected_sender),
            Some(recipient_str),
            Some(amount_raw_str),
            Some(5),
            Some(65_000),
            Some("3000000000"),
            BroadcastNetwork::BscTestnet,
        );
        assert!(verify_ok.is_ok(), "BSC preflight valid draft must succeed: {:?}", verify_ok);

        // BSC 2: Forged recipient -> REJECTED
        let forged_recip = "0x1111111111111111111111111111111111111111";
        let recip_fail = bsc_adapter.dry_run_verify_against_draft(
            &bsc_raw_hex,
            Some(&expected_sender),
            Some(forged_recip),
            Some(amount_raw_str),
            Some(5),
            Some(65_000),
            Some("3000000000"),
            BroadcastNetwork::BscTestnet,
        );
        assert!(recip_fail.is_err(), "Must reject forged recipient");
        assert!(recip_fail.unwrap_err().to_string().contains("recipient"));

        // BSC 3: Forged amount -> REJECTED
        let forged_amount = "9999999999999999999";
        let amount_fail = bsc_adapter.dry_run_verify_against_draft(
            &bsc_raw_hex,
            Some(&expected_sender),
            Some(recipient_str),
            Some(forged_amount),
            Some(5),
            Some(65_000),
            Some("3000000000"),
            BroadcastNetwork::BscTestnet,
        );
        assert!(amount_fail.is_err(), "Must reject forged amount");
        assert!(amount_fail.unwrap_err().to_string().contains("amount"));

        // BSC 4: Forged sender -> REJECTED
        let forged_sender = "0x2222222222222222222222222222222222222222";
        let sender_fail = bsc_adapter.dry_run_verify_against_draft(
            &bsc_raw_hex,
            Some(forged_sender),
            Some(recipient_str),
            Some(amount_raw_str),
            Some(5),
            Some(65_000),
            Some("3000000000"),
            BroadcastNetwork::BscTestnet,
        );
        assert!(sender_fail.is_err(), "Must reject forged sender");
        assert!(sender_fail.unwrap_err().to_string().contains("sender"));

        // BSC 5: Trailing garbage bytes appended -> REJECTED by strict RLP consumption check
        let trailing_garbage_hex = format!("{}deadbeef", bsc_raw_hex);
        let trailing_fail = bsc_adapter.dry_run_verify_against_draft(
            &trailing_garbage_hex,
            Some(&expected_sender),
            Some(recipient_str),
            Some(amount_raw_str),
            Some(5),
            Some(65_000),
            Some("3000000000"),
            BroadcastNetwork::BscTestnet,
        );
        assert!(trailing_fail.is_err(), "Must reject trailing garbage bytes");
        assert!(trailing_fail.unwrap_err().to_string().contains("trailing"));
    }

    #[test]
    fn test_crash_safe_broadcaster_auto_persist() {
        let temp_dir = tempfile::tempdir().unwrap();
        let persist_file = temp_dir.path().join("crash_safe_lifecycle.json");

        let mut coordinator = TxLifecycleCoordinator::new();
        coordinator.set_storage_path(persist_file.clone());

        let draft_id = "draft_crash_safety_btc_01";
        coordinator.register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 2000).unwrap();

        let raw_tx = "0200000001deadbeef0000000000";
        let payload_hash = "0xhash_auto_persist_test";
        coordinator.record_signed_btc(draft_id, payload_hash, raw_tx, "txid_expected_01", "wtxid_expected_01", None, 2005).unwrap();

        // Broadcaster mock configured to fail on broadcast with timeout
        let broadcaster = MockBtcBroadcaster::new();
        broadcaster.next_broadcast_result.lock().unwrap().replace(Err(BtcBroadcastError::Timeout("Connection timeout".into())));

        // Execute broadcast (which will retry once and fail)
        let broadcast_res = coordinator.execute_broadcast_btc(draft_id, &broadcaster, 1, 2010);
        assert!(broadcast_res.is_err());

        // CRITICAL CHECK: Auto-persist file on disk must exist WITHOUT having manually called save_to_file!
        assert!(persist_file.exists(), "Coordinator must auto-persist state before and after network I/O");

        // Simulate application crash and restart by dropping coordinator and loading fresh instance from file
        drop(coordinator);
        let restored_coordinator = TxLifecycleCoordinator::load_from_file(&persist_file).unwrap();
        let restored_rec = restored_coordinator.get_record(draft_id).unwrap();

        // Verify that raw_tx_hex and payload_hash were preserved across the crash
        assert_eq!(restored_rec.raw_tx_hex.as_deref(), Some(raw_tx));
        assert_eq!(restored_rec.payload_hash.as_deref(), Some(payload_hash));
        assert!(matches!(restored_rec.status, TxLifecycleStatus::Failed { permanent: false, .. }));
    }

    #[test]
    fn test_sqlite_reset_with_open_connection_safety() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bdk_db_dir = temp_dir.path().join("bdk_signet_db");
        std::fs::create_dir_all(&bdk_db_dir).unwrap();
        let db_file = bdk_db_dir.join("bdk_signet.sqlite3");
        std::fs::write(&db_file, b"sample bdk sqlite database").unwrap();

        let tracker = std::sync::Arc::new(crate::wallet::SqliteTracker::default());

        // 1. Simulate an active connection open by acquiring a connection guard
        let conn_guard = tracker.acquire_connection().expect("Must acquire connection");
        assert_eq!(tracker.active_connections(), 1);

        // 2. Attempting to begin reset while connection is active MUST fail safely
        let reset_attempt = tracker.begin_reset();
        assert!(reset_attempt.is_err(), "Reset must fail safely when SQLite connection is open");
        assert!(reset_attempt.unwrap_err().contains("active SQLite database connection(s) still open"));

        // Ensure database files were NOT deleted or corrupted
        assert!(db_file.exists(), "Database file must remain intact after aborted reset");

        // 3. Close the connection by dropping the guard
        drop(conn_guard);
        assert_eq!(tracker.active_connections(), 0);

        // 4. Now begin_reset succeeds
        assert!(tracker.begin_reset().is_ok());

        // 5. Execute cleanup_sqlite_files
        let cleanup_res = crate::wallet::cleanup_sqlite_files(temp_dir.path());
        assert!(cleanup_res.is_ok(), "Cleanup must succeed after connections closed: {:?}", cleanup_res);

        // Verify complete purge
        assert!(!bdk_db_dir.exists(), "Database directory must be purged");
        assert!(!db_file.exists(), "Database file must be purged");

        tracker.finish_reset();
    }

    #[test]
    fn test_t61_atomic_sqlite_tracker_concurrent_race() {
        let tracker = std::sync::Arc::new(crate::wallet::SqliteTracker::default());
        let mut handles = Vec::new();

        // 8 worker threads acquiring and releasing connections
        for _ in 0..8 {
            let t = std::sync::Arc::clone(&tracker);
            handles.push(std::thread::spawn(move || {
                for _ in 0..60 {
                    if let Ok(guard) = t.acquire_connection() {
                        assert!(!t.is_locked(), "Must never hold an active connection while reset is locked");
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        drop(guard);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }));
        }

        // 8 reset threads attempting to lock and begin reset
        for _ in 0..8 {
            let t = std::sync::Arc::clone(&tracker);
            handles.push(std::thread::spawn(move || {
                for _ in 0..60 {
                    if t.begin_reset().is_ok() {
                        // Critical invariant: While reset is active, active connections must be strictly 0
                        assert_eq!(t.active_connections(), 0, "active_connections must be 0 during reset");
                        std::thread::sleep(std::time::Duration::from_millis(2));
                        t.finish_reset();
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }));
        }

        for h in handles {
            h.join().expect("Worker thread panicked");
        }

        assert_eq!(tracker.active_connections(), 0);
        assert!(!tracker.is_locked());
    }

    #[test]
    fn test_t62_mandatory_signed_authorization() {
        let temp_dir = tempfile::tempdir().unwrap();
        let persist_file = temp_dir.path().join("t62_lifecycle.json");

        // 1. Missing storage path fails immediately
        let mut unconfigured_coord = TxLifecycleCoordinator::new();
        let mock_btc = MockBtcBroadcaster::new();
        let err_no_path = unconfigured_coord.execute_broadcast_btc("draft_01", &mock_btc, 1, 100);
        assert!(err_no_path.is_err());
        assert!(err_no_path.unwrap_err().contains("Mandatory storage path"));

        let mut coord = TxLifecycleCoordinator::new();
        coord.set_storage_path(persist_file);

        // 2. BTC: Missing expected_txid / expected_wtxid fails-closed with zero fallback
        let draft_btc = "draft_t62_btc";
        coord.register_draft(draft_btc, BroadcastNetwork::BitcoinSignet, 100).unwrap();
        coord.record_signed(draft_btc, "hash_01", "raw_dummy_hex", 105).unwrap();
        let err_btc_missing = coord.execute_broadcast_btc(draft_btc, &mock_btc, 1, 110);
        assert!(err_btc_missing.is_err());
        assert!(err_btc_missing.unwrap_err().contains("missing approved expected_txid"));

        // 3. BSC: Missing expected_sender / recipient / amount fails-closed with zero fallback
        let draft_bsc = "draft_t62_bsc";
        let mock_bsc = MockBscBroadcaster::new();
        coord.register_draft(draft_bsc, BroadcastNetwork::BscTestnet, 200).unwrap();
        coord.record_signed(draft_bsc, "hash_02", "raw_dummy_hex_bsc", 205).unwrap();
        let err_bsc_missing = coord.execute_broadcast_bsc(draft_bsc, &mock_bsc, 1, 210);
        assert!(err_bsc_missing.is_err());
        assert!(err_bsc_missing.unwrap_err().contains("missing approved expected_sender"));
    }

    #[test]
    fn test_t63_real_signature_preflight() {
        // --- BTC Real Signature & BIP-143 Sighash Verification ---
        let btc_adapter = EsploraBtcBroadcaster::new(false);
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let privkey = bip::derive_bip84_privkey_at(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            bitcoin::Network::Signet,
            0,
            0,
        ).unwrap();
        let pubkey = privkey.public_key(&secp);
        let script_pubkey = bitcoin::ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::hash(&pubkey.serialize()));

        let outpoint = bitcoin::OutPoint {
            txid: bitcoin::Txid::from_str("0000000000000000000000000000000000000000000000000000000000000009").unwrap(),
            vout: 0,
        };

        let mut tx_to_sign = bitcoin::Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![bitcoin::TxIn {
                previous_output: outpoint,
                script_sig: bitcoin::ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: bitcoin::Witness::new(),
            }],
            output: vec![bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(50_000),
                script_pubkey: script_pubkey.clone(),
            }],
        };

        // Compute genuine BIP-143 sighash
        let mut sighash_cache = bitcoin::sighash::SighashCache::new(&tx_to_sign);
        let sighash = sighash_cache.p2wpkh_signature_hash(
            0,
            &script_pubkey,
            bitcoin::Amount::from_sat(55_000),
            bitcoin::sighash::EcdsaSighashType::All,
        ).unwrap();
        let msg = secp256k1::Message::from_digest(sighash.to_byte_array());
        let real_sig = secp.sign_ecdsa(&msg, &privkey);
        let mut sig_bytes = real_sig.serialize_der().to_vec();
        sig_bytes.push(0x01); // SIGHASH_ALL

        let mut witness = bitcoin::Witness::new();
        witness.push(sig_bytes.clone());
        witness.push(pubkey.serialize());
        tx_to_sign.input[0].witness = witness;

        let valid_btc_hex = hex::encode(bitcoin::consensus::serialize(&tx_to_sign));
        let expected_txid = tx_to_sign.compute_txid().to_string();
        let expected_wtxid = tx_to_sign.compute_wtxid().to_string();

        // 1. Valid signed BTC tx with input values passes cryptographic verification
        let valid_res = btc_adapter.broadcast_verified(
            &valid_btc_hex,
            &expected_txid,
            &expected_wtxid,
            Some(&[55_000]),
            BroadcastNetwork::BitcoinSignet,
        );
        assert!(valid_res.is_ok(), "Valid BIP-143 signed tx must pass: {:?}", valid_res);

        // 2. Corrupted signature byte fails ECDSA verification
        let mut corrupted_sig_bytes = sig_bytes.clone();
        let last_idx = corrupted_sig_bytes.len() - 3;
        corrupted_sig_bytes[last_idx] ^= 0x55; // flip signature bits
        let mut corrupted_tx = tx_to_sign.clone();
        let mut bad_witness = bitcoin::Witness::new();
        bad_witness.push(corrupted_sig_bytes);
        bad_witness.push(pubkey.serialize());
        corrupted_tx.input[0].witness = bad_witness;
        let corrupted_btc_hex = hex::encode(bitcoin::consensus::serialize(&corrupted_tx));

        let bad_sig_res = btc_adapter.broadcast_verified(
            &corrupted_btc_hex,
            &corrupted_tx.compute_txid().to_string(),
            &corrupted_tx.compute_wtxid().to_string(),
            Some(&[55_000]),
            BroadcastNetwork::BitcoinSignet,
        );
        assert!(bad_sig_res.is_err(), "Corrupted signature byte must fail cryptographic verification");

        // 3. Dummy / mock witness fails immediately
        let mut dummy_tx = tx_to_sign.clone();
        let mut dummy_witness = bitcoin::Witness::new();
        dummy_witness.push(vec![0x30, 0x44, 0x02, 0x20, 0xaa]); // truncated mock
        dummy_witness.push(pubkey.serialize());
        dummy_tx.input[0].witness = dummy_witness;
        let dummy_hex = hex::encode(bitcoin::consensus::serialize(&dummy_tx));
        let dummy_res = btc_adapter.dry_run_verify(&dummy_hex, BroadcastNetwork::BitcoinSignet);
        assert!(dummy_res.is_err(), "Mock dummy witness must fail preflight check");

        // --- BSC Parameter Tampering Verification ---
        let bsc_adapter = RpcBscBroadcaster::new(false);
        let secp_bsc = secp256k1::Secp256k1::new();
        let privkey_bsc = secp256k1::SecretKey::from_slice(&[9u8; 32]).unwrap();
        let pubkey_bsc = secp256k1::PublicKey::from_secret_key(&secp_bsc, &privkey_bsc);
        let uncompressed = pubkey_bsc.serialize_uncompressed();
        let mut hasher = tiny_keccak::Keccak::v256();
        let mut addr_hash = [0u8; 32];
        hasher.update(&uncompressed[1..]);
        hasher.finalize(&mut addr_hash);
        let sender = crate::wallet::tx::policy::to_eip55_checksum(&hex::encode(&addr_hash[12..32]));

        let recipient = "0x9858EfFD232B4033E47d90003D41EC34EcaEda94";
        let amount_str = "1000000000000000000"; // 1 USDT
        let calldata = crate::wallet::tx::bsc::calldata::encode_erc20_transfer(
            recipient,
            &num_bigint::BigUint::from(1000000000000000000u64),
        ).unwrap();

        let tx_bsc = crate::wallet::signer::bsc::Eip155Transaction {
            nonce: 10,
            gas_price: num_bigint::BigUint::from(5_000_000_000u64),
            gas_limit: 70_000,
            to: crate::wallet::blockchain::BSC_TESTNET_USDT_CONTRACT.to_string(),
            value: num_bigint::BigUint::from(0u32),
            data: hex::decode(calldata.trim_start_matches("0x")).unwrap(),
            chain_id: 97,
        };
        let signed_bsc = tx_bsc.sign(&secp_bsc, &privkey_bsc).unwrap();
        let bsc_hex = hex::encode(&signed_bsc.raw_signed_tx_bytes);

        // Valid match passes
        assert!(bsc_adapter.dry_run_verify_against_draft(
            &bsc_hex,
            Some(&sender),
            Some(recipient),
            Some(amount_str),
            Some(10),
            Some(70_000),
            Some("5000000000"),
            BroadcastNetwork::BscTestnet,
        ).is_ok());

        // Modified nonce fails
        let bad_nonce = bsc_adapter.dry_run_verify_against_draft(
            &bsc_hex,
            Some(&sender),
            Some(recipient),
            Some(amount_str),
            Some(11), // Expected 11 but tx has 10
            Some(70_000),
            Some("5000000000"),
            BroadcastNetwork::BscTestnet,
        );
        assert!(bad_nonce.is_err());
        assert!(bad_nonce.unwrap_err().to_string().contains("nonce"));

        // Modified gas_limit fails
        let bad_gas = bsc_adapter.dry_run_verify_against_draft(
            &bsc_hex,
            Some(&sender),
            Some(recipient),
            Some(amount_str),
            Some(10),
            Some(80_000), // Expected 80_000 but tx has 70_000
            Some("5000000000"),
            BroadcastNetwork::BscTestnet,
        );
        assert!(bad_gas.is_err());
        assert!(bad_gas.unwrap_err().to_string().contains("gas limit"));
    }

    #[test]
    fn test_t64_kill_restart_broadcast_and_recovery() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage_file = temp_dir.path().join("t64_recovery.json");

        let draft_id = "draft_t64_lifecycle";
        let raw_tx = "020000000100000000000000000000000000000000000000000000000000000000000000000000000000ffffffff";
        let txid = "txid_t64_recovery_test";
        let wtxid = "wtxid_t64_recovery_test";

        // Phase 1: Setup draft, record signed, initiate broadcast with timeout
        {
            let mut coordinator = TxLifecycleCoordinator::new();
            coordinator.set_storage_path(storage_file.clone());
            coordinator.register_draft(draft_id, BroadcastNetwork::BitcoinSignet, 5000).unwrap();
            coordinator.record_signed_btc(draft_id, "payload_hash_t64", raw_tx, txid, wtxid, None, 5005).unwrap();

            let broadcaster = MockBtcBroadcaster::new();
            broadcaster.next_broadcast_result.lock().unwrap().replace(Err(BtcBroadcastError::Timeout("Network timeout during broadcast".into())));

            let res = coordinator.execute_broadcast_btc(draft_id, &broadcaster, 1, 5010);
            assert!(res.is_err(), "Broadcast must report transient error");
            // Coordinator is dropped here (simulating crash / app shutdown)
        }

        // Phase 2: RESTART FROM DISK
        assert!(storage_file.exists(), "Lifecycle file must have been auto-persisted prior to crash");
        let mut restored_coord = TxLifecycleCoordinator::load_from_file(&storage_file).unwrap();
        let record = restored_coord.get_record(draft_id).unwrap();
        assert!(matches!(record.status, TxLifecycleStatus::Failed { permanent: false, .. }));
        assert_eq!(record.raw_tx_hex.as_deref(), Some(raw_tx));
        assert_eq!(record.expected_txid.as_deref(), Some(txid));
        assert_eq!(record.expected_wtxid.as_deref(), Some(wtxid));

        // Phase 3: Resume broadcast WITHOUT re-signing
        let broadcaster = MockBtcBroadcaster::new();
        // Broadcaster now succeeds and returns txid
        broadcaster.set_next_broadcast_result(Ok(txid.to_string()));
        let resume_res = restored_coord.execute_broadcast_btc(draft_id, &broadcaster, 1, 5020);
        assert!(resume_res.is_ok(), "Must resume broadcast successfully without re-signing: {:?}", resume_res);

        // Verify status moved to Pending and persisted
        let record_pending = restored_coord.get_record(draft_id).unwrap();
        assert!(matches!(record_pending.status, TxLifecycleStatus::Pending { .. }));

        // Phase 4: Poll status to Confirmed
        broadcaster.set_tx_status(txid, BtcTxQueryStatus::Confirmed {
            block_height: 800_000,
            block_hash: "0000000000000000000000000000000000000000000000000000000000abcdef".to_string(),
            confirmations: 1,
        });
        let poll_res = restored_coord.poll_btc_lifecycle(draft_id, &broadcaster, 5030);
        assert!(poll_res.is_ok());
        assert!(matches!(poll_res.unwrap(), TxLifecycleStatus::Confirmed { .. }));

        // Phase 5: Reload once more to confirm final Confirmed status is on disk
        drop(restored_coord);
        let final_coord = TxLifecycleCoordinator::load_from_file(&storage_file).unwrap();
        let final_rec = final_coord.get_record(draft_id).unwrap();
        assert!(matches!(final_rec.status, TxLifecycleStatus::Confirmed { .. }));
    }
}
