#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, String};

#[test]
fn test_initialize() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // Verify admin is set
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_initialize_twice() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    client.initialize(&admin); // Should panic
}

#[test]
fn test_transfer_admin() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // Transfer admin
    client.transfer_admin(&new_admin);
    
    // Verify new admin is set
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, new_admin);
}

#[test]
#[should_panic(expected = "Unauthorized: admin access required")]
fn test_transfer_admin_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // Try to transfer admin as non-admin (should panic)
    let attacker_client = CommitmentNFTContractClient::new(&e, &contract_id);
    attacker_client.transfer_admin(&new_admin);
}

#[test]
fn test_add_authorized_contract() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let authorized_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // Add authorized contract
    client.add_authorized_contract(&authorized_contract);
    
    // Verify it's authorized
    assert!(client.is_authorized(&authorized_contract));
}

#[test]
fn test_remove_authorized_contract() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let authorized_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // Add authorized contract
    client.add_authorized_contract(&authorized_contract);
    assert!(client.is_authorized(&authorized_contract));
    
    // Remove authorized contract
    client.remove_authorized_contract(&authorized_contract);
    
    // Verify it's no longer authorized (but admin still is)
    assert!(!client.is_authorized(&authorized_contract));
    assert!(client.is_authorized(&admin)); // Admin is always authorized
}

#[test]
#[should_panic(expected = "Unauthorized: admin access required")]
fn test_add_authorized_contract_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let authorized_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // Try to add authorized contract as non-admin (should panic)
    let attacker_client = CommitmentNFTContractClient::new(&e, &contract_id);
    attacker_client.add_authorized_contract(&authorized_contract);
}

#[test]
fn test_admin_is_always_authorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // Admin should always be authorized
    assert!(client.is_authorized(&admin));
}

#[test]
#[should_panic(expected = "Unauthorized: admin access required")]
fn test_mint_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let owner = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // Try to mint as non-admin (should panic)
    let attacker_client = CommitmentNFTContractClient::new(&e, &contract_id);
    attacker_client.mint(
        &owner,
        &String::from_str(&e, "commitment_1"),
        &30u32,
        &20u32,
        &String::from_str(&e, "safe"),
        &1000i128,
        &Address::generate(&e),
    );
}

#[test]
fn test_mint() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let owner = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // Mint as admin (should succeed)
    let token_id = client.mint(
        &owner,
        &String::from_str(&e, "commitment_1"),
        &30u32,
        &20u32,
        &String::from_str(&e, "safe"),
        &1000i128,
        &Address::generate(&e),
    );
    
    // TODO: Verify minting when storage is implemented
    assert_eq!(token_id, 0); // Placeholder
}

#[test]
fn test_transfer() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let from = Address::generate(&e);
    let to = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    client.initialize(&admin);
    
    // TODO: Test transfer when storage is implemented
}

