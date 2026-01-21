#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, String};

#[test]
fn test_initialize() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Verify admin is set
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_initialize_twice() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    client.initialize(&admin, &nft_contract); // Should panic
}

#[test]
fn test_transfer_admin() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
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
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Try to transfer admin as non-admin (should panic)
    let attacker_client = CommitmentCoreContractClient::new(&e, &contract_id);
    attacker_client.transfer_admin(&new_admin);
}

#[test]
fn test_add_authorized_allocator() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let allocator = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Add authorized allocator
    client.add_authorized_allocator(&allocator);
    
    // Verify it's authorized
    assert!(client.is_authorized_allocator(&allocator));
}

#[test]
fn test_remove_authorized_allocator() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let allocator = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Add authorized allocator
    client.add_authorized_allocator(&allocator);
    assert!(client.is_authorized_allocator(&allocator));
    
    // Remove authorized allocator
    client.remove_authorized_allocator(&allocator);
    
    // Verify it's no longer authorized (but admin still is)
    assert!(!client.is_authorized_allocator(&allocator));
    assert!(client.is_authorized_allocator(&admin)); // Admin is always authorized
}

#[test]
#[should_panic(expected = "Unauthorized: admin access required")]
fn test_add_authorized_allocator_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let allocator = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Try to add authorized allocator as non-admin (should panic)
    let attacker_client = CommitmentCoreContractClient::new(&e, &contract_id);
    attacker_client.add_authorized_allocator(&allocator);
}

#[test]
fn test_admin_is_always_authorized_allocator() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Admin should always be authorized
    assert!(client.is_authorized_allocator(&admin));
}

#[test]
#[should_panic(expected = "Unauthorized: admin or authorized allocator access required")]
fn test_update_value_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Try to update value as unauthorized (should panic)
    let attacker_client = CommitmentCoreContractClient::new(&e, &contract_id);
    attacker_client.update_value(
        &String::from_str(&e, "commitment_1"),
        &1500i128,
    );
}

#[test]
fn test_update_value_authorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Update value as admin (should succeed)
    client.update_value(
        &String::from_str(&e, "commitment_1"),
        &1500i128,
    );
    
    // TODO: Verify value update when storage is implemented
}

#[test]
#[should_panic(expected = "Unauthorized: admin or authorized allocator access required")]
fn test_allocate_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Try to allocate as unauthorized (should panic)
    let attacker_client = CommitmentCoreContractClient::new(&e, &contract_id);
    attacker_client.allocate(
        &String::from_str(&e, "commitment_1"),
        &Address::generate(&e),
        &500i128,
    );
}

#[test]
fn test_allocate_authorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let allocator = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Add allocator
    client.add_authorized_allocator(&allocator);
    
    // Allocate as authorized allocator (should succeed)
    let allocator_client = CommitmentCoreContractClient::new(&e, &contract_id);
    allocator_client.allocate(
        &String::from_str(&e, "commitment_1"),
        &Address::generate(&e),
        &500i128,
    );
    
    // TODO: Verify allocation when storage is implemented
}

#[test]
fn test_create_commitment() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let owner = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // TODO: Test commitment creation when storage is implemented
}

#[test]
fn test_settle() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.initialize(&admin, &nft_contract);
    
    // Settle as admin (should succeed)
    client.settle(&String::from_str(&e, "commitment_1"));
    
    // TODO: Verify settlement when storage is implemented
}

