#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_env() -> (Env, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let admin = Address::generate(&e);
    (e, contract_id, admin)
}

#[test]
fn test_initialize() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    let result = client.initialize(&admin);
    assert_eq!(result, ());

    // Verify total supply is 0
    let supply = client.total_supply();
    assert_eq!(supply, 0);
}

#[test]
fn test_initialize_twice_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    client.initialize(&admin);
    let result = client.try_initialize(&admin);
    assert!(result.is_err());
}

#[test]
fn test_mint_success() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert_eq!(token_id, 1);

    // Verify ownership
    let fetched_owner = client.owner_of(&token_id);
    assert_eq!(fetched_owner, owner);

    // Verify metadata
    let metadata = client.get_metadata(&token_id);
    assert_eq!(metadata.duration_days, 30);
    assert_eq!(metadata.max_loss_percent, 10);
    assert_eq!(metadata.initial_amount, 1000);

    // Verify is_active
    let active = client.is_active(&token_id);
    assert!(active);

    // Verify total supply incremented
    let supply = client.total_supply();
    assert_eq!(supply, 1);
}

#[test]
fn test_mint_sequential_token_ids() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let token_id_1 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );
    let token_id_2 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_002"),
        &60,
        &20,
        &String::from_str(&e, "balanced"),
        &2000,
        &asset,
    );

    assert_eq!(token_id_1, 1);
    assert_eq!(token_id_2, 2);
    assert_eq!(client.total_supply(), 2);
}

#[test]
fn test_mint_unauthorized_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let unauthorized = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &unauthorized,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_authorized_contract() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let minter = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);
    client.add_authorized_contract(&admin, &minter);

    let token_id = client.mint(
        &minter,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert_eq!(token_id, 1);
}

#[test]
fn test_mint_invalid_duration_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &0, // Invalid: duration must be > 0
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_invalid_max_loss_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &101, // Invalid: max_loss must be 0-100
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_invalid_commitment_type_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "invalid_type"), // Invalid
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_invalid_amount_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &0, // Invalid: amount must be > 0
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_all_commitment_types() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    // Test "safe"
    let t1 = client.mint(
        &admin, &owner, &String::from_str(&e, "c1"),
        &30, &10, &String::from_str(&e, "safe"), &1000, &asset,
    );
    assert_eq!(t1, 1);

    // Test "balanced"
    let t2 = client.mint(
        &admin, &owner, &String::from_str(&e, "c2"),
        &30, &10, &String::from_str(&e, "balanced"), &1000, &asset,
    );
    assert_eq!(t2, 2);

    // Test "aggressive"
    let t3 = client.mint(
        &admin, &owner, &String::from_str(&e, "c3"),
        &30, &10, &String::from_str(&e, "aggressive"), &1000, &asset,
    );
    assert_eq!(t3, 3);
}

#[test]
fn test_get_metadata_not_found() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    client.initialize(&admin);

    let result = client.try_get_metadata(&999);
    assert!(result.is_err());
}

#[test]
fn test_owner_of_not_found() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    client.initialize(&admin);

    let result = client.try_owner_of(&999);
    assert!(result.is_err());
}

#[test]
fn test_transfer() {
    let (e, contract_id, _admin) = setup_env();
    let _from = Address::generate(&e);
    let _to = Address::generate(&e);
    let _client = CommitmentNFTContractClient::new(&e, &contract_id);

    // TODO: Test transfer when implemented
}

// Access Control Tests
#[test]
fn test_add_authorized_contract() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let authorized = Address::generate(&e);

    client.initialize(&admin);
    client.add_authorized_contract(&admin, &authorized);

    // Verify it's authorized
    assert!(client.is_authorized(&authorized));
}

#[test]
fn test_add_authorized_contract_unauthorized_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let unauthorized = Address::generate(&e);
    let authorized = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_add_authorized_contract(&unauthorized, &authorized);
    assert!(result.is_err());
}

#[test]
fn test_remove_authorized_contract() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let authorized = Address::generate(&e);

    client.initialize(&admin);
    client.add_authorized_contract(&admin, &authorized);
    assert!(client.is_authorized(&authorized));

    client.remove_authorized_contract(&admin, &authorized);
    // Admin is still authorized, but the contract is not
    assert!(!client.is_authorized(&authorized));
}

#[test]
fn test_update_admin() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let new_admin = Address::generate(&e);

    client.initialize(&admin);
    client.update_admin(&admin, &new_admin);

    // Verify new admin
    let current_admin = client.get_admin();
    assert_eq!(current_admin, new_admin);
}

#[test]
fn test_update_admin_unauthorized_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let unauthorized = Address::generate(&e);
    let new_admin = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_update_admin(&unauthorized, &new_admin);
    assert!(result.is_err());
}

#[test]
fn test_get_admin() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    client.initialize(&admin);
    let retrieved_admin = client.get_admin();
    assert_eq!(retrieved_admin, admin);
}

#[test]
fn test_admin_can_mint() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    // Admin should be able to mint without being explicitly authorized
    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert_eq!(token_id, 1);
}
