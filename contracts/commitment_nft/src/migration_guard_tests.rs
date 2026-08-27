#![cfg(test)]

//! Migration fixtures for the commitment NFT storage schema.
//!
//! These tests intentionally write the old storage layout directly. A normal
//! `initialize` call represents a new deployment and therefore cannot exercise
//! the layout that an already-deployed v1 contract presents to a new WASM.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, Vec};

struct Fixture {
    env: Env,
    client: CommitmentNFTContractClient<'static>,
    contract_id: Address,
    admin: Address,
}

fn legacy_fixture(version: Option<u32>, persistent_index: bool) -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenCounter, &2u32);
        let mut token_ids = Vec::new(&env);
        token_ids.push_back(7);
        token_ids.push_back(11);
        if persistent_index {
            env.storage()
                .persistent()
                .set(&DataKey::TokenIds, &token_ids);
        } else {
            env.storage().instance().set(&DataKey::TokenIds, &token_ids);
        }
        if let Some(version) = version {
            env.storage().instance().set(&DataKey::Version, &version);
        }
    });

    Fixture {
        env,
        client,
        contract_id,
        admin,
    }
}

fn add_existing_nft(fixture: &Fixture) {
    let owner = Address::generate(&fixture.env);
    let asset = Address::generate(&fixture.env);
    let metadata = CommitmentMetadata {
        commitment_id: soroban_sdk::String::from_str(&fixture.env, "legacy-7"),
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: soroban_sdk::String::from_str(&fixture.env, "safe"),
        created_at: 100,
        expires_at: 2_592_100,
        initial_amount: 450,
        asset_address: asset,
        early_exit_penalty: 5,
    };
    let nft = CommitmentNFT {
        owner: owner.clone(),
        token_id: 7,
        metadata,
        is_active: true,
        early_exit_penalty: 5,
    };
    fixture.env.as_contract(&fixture.contract_id, || {
        fixture
            .env
            .storage()
            .persistent()
            .set(&DataKey::NFT(7), &nft);
        fixture
            .env
            .storage()
            .persistent()
            .set(&DataKey::OwnerBalance(owner.clone()), &1u32);
        let mut owner_tokens = Vec::new(&fixture.env);
        owner_tokens.push_back(7);
        fixture
            .env
            .storage()
            .persistent()
            .set(&DataKey::OwnerTokens(owner), &owner_tokens);
    });
}

#[test]
fn fresh_initialization_records_current_schema_version() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    assert_eq!(client.get_version(), CURRENT_VERSION);
}

#[test]
fn v1_instance_index_is_copied_without_changing_existing_nft_data() {
    let fixture = legacy_fixture(Some(1), false);
    add_existing_nft(&fixture);

    assert_eq!(
        fixture.client.balance_of(&Address::generate(&fixture.env)),
        0
    );
    fixture.client.migrate(&fixture.admin, &1);

    assert_eq!(fixture.client.get_version(), CURRENT_VERSION);
    assert_eq!(fixture.client.total_supply(), 2);
    fixture.env.as_contract(&fixture.contract_id, || {
        let token_ids: Vec<u32> = fixture
            .env
            .storage()
            .persistent()
            .get(&DataKey::TokenIds)
            .unwrap();
        assert_eq!(token_ids.len(), 2);
        let nft: CommitmentNFT = fixture
            .env
            .storage()
            .persistent()
            .get(&DataKey::NFT(7))
            .unwrap();
        assert_eq!(nft.metadata.initial_amount, 450);
        assert!(nft.is_active);
    });
}

#[test]
fn v0_legacy_deployment_migrates_from_missing_version_marker() {
    let fixture = legacy_fixture(None, false);

    assert_eq!(fixture.client.get_version(), 0);
    fixture.client.migrate(&fixture.admin, &0);
    assert_eq!(fixture.client.get_version(), CURRENT_VERSION);
}

#[test]
fn already_persistent_index_is_preserved_during_migration() {
    let fixture = legacy_fixture(Some(1), true);

    fixture.client.migrate(&fixture.admin, &1);
    fixture.env.as_contract(&fixture.contract_id, || {
        let token_ids: Vec<u32> = fixture
            .env
            .storage()
            .persistent()
            .get(&DataKey::TokenIds)
            .unwrap();
        assert_eq!(token_ids.len(), 2);
    });
    assert_eq!(fixture.client.get_version(), CURRENT_VERSION);
}

#[test]
fn migration_is_idempotence_guarded_by_schema_version() {
    let fixture = legacy_fixture(Some(1), false);

    fixture.client.migrate(&fixture.admin, &1);
    assert_eq!(
        fixture.client.try_migrate(&fixture.admin, &1),
        Err(Ok(ContractError::AlreadyMigrated))
    );
}

#[test]
fn unsupported_stored_version_fails_before_any_migration_write() {
    let fixture = legacy_fixture(Some(CURRENT_VERSION + 1), false);

    assert_eq!(
        fixture
            .client
            .try_migrate(&fixture.admin, &(CURRENT_VERSION + 1)),
        Err(Ok(ContractError::UnsupportedStorageVersion))
    );
    assert_eq!(fixture.client.get_version(), CURRENT_VERSION + 1);
}

#[test]
fn mismatched_source_version_fails_without_marking_migration_complete() {
    let fixture = legacy_fixture(Some(1), false);

    assert_eq!(
        fixture.client.try_migrate(&fixture.admin, &0),
        Err(Ok(ContractError::InvalidVersion))
    );
    assert_eq!(fixture.client.get_version(), 1);
}

#[test]
fn missing_counter_is_rejected_before_partial_state_is_created() {
    let fixture = legacy_fixture(Some(1), false);
    fixture.env.as_contract(&fixture.contract_id, || {
        fixture
            .env
            .storage()
            .instance()
            .remove(&DataKey::TokenCounter);
    });

    assert_eq!(
        fixture.client.try_migrate(&fixture.admin, &1),
        Err(Ok(ContractError::MigrationSchemaMismatch))
    );
    assert_eq!(fixture.client.get_version(), 1);
    fixture.env.as_contract(&fixture.contract_id, || {
        assert!(!fixture
            .env
            .storage()
            .instance()
            .has(&DataKey::ReentrancyGuard));
    });
}

#[test]
fn missing_token_index_is_rejected_without_fabricating_empty_history() {
    let fixture = legacy_fixture(Some(1), false);
    fixture.env.as_contract(&fixture.contract_id, || {
        fixture.env.storage().instance().remove(&DataKey::TokenIds);
    });

    assert_eq!(
        fixture.client.try_migrate(&fixture.admin, &1),
        Err(Ok(ContractError::MigrationSchemaMismatch))
    );
    fixture.env.as_contract(&fixture.contract_id, || {
        assert!(!fixture.env.storage().persistent().has(&DataKey::TokenIds));
    });
}

#[test]
fn unauthorized_migration_cannot_change_schema_version() {
    let fixture = legacy_fixture(Some(1), false);
    let attacker = Address::generate(&fixture.env);

    assert_eq!(
        fixture.client.try_migrate(&attacker, &1),
        Err(Ok(ContractError::NotAuthorized))
    );
    assert_eq!(fixture.client.get_version(), 1);
}

#[test]
fn partial_source_fixture_with_only_admin_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Admin, &admin);
    });

    assert_eq!(
        client.try_migrate(&admin, &0),
        Err(Ok(ContractError::MigrationSchemaMismatch))
    );
    assert_eq!(client.get_version(), 0);
}
