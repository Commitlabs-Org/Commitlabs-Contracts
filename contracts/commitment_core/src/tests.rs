#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String,
};

// Helper function to create a test commitment in storage
fn create_test_commitment(
    e: &Env,
    contract_id: &Address,
    commitment_id: String,
    owner: Address,
    initial_value: i128,
    current_value: i128,
    max_loss_percent: u32,
) {
    let rules = CommitmentRules {
        duration_days: 365,
        max_loss_percent,
        commitment_type: String::from_str(e, "balanced"),
        early_exit_penalty: 10,
        min_fee_threshold: 1000,
    };

    let commitment = Commitment {
        commitment_id: commitment_id.clone(),
        owner,
        nft_token_id: 1,
        rules,
        amount: initial_value,
        asset_address: Address::generate(e),
        created_at: 1000,
        expires_at: 1000 + (365 * 24 * 60 * 60),
        current_value,
        status: String::from_str(e, "active"),
    };

    let key = DataKey::Commitment(commitment_id);
    e.as_contract(contract_id, || {
        e.storage().persistent().set(&key, &commitment);
    });
}

#[test]
fn test_initialize() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    client.initialize(&admin, &nft_contract);

    // Test that we can add authorized updater (proves admin is stored)
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);
}

#[test]
fn test_value_update_success() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment
    let commitment_id = String::from_str(&e, "commit_1");
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 10000;
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, initial_value, current_value, 20);

    // Add authorized updater
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);

    // Update value
    let new_value = 9500; // 5% loss, within 20% limit
    e.mock_all_auths();
    client.update_value(&updater, &commitment_id, &new_value);

    // Verify value was updated
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);
    assert_eq!(commitment.status, String::from_str(&e, "active")); // Should still be active
}

#[test]
fn test_violation_detection() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment with 20% max loss
    let commitment_id = String::from_str(&e, "commit_2");
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 10000;
    let max_loss_percent = 20;
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, initial_value, current_value, max_loss_percent);

    // Add authorized updater
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);

    // Update value to trigger violation (25% loss exceeds 20% limit)
    let new_value = 7500; // 25% loss
    e.mock_all_auths();
    client.update_value(&updater, &commitment_id, &new_value);

    // Verify violation was detected
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);
    assert_eq!(commitment.status, String::from_str(&e, "violated"));
}

#[test]
fn test_violation_at_threshold() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment with 20% max loss
    let commitment_id = String::from_str(&e, "commit_3");
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 10000;
    let max_loss_percent = 20;
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, initial_value, current_value, max_loss_percent);

    // Add authorized updater
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);

    // Update value to exactly 20% loss (should NOT violate, since violation is > not >=)
    let new_value = 8000; // Exactly 20% loss
    e.mock_all_auths();
    client.update_value(&updater, &commitment_id, &new_value);

    // Verify no violation (since drawdown > max_loss, and 20% is not > 20%)
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);
    assert_eq!(commitment.status, String::from_str(&e, "active")); // Should still be active
}

#[test]
#[should_panic(expected = "Caller is not authorized")]
fn test_access_control_unauthorized() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment
    let commitment_id = String::from_str(&e, "commit_4");
    let owner = Address::generate(&e);
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, 10000, 10000, 20);

    // Try to update with unauthorized caller
    let unauthorized = Address::generate(&e);
    e.mock_all_auths();
    client.update_value(&unauthorized, &commitment_id, &9500);
}

#[test]
fn test_access_control_admin() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment
    let commitment_id = String::from_str(&e, "commit_5");
    let owner = Address::generate(&e);
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, 10000, 10000, 20);

    // Admin should be able to update without being added to whitelist
    let new_value = 9500;
    e.mock_all_auths();
    client.update_value(&admin, &commitment_id, &new_value);

    // Verify update succeeded
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);
}

#[test]
fn test_add_remove_authorized_updater() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment
    let commitment_id = String::from_str(&e, "commit_6");
    let owner = Address::generate(&e);
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, 10000, 10000, 20);

    // Add authorized updater
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);

    // Verify updater can update
    let new_value = 9500;
    e.mock_all_auths();
    client.update_value(&updater, &commitment_id, &new_value);
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);

    // Remove authorized updater
    e.mock_all_auths();
    client.remove_authorized_updater(&admin, &updater);
}

#[test]
#[should_panic(expected = "Only admin can add authorized updaters")]
fn test_add_updater_non_admin() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Non-admin tries to add updater
    let non_admin = Address::generate(&e);
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&non_admin, &updater);
}

#[test]
#[should_panic(expected = "Cannot update value for non-active commitment")]
fn test_update_non_active_commitment() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment with violated status
    let commitment_id = String::from_str(&e, "commit_7");
    let owner = Address::generate(&e);
    let commitment = Commitment {
        commitment_id: commitment_id.clone(),
        owner,
        nft_token_id: 1,
        rules: CommitmentRules {
            duration_days: 365,
            max_loss_percent: 20,
            commitment_type: String::from_str(&e, "balanced"),
            early_exit_penalty: 10,
            min_fee_threshold: 1000,
        },
        amount: 10000,
        asset_address: Address::generate(&e),
        created_at: 1000,
        expires_at: 1000 + (365 * 24 * 60 * 60),
        current_value: 7500,
        status: String::from_str(&e, "violated"), // Already violated
    };

    let key = DataKey::Commitment(commitment_id.clone());
    e.as_contract(&contract_id, || {
        e.storage().persistent().set(&key, &commitment);
    });

    // Try to update violated commitment
    e.mock_all_auths();
    client.update_value(&admin, &commitment_id, &8000);
}

#[test]
fn test_edge_case_zero_initial_value() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment with zero initial value
    let commitment_id = String::from_str(&e, "commit_8");
    let owner = Address::generate(&e);
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, 0, 0, 20);

    // Update value - should not panic even with zero initial value
    let new_value = 1000;
    e.mock_all_auths();
    client.update_value(&admin, &commitment_id, &new_value);

    // Verify value was updated
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);
    // Should not be marked as violated (edge case handled)
    assert_eq!(commitment.status, String::from_str(&e, "active"));
}

#[test]
fn test_create_commitment() {
    // TODO: Test commitment creation
}

#[test]
fn test_edge_case_negative_drawdown() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment
    let commitment_id = String::from_str(&e, "commit_9");
    let owner = Address::generate(&e);
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, 10000, 10000, 20);

    // Update value to be higher than initial (gain, not loss)
    let new_value = 12000; // 20% gain
    e.mock_all_auths();
    client.update_value(&admin, &commitment_id, &new_value);

    // Verify value was updated and no violation (negative drawdown)
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);
    assert_eq!(commitment.status, String::from_str(&e, "active"));
}

#[test]
#[should_panic(expected = "Commitment not found")]
fn test_update_nonexistent_commitment() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Try to update non-existent commitment
    let commitment_id = String::from_str(&e, "nonexistent");
    e.mock_all_auths();
    client.update_value(&admin, &commitment_id, &9500);
}

#[test]
fn test_check_violations() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment with violation
    let commitment_id = String::from_str(&e, "commit_10");
    let owner = Address::generate(&e);
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, 10000, 7500, 20); // 25% loss

    // Check violations
    let has_violations = client.check_violations(&commitment_id);
    assert!(has_violations);
}

#[test]
fn test_settle() {
    // TODO: Test settlement
}

#[test]
fn test_check_violations_no_violation() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Initialize
    client.initialize(&admin, &nft_contract);

    // Create test commitment without violation
    let commitment_id = String::from_str(&e, "commit_11");
    let owner = Address::generate(&e);
    create_test_commitment(&e, &contract_id, commitment_id.clone(), owner, 10000, 9500, 20); // 5% loss

    // Check violations
    let has_violations = client.check_violations(&commitment_id);
    assert!(!has_violations);
}

