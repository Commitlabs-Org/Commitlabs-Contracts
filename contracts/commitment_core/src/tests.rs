#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup_env() -> (Env, Address, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    (e, contract_id, admin, nft_contract)
}

#[test]
fn test_initialize() {
    let (e, contract_id, admin, nft_contract) = setup_env();
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    
    let result = client.initialize(&admin, &nft_contract);
    assert!(result.is_ok());
}

#[test]
fn test_initialize_twice_fails() {
    let (e, contract_id, admin, nft_contract) = setup_env();
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &nft_contract);
    let result = client.try_initialize(&admin, &nft_contract);
    assert!(result.is_err());
}

// Access Control Tests
#[test]
fn test_add_authorized_allocator() {
    let (e, contract_id, admin, nft_contract) = setup_env();
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let allocator = Address::generate(&e);

    client.initialize(&admin, &nft_contract);
    client.add_authorized_allocator(&admin, &allocator);

    assert!(client.is_authorized_allocator(&allocator));
}

#[test]
fn test_add_authorized_allocator_unauthorized_fails() {
    let (e, contract_id, admin, nft_contract) = setup_env();
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let unauthorized = Address::generate(&e);
    let allocator = Address::generate(&e);

    client.initialize(&admin, &nft_contract);

    let result = client.try_add_authorized_allocator(&unauthorized, &allocator);
    assert!(result.is_err());
}

#[test]
fn test_remove_authorized_allocator() {
    let (e, contract_id, admin, nft_contract) = setup_env();
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let allocator = Address::generate(&e);

    client.initialize(&admin, &nft_contract);
    client.add_authorized_allocator(&admin, &allocator);
    assert!(client.is_authorized_allocator(&allocator));

    client.remove_authorized_allocator(&admin, &allocator);
    assert!(!client.is_authorized_allocator(&allocator));
}

#[test]
fn test_update_admin() {
    let (e, contract_id, admin, nft_contract) = setup_env();
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let new_admin = Address::generate(&e);

    client.initialize(&admin, &nft_contract);
    client.update_admin(&admin, &new_admin);

    let current_admin = client.get_admin();
    assert_eq!(current_admin, new_admin);
}

#[test]
fn test_get_admin() {
    let (e, contract_id, admin, nft_contract) = setup_env();
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    client.initialize(&admin, &nft_contract);
    let retrieved_admin = client.get_admin();
    assert_eq!(retrieved_admin, admin);
}

#[test]
fn test_create_commitment() {
    let (e, contract_id, admin, nft_contract) = setup_env();
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    // TODO: Test commitment creation when implemented
}

#[test]
fn test_settle() {
    let (e, contract_id, admin, nft_contract) = setup_env();
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &nft_contract);
    
    // TODO: Test settlement when implemented
}

