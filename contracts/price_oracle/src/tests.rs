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

// ============================================================================
// Oracle Consumer Expectations Tests for commitment_core/marketplace
// ============================================================================

#[test]
fn test_get_price_for_commitment_fresh() {
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
    
    // Should succeed with fresh price (within 5 minutes)
    let data = client.get_price_for_commitment(&asset, &Some(10)); // 10% max variation
    assert_eq!(data.price, 1000_00000000);
    assert_eq!(data.decimals, 8);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // StalePrice
fn test_get_price_for_commitment_stale() {
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
    
    // Advance time past 5 minutes (300 seconds)
    e.ledger().with_mut(|li| {
        li.timestamp += 301;
    });

    // Should fail due to staleness
    let _ = client.get_price_for_commitment(&asset, &Some(10));
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // StalePrice (reused for variation)
fn test_get_price_for_commitment_excessive_variation() {
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

    // Set initial price
    client.set_price(&oracle, &asset, &1000_00000000, &8);
    
    // Advance time a bit and set new price with >20% variation
    e.ledger().with_mut(|li| {
        li.timestamp += 60;
    });
    client.set_price(&oracle, &asset, &1250_00000000, &8); // 25% increase
    
    // Should fail due to excessive variation
    let _ = client.get_price_for_commitment(&asset, &Some(20)); // 20% max variation
}

#[test]
fn test_get_price_for_marketplace_valid() {
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

    client.set_price(&oracle, &asset, &50_00000000, &8); // $50 USD in 8 decimals
    
    // Should succeed with price above minimum
    let data = client.get_price_for_marketplace(&asset, &Some(10_00000000)); // $10 minimum
    assert_eq!(data.price, 50_00000000);
    assert_eq!(data.decimals, 8);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")] // InvalidPrice
fn test_get_price_for_marketplace_below_minimum() {
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

    client.set_price(&oracle, &asset, &5_00000000, &8); // $5 USD in 8 decimals
    
    // Should fail due to price below minimum
    let _ = client.get_price_for_marketplace(&asset, &Some(10_00000000)); // $10 minimum
}

#[test]
fn test_get_price_for_marketplace_different_decimals() {
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

    // Set price in 6 decimals ($100 USD = 100_000000)
    client.set_price(&oracle, &asset, &100_000000, &6);
    
    // Should succeed when converting to 8 decimals for minimum check
    let data = client.get_price_for_marketplace(&asset, &Some(50_00000000)); // $50 minimum
    assert_eq!(data.price, 100_000000);
    assert_eq!(data.decimals, 6);
}

#[test]
fn test_get_batch_prices_success() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset1 = Address::generate(&e);
    let asset2 = Address::generate(&e);
    let asset3 = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // Set prices for multiple assets
    client.set_price(&oracle, &asset1, &1000_00000000, &8);
    client.set_price(&oracle, &asset2, &2000_00000000, &8);
    client.set_price(&oracle, &asset3, &500_00000000, &6);

    let mut assets = Vec::new(&e);
    assets.push_back(asset1.clone());
    assets.push_back(asset2.clone());
    assets.push_back(asset3.clone());

    // Should succeed with all fresh prices
    let results = client.get_batch_prices(&assets, &600); // 10 minutes max staleness
    assert_eq!(results.len(), 3);
    
    // Verify results
    for (asset, data) in results.iter() {
        if *asset == asset1 {
            assert_eq!(data.price, 1000_00000000);
            assert_eq!(data.decimals, 8);
        } else if *asset == asset2 {
            assert_eq!(data.price, 2000_00000000);
            assert_eq!(data.decimals, 8);
        } else if *asset == asset3 {
            assert_eq!(data.price, 500_00000000);
            assert_eq!(data.decimals, 6);
        }
    }
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // StalePrice
fn test_get_batch_prices_one_stale() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let asset1 = Address::generate(&e);
    let asset2 = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // Set prices
    client.set_price(&oracle, &asset1, &1000_00000000, &8);
    client.set_price(&oracle, &asset2, &2000_00000000, &8);
    
    // Advance time and update only one price
    e.ledger().with_mut(|li| {
        li.timestamp += 120; // 2 minutes
    });
    client.set_price(&oracle, &asset2, &2100_00000000, &8);
    
    // Advance time past batch staleness limit
    e.ledger().with_mut(|li| {
        li.timestamp += 500; // Total > 10 minutes
    });

    let mut assets = Vec::new(&e);
    assets.push_back(asset1);
    assets.push_back(asset2);

    // Should fail because asset1 is stale
    let _ = client.get_batch_prices(&assets, &600); // 10 minutes max staleness
}

#[test]
fn test_get_price_for_high_value_operation_normal_value() {
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
    
    // Normal value operation ($50 USD) should use 15-minute staleness
    let data = client.get_price_for_high_value_operation(
        &asset, 
        &50_00000000, // $50 USD in 8 decimals
        &10 // 10% max deviation
    );
    assert_eq!(data.price, 1000_00000000);
}

#[test]
fn test_get_price_for_high_value_operation_high_value() {
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
    
    // High value operation ($2000 USD) should use 5-minute staleness
    let data = client.get_price_for_high_value_operation(
        &asset, 
        &200_000_000_000, // $2000 USD in 8 decimals
        &10 // 10% max deviation
    );
    assert_eq!(data.price, 1000_00000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // StalePrice
fn test_get_price_for_high_value_operation_very_high_value_stale() {
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
    
    // Advance time past 1 minute (very high value requires 1-minute freshness)
    e.ledger().with_mut(|li| {
        li.timestamp += 61;
    });

    // Very high value operation ($20000 USD) should fail with stale price
    let _ = client.get_price_for_high_value_operation(
        &asset, 
        &2_000_000_000_000, // $20000 USD in 8 decimals
        &10 // 10% max deviation
    );
}

#[test]
fn test_get_oracle_health() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
    });

    // Should return healthy status
    let health = client.get_oracle_health();
    assert!(health.is_healthy);
    assert_eq!(health.max_staleness_seconds, 3600); // Default value
    assert!(health.last_check > 0);
    assert_eq!(health.active_oracles_count, 0); // Not tracked in current implementation
}

#[test]
fn test_oracle_consumer_functions_edge_cases() {
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

    // Test zero price rejection
    client.set_price(&oracle, &asset, &0, &8);
    let result = client.try_get_price_for_commitment(&asset, &Some(10));
    assert_eq!(result, Err(Ok(OracleError::InvalidPrice)));

    // Test negative price rejection
    client.set_price(&oracle, &asset, &-1000, &8);
    let result = client.try_get_price_for_commitment(&asset, &Some(10));
    assert_eq!(result, Err(Ok(OracleError::InvalidPrice)));

    // Test invalid variation percentage
    client.set_price(&oracle, &asset, &1000_00000000, &8);
    let result = client.try_get_price_for_commitment(&asset, &Some(150)); // > 100%
    assert_eq!(result, Err(Ok(OracleError::StalePrice))); // Reused error

    // Test invalid minimum price
    client.set_price(&oracle, &asset, &1000_00000000, &8);
    let result = client.try_get_price_for_marketplace(&asset, &Some(-1000)); // Negative minimum
    assert_eq!(result, Err(Ok(OracleError::InvalidPrice)));
}

#[test]
fn test_oracle_consumer_integration_scenario() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let oracle = Address::generate(&e);
    let usdc_asset = Address::generate(&e);
    let eth_asset = Address::generate(&e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(&e, &contract_id);

    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle.clone()).unwrap();
    });

    // Set realistic prices
    client.set_price(&oracle, &usdc_asset, &1_00000000, &8); // $1 USDC
    client.set_price(&oracle, &eth_asset, &3000_00000000, &8); // $3000 ETH

    // Simulate commitment_core operation - high value commitment
    let commitment_value = 500_000_000_000; // $5000 USD
    let eth_price = client.get_price_for_high_value_operation(
        &eth_asset,
        &commitment_value,
        &5 // 5% max deviation
    );
    assert_eq!(eth_price.price, 3000_00000000);

    // Simulate marketplace listing with minimum price
    let usdc_price = client.get_price_for_marketplace(
        &usdc_asset,
        &Some(100_000000) // $1 minimum
    );
    assert_eq!(usdc_price.price, 1_00000000);

    // Batch price update for portfolio valuation
    let mut assets = Vec::new(&e);
    assets.push_back(usdc_asset.clone());
    assets.push_back(eth_asset.clone());
    
    let portfolio_prices = client.get_batch_prices(&assets, &300); // 5 minutes
    assert_eq!(portfolio_prices.len(), 2);

    // Check oracle health before critical operation
    let health = client.get_oracle_health();
    assert!(health.is_healthy);
}
