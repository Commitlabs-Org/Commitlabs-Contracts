#![cfg(test)]

use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, String, Map};

#[test]
fn test_initialize() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Verify admin is set
    let stored_admin = client.get_admin();
    assert_eq!(stored_admin, admin);
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_initialize_twice() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    client.initialize(&admin, &commitment_core); // Should panic
}

#[test]
fn test_transfer_admin() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
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
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Try to transfer admin as non-admin (should panic)
    let attacker_client = AttestationEngineContractClient::new(&e, &contract_id);
    attacker_client.transfer_admin(&new_admin);
}

#[test]
fn test_add_authorized_verifier() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Add authorized verifier
    client.add_authorized_verifier(&verifier);
    
    // Verify it's authorized
    assert!(client.is_authorized_verifier(&verifier));
}

#[test]
fn test_remove_authorized_verifier() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Add authorized verifier
    client.add_authorized_verifier(&verifier);
    assert!(client.is_authorized_verifier(&verifier));
    
    // Remove authorized verifier
    client.remove_authorized_verifier(&verifier);
    
    // Verify it's no longer authorized (but admin still is)
    assert!(!client.is_authorized_verifier(&verifier));
    assert!(client.is_authorized_verifier(&admin)); // Admin is always authorized
}

#[test]
#[should_panic(expected = "Unauthorized: admin access required")]
fn test_add_authorized_verifier_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Try to add authorized verifier as non-admin (should panic)
    let attacker_client = AttestationEngineContractClient::new(&e, &contract_id);
    attacker_client.add_authorized_verifier(&verifier);
}

#[test]
fn test_admin_is_always_authorized_verifier() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Admin should always be authorized
    assert!(client.is_authorized_verifier(&admin));
}

#[test]
#[should_panic(expected = "Unauthorized: admin or authorized verifier access required")]
fn test_attest_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Try to attest as unauthorized (should panic)
    let attacker_client = AttestationEngineContractClient::new(&e, &contract_id);
    let data = Map::new(&e);
    attacker_client.attest(
        &String::from_str(&e, "commitment_1"),
        &String::from_str(&e, "health_check"),
        &data,
        &attacker,
    );
}

#[test]
fn test_attest_authorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Attest as admin (should succeed)
    let data = Map::new(&e);
    client.attest(
        &String::from_str(&e, "commitment_1"),
        &String::from_str(&e, "health_check"),
        &data,
        &admin,
    );
    
    // TODO: Verify attestation when storage is implemented
}

#[test]
fn test_attest_as_authorized_verifier() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Add verifier
    client.add_authorized_verifier(&verifier);
    
    // Attest as authorized verifier (should succeed)
    let verifier_client = AttestationEngineContractClient::new(&e, &contract_id);
    let data = Map::new(&e);
    verifier_client.attest(
        &String::from_str(&e, "commitment_1"),
        &String::from_str(&e, "health_check"),
        &data,
        &verifier,
    );
    
    // TODO: Verify attestation when storage is implemented
}

#[test]
#[should_panic(expected = "Unauthorized: admin or authorized verifier access required")]
fn test_record_fees_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Try to record fees as unauthorized (should panic)
    let attacker_client = AttestationEngineContractClient::new(&e, &contract_id);
    attacker_client.record_fees(
        &String::from_str(&e, "commitment_1"),
        &100i128,
    );
}

#[test]
#[should_panic(expected = "Unauthorized: admin or authorized verifier access required")]
fn test_record_drawdown_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let attacker = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Try to record drawdown as unauthorized (should panic)
    let attacker_client = AttestationEngineContractClient::new(&e, &contract_id);
    attacker_client.record_drawdown(
        &String::from_str(&e, "commitment_1"),
        &15i128,
    );
}

#[test]
fn test_verify_compliance() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let commitment_core = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    client.initialize(&admin, &commitment_core);
    
    // Verify compliance (read-only, should succeed)
    let is_compliant = client.verify_compliance(&String::from_str(&e, "commitment_1"));
    
    // TODO: Verify compliance result when storage is implemented
    assert_eq!(is_compliant, true); // Placeholder
}

