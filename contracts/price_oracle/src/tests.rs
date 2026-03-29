#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Bytes, BytesN};

fn upload_wasm(e: &Env) -> BytesN<32> {
    // Empty WASM is accepted in testutils and is sufficient for upgrade tests.
    let wasm = Bytes::new(e);
    e.deployer().upload_contract_wasm(wasm)
}

#[test]
fn test_initialize() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        let r = PriceOracleContract::initialize(e.clone(), admin.clone());
        assert_eq!(r, Ok(()));
    });

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_max_staleness(), 3600);
    assert_eq!(client.get_version(), CURRENT_VERSION);
}

#[test]
fn test_initialize_twice_fails() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        let r = PriceOracleContract::initialize(e.clone(), admin.clone());
        assert_eq!(r, Err(OracleError::AlreadyInitialized));
    });
}

#[test]
fn test_add_remove_oracle_admin_only() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    client.add_oracle(&admin, &oracle);
    assert!(client.is_oracle_whitelisted(&oracle));

    client.remove_oracle(&admin, &oracle);
    assert!(!client.is_oracle_whitelisted(&oracle));
}

#[test]
fn test_set_price_whitelisted() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &1000_00000000, &8);
    let data = client.get_price(&asset);
    assert_eq!(data.price, 1000_00000000);
    assert_eq!(data.decimals, 8);
    assert_eq!(data.updated_at, e.ledger().timestamp());
}

#[test]
#[should_panic(expected = "Oracle not whitelisted")]
fn test_set_price_unauthorized_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let unauthorized = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    client.set_price(&unauthorized, &asset, &1000, &8);
}

#[test]
fn test_get_price_valid_fresh() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &500_0000000, &8);
    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 500_0000000);
}

#[test]
#[should_panic]
fn test_get_price_valid_not_found() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    let _ = client.get_price_valid(&asset, &None);
}

#[test]
#[should_panic]
fn test_get_price_valid_stale() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &1000, &8);

    // Advance time past max staleness (default 3600)
    e.ledger().with_mut(|li| {
        li.timestamp += 4000;
    });

    let _ = client.get_price_valid(&asset, &None);
}

#[test]
fn test_get_price_valid_override_staleness() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &1000, &8);
    e.ledger().with_mut(|li| {
        li.timestamp += 100;
    });

    // Override: allow 200 seconds staleness, so still valid
    let data = client.get_price_valid(&asset, &Some(200));
    assert_eq!(data.price, 1000);
}

#[test]
fn test_get_price_valid_accepts_exact_staleness_boundary() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &42_00000000, &8);
    e.ledger().with_mut(|li| {
        li.timestamp += 3600;
    });

    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 42_00000000);
    assert_eq!(data.decimals, 8);
}

#[test]
fn test_get_price_valid_rejects_future_dated_price() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        e.storage().instance().set(
            &DataKey::Price(asset.clone()),
            &PriceData {
                price: 1234,
                updated_at: 500,
                decimals: 8,
            },
        );
    });

    e.ledger().with_mut(|li| {
        li.timestamp = 499;
    });

    assert_eq!(
        client.try_get_price_valid(&asset, &None),
        Err(Ok(OracleError::StalePrice))
    );
}

#[test]
fn test_set_max_staleness() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    client.set_max_staleness(&admin, &7200);
    assert_eq!(client.get_max_staleness(), 7200);
}

#[test]
fn test_fallback_get_price_returns_default_when_not_set() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    let data = client.get_price(&asset);
    assert_eq!(data.price, 0);
    assert_eq!(data.updated_at, 0);
    assert_eq!(data.decimals, 0);
}

#[test]
fn test_upgrade_and_migrate_preserves_state() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    client.add_oracle(&admin, &oracle);
    client.set_price(&oracle, &asset, &2_000, &6);

    // Simulate legacy storage layout (version 0)
    e.as_contract(&contract_id, || {
        e.storage().instance().remove(&DataKey::Version);
        e.storage().instance().remove(&DataKey::OracleConfig);
        e.storage()
            .instance()
            .set(&DataKey::MaxStalenessSeconds, &3000u64);
    });

    let wasm_hash = upload_wasm(&e);
    assert_eq!(client.try_upgrade(&admin, &wasm_hash), Ok(Ok(())));

    assert_eq!(client.try_migrate(&admin, &0), Ok(Ok(())));
    assert_eq!(client.get_version(), CURRENT_VERSION);
    assert_eq!(client.get_max_staleness(), 3000);

    let data = client.get_price(&asset);
    assert_eq!(data.price, 2_000);
    assert_eq!(data.decimals, 6);
}

#[test]
fn test_upgrade_authorization_and_invalid_hash() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    let wasm_hash = upload_wasm(&e);
    assert_eq!(
        client.try_upgrade(&attacker, &wasm_hash),
        Err(Ok(OracleError::Unauthorized))
    );

    let zero = BytesN::from_array(&e, &[0; 32]);
    assert_eq!(
        client.try_upgrade(&admin, &zero),
        Err(Ok(OracleError::InvalidWasmHash))
    );
}

#[test]
fn test_migrate_version_checks_and_replay_safety() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    // Simulate legacy layout (version 0)
    e.as_contract(&contract_id, || {
        e.storage().instance().remove(&DataKey::Version);
        e.storage().instance().remove(&DataKey::OracleConfig);
        e.storage()
            .instance()
            .set(&DataKey::MaxStalenessSeconds, &7200u64);
    });

    assert_eq!(
        client.try_migrate(&attacker, &0),
        Err(Ok(OracleError::Unauthorized))
    );
    assert_eq!(
        client.try_migrate(&admin, &(CURRENT_VERSION + 1)),
        Err(Ok(OracleError::InvalidVersion))
    );

    assert_eq!(client.try_migrate(&admin, &0), Ok(Ok(())));
    assert_eq!(
        client.try_migrate(&admin, &0),
        Err(Ok(OracleError::AlreadyMigrated))
    );

    let legacy_exists = e.as_contract(&contract_id, || {
        e.storage().instance().has(&DataKey::MaxStalenessSeconds)
    });
    assert!(!legacy_exists);
}

/// Test set_admin functionality
#[test]
fn test_set_admin() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    // Verify current admin
    assert_eq!(client.get_admin(), admin);

    // Attacker cannot set admin
    assert_eq!(
        client.try_set_admin(&attacker, &new_admin),
        Err(Ok(OracleError::Unauthorized))
    );

    // Admin can transfer to new admin
    client.set_admin(&admin, &new_admin);
    assert_eq!(client.get_admin(), new_admin);

    // Old admin no longer has authority
    assert_eq!(
        client.try_set_admin(&admin, &admin),
        Err(Ok(OracleError::Unauthorized))
    );

    // New admin has authority
    let another_admin = Address::generate(&e);
    client.set_admin(&new_admin, &another_admin);
    assert_eq!(client.get_admin(), another_admin);
}

/// Test require_admin panic path (unauthorized caller without result handling)
#[test]
#[should_panic(expected = "Unauthorized: admin only")]
fn test_require_admin_panic() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let oracle = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    // This will trigger the panic path in require_admin (not the result-returning version)
    client.add_oracle(&attacker, &oracle);
}

/// Test legacy staleness key is preserved when it exists
#[test]
fn test_legacy_staleness_key_preserved() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    // Simulate the legacy key existing (pre-v1 state)
    e.as_contract(&contract_id, || {
        e.storage()
            .instance()
            .set(&DataKey::MaxStalenessSeconds, &1800u64);
    });

    // Now set a new staleness value - this should preserve the legacy key
    client.set_max_staleness(&admin, &3600);

    // Verify both keys are updated
    let config = client.get_max_staleness();
    assert_eq!(config, 3600);

    // Verify legacy key is also updated
    let legacy_value: u64 = e.as_contract(&contract_id, || {
        e.storage()
            .instance()
            .get(&DataKey::MaxStalenessSeconds)
            .unwrap()
    });
    assert_eq!(legacy_value, 3600);
}

/// Test read_config fallback to legacy MaxStalenessSeconds when OracleConfig is missing
#[test]
fn test_read_config_fallback_to_legacy() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    // Initialize normally
    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    // Verify we start with default value
    assert_eq!(client.get_max_staleness(), 3600);

    // Now simulate a state where OracleConfig is removed but legacy key exists with different value
    e.as_contract(&contract_id, || {
        e.storage().instance().remove(&DataKey::OracleConfig);
        e.storage()
            .instance()
            .set(&DataKey::MaxStalenessSeconds, &900u64);
    });

    // read_config should fall back to the legacy key
    let config_value = client.get_max_staleness();
    assert_eq!(config_value, 900);
}

/// Test migration path where OracleConfig already exists (from_version 0 with existing config)
#[test]
fn test_migrate_with_existing_oracle_config() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    // Set a custom staleness value
    client.set_max_staleness(&admin, &7200);
    assert_eq!(client.get_max_staleness(), 7200);

    // Simulate legacy layout (version 0) but WITH OracleConfig already set (not just MaxStalenessSeconds)
    e.as_contract(&contract_id, || {
        e.storage().instance().remove(&DataKey::Version);
        // Note: we keep OracleConfig set (simulating the case where it exists)
        // This tests the branch where existing config is read
    });

    // Migration should preserve the OracleConfig value
    assert_eq!(client.try_migrate(&admin, &0), Ok(Ok(())));
    assert_eq!(client.get_version(), CURRENT_VERSION);
    assert_eq!(client.get_max_staleness(), 7200);
}

// ============================================================================
// COMPREHENSIVE STALENESS TESTS
// ============================================================================

/// Test staleness with very small window (1 second)
#[test]
fn test_staleness_very_small_window() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &1000_0000000, &8);

    // Set staleness to 1 second
    client.set_max_staleness(&admin, &1);

    // Price should be valid immediately
    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 1000_0000000);

    // Advance by 1 second - still valid (exact boundary)
    e.ledger().with_mut(|li| {
        li.timestamp += 1;
    });
    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 1000_0000000);

    // Advance by 1 more second - now stale
    e.ledger().with_mut(|li| {
        li.timestamp += 1;
    });
    assert_eq!(
        client.try_get_price_valid(&asset, &None),
        Err(Ok(OracleError::StalePrice))
    );
}

/// Test staleness with various override values
#[test]
fn test_staleness_override_edge_cases() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &500_0000000, &8);

    // Default staleness is 3600 seconds
    // Override with 0 seconds at exact timestamp - price is still valid (not stale)
    // because updated_at == current timestamp, so now - updated_at = 0 which is not > 0
    let data = client.get_price_valid(&asset, &Some(0));
    assert_eq!(data.price, 500_0000000);

    // Override with u64::MAX - effectively never stale
    let data = client.get_price_valid(&asset, &Some(u64::MAX));
    assert_eq!(data.price, 500_0000000);

    // Advance time by a large amount
    e.ledger().with_mut(|li| {
        li.timestamp += 1_000_000;
    });

    // With u64::MAX override, still not stale
    let data = client.get_price_valid(&asset, &Some(u64::MAX));
    assert_eq!(data.price, 500_0000000);

    // With override of 0 seconds, now it's stale because now - updated_at > 0
    assert_eq!(
        client.try_get_price_valid(&asset, &Some(0)),
        Err(Ok(OracleError::StalePrice))
    );

    // Without override, it's also stale
    assert_eq!(
        client.try_get_price_valid(&asset, &None),
        Err(Ok(OracleError::StalePrice))
    );
}

/// Test staleness boundary exactly at threshold
#[test]
fn test_staleness_exact_boundary_threshold() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // Set specific staleness window
    client.set_max_staleness(&admin, &100);

    client.set_price(&oracle, &asset, &1234_0000000, &8);

    // At exact staleness boundary (100 seconds), should be valid
    e.ledger().with_mut(|li| {
        li.timestamp += 100;
    });
    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 1234_0000000);

    // One second past boundary, should be stale
    e.ledger().with_mut(|li| {
        li.timestamp += 1;
    });
    assert_eq!(
        client.try_get_price_valid(&asset, &None),
        Err(Ok(OracleError::StalePrice))
    );
}

/// Test multiple price updates and staleness tracking
#[test]
fn test_staleness_multiple_updates() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // First price update
    client.set_price(&oracle, &asset, &100_0000000, &8);
    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 100_0000000);

    // Advance partially
    e.ledger().with_mut(|li| {
        li.timestamp += 1800;
    });

    // Second price update - refreshes timestamp
    client.set_price(&oracle, &asset, &200_0000000, &8);
    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 200_0000000);

    // Advance partially again - still valid due to fresh update
    e.ledger().with_mut(|li| {
        li.timestamp += 1800;
    });
    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 200_0000000);

    // Advance past staleness from second update
    e.ledger().with_mut(|li| {
        li.timestamp += 2000;
    });
    assert_eq!(
        client.try_get_price_valid(&asset, &None),
        Err(Ok(OracleError::StalePrice))
    );
}

/// Test staleness with very large timestamp values
#[test]
fn test_staleness_large_timestamps() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
        // Set a large initial timestamp
        e.storage().instance().set(
            &DataKey::Price(asset.clone()),
            &PriceData {
                price: 9999_0000000,
                updated_at: 1_000_000_000u64,
                decimals: 8,
            },
        );
    });

    // Current timestamp is less than updated_at - future-dated price
    e.ledger().with_mut(|li| {
        li.timestamp = 1_000_000_000u64 - 1;
    });

    assert_eq!(
        client.try_get_price_valid(&asset, &None),
        Err(Ok(OracleError::StalePrice))
    );

    // Now set current equal to updated_at
    e.ledger().with_mut(|li| {
        li.timestamp = 1_000_000_000u64;
    });
    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 9999_0000000);
}

// ============================================================================
// COMPREHENSIVE DECIMALS TESTS
// ============================================================================

/// Test various decimal values
#[test]
fn test_decimals_various_values() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // Test decimals = 0
    client.set_price(&oracle, &asset, &12345, &0);
    let data = client.get_price(&asset);
    assert_eq!(data.price, 12345);
    assert_eq!(data.decimals, 0);

    // Test decimals = 6 (common for stablecoins)
    client.set_price(&oracle, &asset, &1_000000, &6);
    let data = client.get_price(&asset);
    assert_eq!(data.price, 1_000000);
    assert_eq!(data.decimals, 6);

    // Test decimals = 8 (common for BTC)
    client.set_price(&oracle, &asset, &50000_00000000, &8);
    let data = client.get_price(&asset);
    assert_eq!(data.price, 50000_00000000);
    assert_eq!(data.decimals, 8);

    // Test decimals = 18 (common for ETH)
    client.set_price(&oracle, &asset, &3000_000000000000000000, &18);
    let data = client.get_price(&asset);
    assert_eq!(data.price, 3000_000000000000000000);
    assert_eq!(data.decimals, 18);

    // Test high decimals = 30
    client.set_price(&oracle, &asset, &1_000000000000000000000000000000, &30);
    let data = client.get_price(&asset);
    assert_eq!(data.decimals, 30);
}

/// Test decimals consistency across get_price and get_price_valid
#[test]
fn test_decimals_consistency() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &123456789_00000000, &8);

    let data_raw = client.get_price(&asset);
    let data_valid = client.get_price_valid(&asset, &None);

    assert_eq!(data_raw.price, data_valid.price);
    assert_eq!(data_raw.decimals, data_valid.decimals);
    assert_eq!(data_raw.updated_at, data_valid.updated_at);
    assert_eq!(data_raw.decimals, 8);
}

/// Test decimals change on price update
#[test]
fn test_decimals_change_on_update() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // Initial price with 8 decimals
    client.set_price(&oracle, &asset, &100_00000000, &8);
    let data = client.get_price(&asset);
    assert_eq!(data.decimals, 8);

    // Update with different decimals
    client.set_price(&oracle, &asset, &100_000000, &6);
    let data = client.get_price(&asset);
    assert_eq!(data.decimals, 6);
    assert_eq!(data.price, 100_000000);
}

// ============================================================================
// COMPREHENSIVE ZERO/NEGATIVE PRICE REJECTION TESTS
// ============================================================================

/// Test zero price acceptance (zero is non-negative, so it's allowed)
#[test]
fn test_zero_price_accepted() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // Zero price should be accepted (non-negative)
    client.set_price(&oracle, &asset, &0, &8);
    let data = client.get_price(&asset);
    assert_eq!(data.price, 0);
    assert_eq!(data.decimals, 8);
}

/// Test negative price rejection
#[test]
#[should_panic(expected = "Invalid amount")]
fn test_negative_price_rejection() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &-100, &8);
}

/// Test various small positive prices are accepted
#[test]
fn test_small_positive_prices_accepted() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // Price of 1
    client.set_price(&oracle, &asset, &1, &0);
    let data = client.get_price(&asset);
    assert_eq!(data.price, 1);

    // Very small price with high decimals
    client.set_price(&oracle, &asset, &1, &18);
    let data = client.get_price(&asset);
    assert_eq!(data.price, 1);
}

/// Test large positive prices are accepted
#[test]
fn test_large_positive_prices_accepted() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // Large price near i128::MAX (leave some headroom)
    let large_price: i128 = 170_000_000_000_000_000_000_000_000_000_000_000_000i128;
    client.set_price(&oracle, &asset, &large_price, &18);
    let data = client.get_price(&asset);
    assert_eq!(data.price, large_price);
}

/// Test that negative price in storage returns InvalidPrice error from get_price_valid
#[test]
fn test_negative_price_in_storage_returns_error() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        // Directly set a negative price in storage (simulating potential corruption)
        e.storage().instance().set(
            &DataKey::Price(asset.clone()),
            &PriceData {
                price: -100,
                updated_at: e.ledger().timestamp(),
                decimals: 8,
            },
        );
    });

    // get_price should return the negative value (raw read)
    let data = client.get_price(&asset);
    assert_eq!(data.price, -100);

    // get_price_valid should reject it
    assert_eq!(
        client.try_get_price_valid(&asset, &None),
        Err(Ok(OracleError::InvalidPrice))
    );
}

/// Test edge case: price of 1 is accepted
#[test]
fn test_price_one_accepted() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &1, &8);
    let data = client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 1);
}

/// Test multiple negative price values are all rejected
#[test]
#[should_panic(expected = "Invalid amount")]
fn test_various_negative_prices_rejected_1() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &-1, &8);
}

#[test]
#[should_panic(expected = "Invalid amount")]
fn test_various_negative_prices_rejected_2() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &-999999999999999999i128, &8);
}

#[test]
#[should_panic(expected = "Invalid amount")]
fn test_various_negative_prices_rejected_3() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    client.set_price(&oracle, &asset, &i128::MIN, &8);
}
