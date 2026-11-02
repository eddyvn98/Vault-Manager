use std::path::Path;
use bdk_wallet::{KeychainKind, PersistedWallet, Wallet};
use bitcoin::Network;
use rusqlite::Connection;

use super::descriptors::Bip84Descriptors;

/// Open an existing BDK SQLite database or initialize a new one with the given BIP84 descriptors.
/// If `db_path` is `None`, an in-memory SQLite connection is used.
pub fn open_or_init_wallet(
    db_path: Option<&Path>,
    desc: &Bip84Descriptors,
    network: Network,
) -> Result<(PersistedWallet<Connection>, Connection), String> {
    let mut conn = match db_path {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create SQLite DB directory {:?}: {}", parent, e))?;
            }
            Connection::open(path)
                .map_err(|e| format!("Failed to open SQLite database at {:?}: {}", path, e))?
        }
        None => Connection::open_in_memory()
            .map_err(|e| format!("Failed to open in-memory SQLite connection: {}", e))?,
    };

    // 1. Attempt to load existing wallet from SQLite
    let loaded = Wallet::load()
        .descriptor(KeychainKind::External, Some(desc.receive_descriptor.clone()))
        .descriptor(KeychainKind::Internal, Some(desc.change_descriptor.clone()))
        .load_wallet(&mut conn);

    match loaded {
        Ok(Some(wallet)) => {
            if wallet.network() != network {
                return Err(format!(
                    "Persisted wallet network mismatch: expected {:?}, found {:?}",
                    network,
                    wallet.network()
                ));
            }
            Ok((wallet, conn))
        }
        Ok(None) => {
            // Database is uninitialized: create wallet and persist initial changeset
            let wallet = Wallet::create(desc.receive_descriptor.clone(), desc.change_descriptor.clone())
                .network(network)
                .create_wallet(&mut conn)
                .map_err(|e| format!("Failed to initialize BDK wallet in SQLite: {}", e))?;
            Ok((wallet, conn))
        }
        Err(e) => Err(format!(
            "Failed to load persisted BDK wallet (descriptor mismatch or corrupted DB): {}",
            e
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bdk_wallet::KeychainKind;
    use tempfile::tempdir;
    use crate::wallet::tx::btc::descriptors;

    const TEST_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn it01_bdk_sqlite_persistence_and_index_recovery() {
        let dir = tempdir().expect("must create temp dir");
        let db_path = dir.path().join("bdk_signet_test.sqlite3");
        let desc = descriptors::derive_signet_bip84_descriptors(TEST_MNEMONIC).unwrap();

        // 1. First run: Initialize new wallet in SQLite
        let first_addr_0;
        let first_addr_1;
        {
            let (mut wallet, mut conn) = open_or_init_wallet(Some(&db_path), &desc, Network::Signet)
                .expect("Must initialize new wallet");

            assert_eq!(wallet.network(), Network::Signet);

            // Reveal 2 receive addresses
            let addr_info_0 = wallet.reveal_next_address(KeychainKind::External);
            let addr_info_1 = wallet.reveal_next_address(KeychainKind::External);
            first_addr_0 = addr_info_0.address.to_string();
            first_addr_1 = addr_info_1.address.to_string();

            assert_eq!(addr_info_0.index, 0);
            assert_eq!(addr_info_1.index, 1);
            assert_ne!(first_addr_0, first_addr_1);

            // Reveal 1 change address
            let change_info_0 = wallet.reveal_next_address(KeychainKind::Internal);
            assert_eq!(change_info_0.index, 0);

            // Crucial: Persist state to SQLite
            wallet.persist(&mut conn).expect("Must persist wallet changes");
        } // Connection and wallet dropped (simulating app restart/close)

        // 2. Second run: Load existing wallet from SQLite
        {
            let (mut wallet, mut conn) = open_or_init_wallet(Some(&db_path), &desc, Network::Signet)
                .expect("Must load existing wallet from SQLite");

            // Verify that previous index is preserved: next revealed address must be index 2!
            let addr_info_2 = wallet.reveal_next_address(KeychainKind::External);
            assert_eq!(
                addr_info_2.index, 2,
                "Next address index must be 2, never reusing indices 0 or 1"
            );
            assert_ne!(addr_info_2.address.to_string(), first_addr_0);
            assert_ne!(addr_info_2.address.to_string(), first_addr_1);

            // Change address index must be 1 (index 0 was used)
            let change_info_1 = wallet.reveal_next_address(KeychainKind::Internal);
            assert_eq!(
                change_info_1.index, 1,
                "Next change index must be 1, never reusing change index 0"
            );

            wallet.persist(&mut conn).expect("Must persist second run changes");
        }

        // 3. Third run: Verify that opening with a mismatched descriptor is rejected
        {
            let other_mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
            let wrong_desc = descriptors::derive_signet_bip84_descriptors(other_mnemonic).unwrap();
            let load_res = open_or_init_wallet(Some(&db_path), &wrong_desc, Network::Signet);
            assert!(
                load_res.is_err(),
                "Loading existing DB with wrong descriptor must fail"
            );
        }
    }
}
