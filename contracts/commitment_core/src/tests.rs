#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger, testutils::Events, Address, Env, String, symbol_short, vec, IntoVal};

// Helper function to create a test commitment
fn create_test_commitment(
    e: &Env,
    commitment_id: &str,
    owner: &Address,
    amount: i128,
    current_value: i128,
    max_loss_percent: u32,
    duration_days: u32,
    created_at: u64,
) -> Commitment {
    let expires_at = created_at + (duration_days as u64 * 86400); // days to seconds
    
    Commitment {
        commitment_id: String::from_str(e, commitment_id),
        owner: owner.clone(),
        nft_token_id: 1,
        rules: CommitmentRules {
            duration_days,
            max_loss_percent,
            commitment_type: String::from_str(e, "balanced"),
            early_exit_penalty: 10,
            min_fee_threshold: 1000,
        },
        amount,
        asset_address: Address::generate(e),
        created_at,
        expires_at,
        current_value,
        status: String::from_str(e, "active"),
    }
}

// Helper to store a commitment for testing using DataKey
fn store_commitment(e: &Env, contract_id: &Address, commitment: &Commitment) {
    e.as_contract(contract_id, || {
        let key = DataKey::Commitment(commitment.commitment_id.clone());
        e.storage().persistent().set(&key, commitment);
        set_commitment(e, commitment);
    });
}

#[allow(dead_code)]
fn build_test_commitment(
    e: &Env,
    commitment_id: &str,
    owner: &Address,
    amount: i128,
    current_value: i128,
    max_loss_percent: u32,
    duration_days: u32,
    created_at: u64,
) -> Commitment {
    Commitment {
        commitment_id: String::from_str(e, commitment_id),
        owner: owner.clone(),
        nft_token_id: 1,
        rules: CommitmentRules {
            duration_days,
            max_loss_percent,
            commitment_type: String::from_str(e, "balanced"),
            early_exit_penalty: 10,
            min_fee_threshold: 1000,
        },
        amount,
        asset_address: Address::generate(e),
        created_at,
        expires_at: created_at + (duration_days as u64 * 86400),
        current_value,
        status: String::from_str(e, "active"),
    }
}

#[test]
fn test_initialize() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
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
    let commitment_id_str = "commit_1";
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 10000;
    let max_loss_percent = 20;
    let duration_days = 365;
    let created_at = 1000;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        initial_value,
        current_value,
        max_loss_percent,
        duration_days,
        created_at,
    );
    store_commitment(&e, &contract_id, &commitment);

    // Add authorized updater
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);

    // Update value
    let commitment_id = String::from_str(&e, commitment_id_str);
    let new_value = 9500; // 5% loss, within 20% limit
    e.mock_all_auths();
    client.update_value(&updater, &commitment_id, &new_value);

    // Verify value was updated
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);
    assert_eq!(commitment.status, String::from_str(&e, "active"));
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
    let commitment_id_str = "commit_2";
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 10000;
    let max_loss_percent = 20;
    let duration_days = 365;
    let created_at = 1000;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        initial_value,
        current_value,
        max_loss_percent,
        duration_days,
        created_at,
    );
    store_commitment(&e, &contract_id, &commitment);

    // Add authorized updater
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);

    // Update value to trigger violation (25% loss exceeds 20% limit)
    let commitment_id = String::from_str(&e, commitment_id_str);
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
    let commitment_id_str = "commit_3";
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 10000;
    let max_loss_percent = 20;
    let duration_days = 365;
    let created_at = 1000;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        initial_value,
        current_value,
        max_loss_percent,
        duration_days,
        created_at,
    );
    store_commitment(&e, &contract_id, &commitment);

    // Add authorized updater
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);

    // Update value to exactly 20% loss (should NOT violate)
    let commitment_id = String::from_str(&e, commitment_id_str);
    let new_value = 8000; // Exactly 20% loss
    e.mock_all_auths();
    client.update_value(&updater, &commitment_id, &new_value);

    // Verify no violation
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);
    assert_eq!(commitment.status, String::from_str(&e, "active"));
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
    let commitment_id_str = "commit_4";
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 10000;
    let max_loss_percent = 20;
    let duration_days = 365;
    let created_at = 1000;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        initial_value,
        current_value,
        max_loss_percent,
        duration_days,
        created_at,
    );
    store_commitment(&e, &contract_id, &commitment);

    // Try to update with unauthorized caller
    let commitment_id = String::from_str(&e, commitment_id_str);
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
    let commitment_id_str = "commit_5";
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 10000;
    let max_loss_percent = 20;
    let duration_days = 365;
    let created_at = 1000;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        initial_value,
        current_value,
        max_loss_percent,
        duration_days,
        created_at,
    );
    store_commitment(&e, &contract_id, &commitment);

    // Admin should be able to update without being added to whitelist
    let commitment_id = String::from_str(&e, commitment_id_str);
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
    let commitment_id_str = "commit_6";
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 10000;
    let max_loss_percent = 20;
    let duration_days = 365;
    let created_at = 1000;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        initial_value,
        current_value,
        max_loss_percent,
        duration_days,
        created_at,
    );
    store_commitment(&e, &contract_id, &commitment);

    // Add authorized updater
    let updater = Address::generate(&e);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);

    // Verify updater can update
    let commitment_id = String::from_str(&e, commitment_id_str);
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
    let commitment_id_str = "commit_7";
    let owner = Address::generate(&e);
    let commitment = Commitment {
        commitment_id: String::from_str(&e, commitment_id_str),
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
        expires_at: 1000 + (365 * 86400),
        current_value: 7500,
        status: String::from_str(&e, "violated"),
    };

    store_commitment(&e, &contract_id, &commitment);

    // Try to update violated commitment
    let commitment_id = String::from_str(&e, commitment_id_str);
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
    let commitment_id_str = "commit_8";
    let owner = Address::generate(&e);
    let initial_value = 0;
    let current_value = 0;
    let max_loss_percent = 20;
    let duration_days = 365;
    let created_at = 1000;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        initial_value,
        current_value,
        max_loss_percent,
        duration_days,
        created_at,
    );
    store_commitment(&e, &contract_id, &commitment);

    // Update value - should not panic even with zero initial value
    let commitment_id = String::from_str(&e, commitment_id_str);
    let new_value = 1000;
    e.mock_all_auths();
    client.update_value(&admin, &commitment_id, &new_value);

    // Verify value was updated
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, new_value);
    assert_eq!(commitment.status, String::from_str(&e, "active"));
}

#[test]
fn test_create_commitment() {
    // TODO: Test commitment creation
}

#[test]
fn test_edge_case_negative_drawdown() {

    // Test successful initialization
    // e.as_contract(&contract_id, || {
    //     CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    // });
}

#[test]
fn test_create_commitment_valid() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let _owner = Address::generate(&e);
    let _asset_address = Address::generate(&e);

    // Initialize the contract
    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });

    // Create valid commitment rules
    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&e, "safe"),
        early_exit_penalty: 5,
        min_fee_threshold: 100,
    };

    let _amount = 1000i128;

    // Test commitment creation (this will panic if NFT contract is not properly set up)
    // For now, we'll test that the validation works by testing individual validation functions
    e.as_contract(&contract_id, || {
        CommitmentCoreContract::validate_rules(&e, &rules); // Should not panic
    });
}

#[test]
#[should_panic(expected = "Invalid duration")]
fn test_validate_rules_invalid_duration() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    let rules = CommitmentRules {
        duration_days: 0, // Invalid duration
        max_loss_percent: 10,
        commitment_type: String::from_str(&e, "safe"),
        early_exit_penalty: 5,
        min_fee_threshold: 100,
    };

    // Test invalid duration - should panic
    e.as_contract(&contract_id, || {
        CommitmentCoreContract::validate_rules(&e, &rules);
    });
}

#[test]
#[should_panic(expected = "Invalid percent")]
fn test_validate_rules_invalid_max_loss() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 150, // Invalid max loss (> 100)
        commitment_type: String::from_str(&e, "safe"),
        early_exit_penalty: 5,
        min_fee_threshold: 100,
    };

    // Test invalid max loss percent - should panic
    e.as_contract(&contract_id, || {
        CommitmentCoreContract::validate_rules(&e, &rules);
    });
}

#[test]
#[should_panic(expected = "Invalid commitment type")]
fn test_validate_rules_invalid_type() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&e, "invalid_type"), // Invalid type
        early_exit_penalty: 5,
        min_fee_threshold: 100,
    };

    // Test invalid commitment type - should panic
    e.as_contract(&contract_id, || {
        CommitmentCoreContract::validate_rules(&e, &rules);
    });
}

#[test]
fn test_get_owner_commitments() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let owner = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });

    // Initially empty
    let commitments = e.as_contract(&contract_id, || {
        CommitmentCoreContract::get_owner_commitments(e.clone(), owner.clone())
    });
    assert_eq!(commitments.len(), 0);
}

#[test]
fn test_get_total_commitments() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);

    // Test successful initialization
    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });
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
    let commitment_id_str = "commit_10";
    let owner = Address::generate(&e);
    let initial_value = 10000;
    let current_value = 7500;
    let max_loss_percent = 20;
    let duration_days = 365;
    let created_at = 1000;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        initial_value,
        current_value,
        max_loss_percent,
        duration_days,
        created_at,
    );
    store_commitment(&e, &contract_id, &commitment);

    // Check violations
    let commitment_id = String::from_str(&e, commitment_id_str);
    let has_violations = client.check_violations(&commitment_id);
    assert!(has_violations);
}

#[test]
fn test_settle() {
    // TODO: Test settlement
    let e = Env::default();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });

    // Initially zero
    let total = e.as_contract(&contract_id, || {
        CommitmentCoreContract::get_total_commitments(e.clone())
    });
    assert_eq!(total, 0);
}

#[test]
fn test_get_admin() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });

    let retrieved_admin = e.as_contract(&contract_id, || {
        CommitmentCoreContract::get_admin(&e)
    });
    assert_eq!(retrieved_admin, admin);
}

#[test]
fn test_get_nft_contract() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });

    let retrieved_nft_contract = e.as_contract(&contract_id, || {
        CommitmentCoreContract::get_nft_contract(e.clone())
    });
    assert_eq!(retrieved_nft_contract, nft_contract);
}

#[test]
fn test_check_violations_no_violations() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_1";
    
    // Create a commitment with no violations
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        1000,
        950, // 5% loss
        10,  // max 10% loss allowed
        30,  // 30 days duration
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    // Set ledger time to 15 days later (halfway through)
    e.ledger().with_mut(|l| {
        l.timestamp = created_at + (15 * 86400);
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let has_violations = client.check_violations(&commitment_id);
    
    assert!(!has_violations, "Should not have violations");
}

#[test]
fn test_check_violations_loss_limit_exceeded() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_2";
    
    // Create a commitment with loss limit violation
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        1000,
        850, // 15% loss - exceeds 10% limit
        10,  // max 10% loss allowed
        30,
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    // Set ledger time to 5 days later (still within duration)
    e.ledger().with_mut(|l| {
        l.timestamp = created_at + (5 * 86400);
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let has_violations = client.check_violations(&commitment_id);
    
    assert!(has_violations, "Should have loss limit violation");
}

#[test]
fn test_check_violations_duration_expired() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_3";
    
    // Create a commitment that has expired
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        1000,
        980, // 2% loss - within limit
        10,  // max 10% loss allowed
        30,  // 30 days duration
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    // Set ledger time to 31 days later (expired)
    e.ledger().with_mut(|l| {
        l.timestamp = created_at + (31 * 86400);
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let has_violations = client.check_violations(&commitment_id);
    
    assert!(has_violations, "Should have duration violation");
}

#[test]
fn test_check_violations_both_violations() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_4";
    
    // Create a commitment with both violations
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        1000,
        800, // 20% loss - exceeds limit
        10,  // max 10% loss allowed
        30,
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    // Set ledger time to 31 days later (expired)
    e.ledger().with_mut(|l| {
        l.timestamp = created_at + (31 * 86400);
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let has_violations = client.check_violations(&commitment_id);
    
    assert!(has_violations, "Should have both violations");
}

#[test]
fn test_get_violation_details_no_violations() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_5";
    
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        1000,
        950, // 5% loss
        10,  // max 10% loss
        30,
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    // Set ledger time to 15 days later
    e.ledger().with_mut(|l| {
        l.timestamp = created_at + (15 * 86400);
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let (has_violations, loss_violated, duration_violated, loss_percent, time_remaining) = 
        client.get_violation_details(&commitment_id);
    
    assert!(!has_violations, "Should not have violations");
    assert!(!loss_violated, "Loss should not be violated");
    assert!(!duration_violated, "Duration should not be violated");
    assert_eq!(loss_percent, 5, "Loss percent should be 5%");
    assert!(time_remaining > 0, "Time should remain");
}

#[test]
fn test_get_violation_details_loss_violation() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_6";
    
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        1000,
        850, // 15% loss - exceeds 10%
        10,
        30,
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    e.ledger().with_mut(|l| {
        l.timestamp = created_at + (10 * 86400);
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let (has_violations, loss_violated, duration_violated, loss_percent, _time_remaining) = 
        client.get_violation_details(&commitment_id);
    
    assert!(has_violations, "Should have violations");
    assert!(loss_violated, "Loss should be violated");
    assert!(!duration_violated, "Duration should not be violated");
    assert_eq!(loss_percent, 15, "Loss percent should be 15%");
}

#[test]
fn test_get_violation_details_duration_violation() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_7";
    
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        1000,
        980, // 2% loss - within limit
        10,
        30,
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    // Set time to 31 days later (expired)
    e.ledger().with_mut(|l| {
        l.timestamp = created_at + (31 * 86400);
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let (has_violations, loss_violated, duration_violated, _loss_percent, time_remaining) = 
        client.get_violation_details(&commitment_id);
    
    assert!(has_violations, "Should have violations");
    assert!(!loss_violated, "Loss should not be violated");
    assert!(duration_violated, "Duration should be violated");
    assert_eq!(time_remaining, 0, "Time remaining should be 0");
}

#[test]
#[should_panic(expected = "Commitment not found")]
fn test_check_violations_not_found() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let commitment_id = String::from_str(&e, "nonexistent");
    client.check_violations(&commitment_id);
}

#[test]
fn test_check_violations_edge_case_exact_loss_limit() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_8";
    
    // Test exactly at the loss limit (should not violate)
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        1000,
        900, // Exactly 10% loss
        10,  // max 10% loss
        30,
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    e.ledger().with_mut(|l| {
        l.timestamp = created_at + (15 * 86400);
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let has_violations = client.check_violations(&commitment_id);
    
    // Exactly at limit should not violate (uses > not >=)
    assert!(!has_violations, "Exactly at limit should not violate");
}

#[test]
fn test_check_violations_edge_case_exact_expiry() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_9";
    
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        1000,
        950,
        10,
        30,
        created_at,
    );
    
    let expires_at = commitment.expires_at;
    store_commitment(&e, &contract_id, &commitment);
    
    // Set time to exactly expires_at
    e.ledger().with_mut(|l| {
        l.timestamp = expires_at;
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let has_violations = client.check_violations(&commitment_id);
    
    // At expiry time, should be violated (uses >=)
    assert!(has_violations, "At expiry time should violate");
}

#[test]
fn test_check_violations_zero_amount() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    
    client.initialize(&admin, &nft_contract);
    
    let owner = Address::generate(&e);
    let commitment_id_str = "test_commitment_10";
    
    // Edge case: zero amount (should not cause division by zero)
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id_str,
        &owner,
        0,   // zero amount
        0,   // zero value
        10,
        30,
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    e.ledger().with_mut(|l| {
        l.timestamp = created_at + (15 * 86400);
    });
    
    let commitment_id = String::from_str(&e, commitment_id_str);
    let has_violations = client.check_violations(&commitment_id);
    
        // Should not panic and should only check duration
    assert!(!has_violations, "Zero amount should not cause issues");
}

// Event Tests

#[test]
fn test_create_commitment_event() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);

    let commitment_id_str = "test_commitment_10";
    let commitment_id = String::from_str(&e, commitment_id_str);
    
    client.initialize(&admin, &nft_contract);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&e, "safe"),
        early_exit_penalty: 5,
        min_fee_threshold: 100,
    };

    // Confirm new contract's total commitments is zero
    let contract_id2 = e.register_contract(None, CommitmentCoreContract);
    let admin2 = Address::generate(&e);
    let nft_contract2 = Address::generate(&e);
    e.as_contract(&contract_id2, || {
        CommitmentCoreContract::initialize(e.clone(), admin2.clone(), nft_contract2.clone());
    });

    // Initially zero
    let total = e.as_contract(&contract_id2, || {
        CommitmentCoreContract::get_total_commitments(e.clone())
    });
    assert_eq!(total, 0);
}

#[test]
fn test_update_value_event() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // We need a commitment first so update_value works
    let commitment_id = String::from_str(&e, "test_id");
    let commitment_id_str = "test_id";
    let owner = Address::generate(&e);
    let updater = Address::generate(&e);
    let created_at = 1000u64;
    let commitment =  create_test_commitment(&e, commitment_id_str, &owner, 1000, 1000, 10, 30, created_at);

    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    client.initialize(&admin, &nft_contract);

    store_commitment(&e, &contract_id, &commitment);
    e.mock_all_auths();
    client.add_authorized_updater(&admin, &updater);
    client.update_value(&updater, &commitment_id, &1100);

    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, contract_id);
    assert!(!events.is_empty(), "Event should be emitted");
}

#[test]
#[should_panic(expected = "Commitment not found")]
fn test_settle_event() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    let commitment_id = String::from_str(&e, "test_id");
    // This will panic because commitment doesn't exist
    // The test verifies that the function properly validates preconditions
    client.settle(&commitment_id);
}

#[test]
#[should_panic(expected = "Commitment not found")]
fn test_early_exit_event() {
    let e = Env::default();
    let caller = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    let commitment_id = String::from_str(&e, "test_id");
    // This will panic because commitment doesn't exist
    // The test verifies that the function properly validates preconditions
    client.early_exit(&commitment_id, &caller);
}

#[test]
#[should_panic(expected = "Commitment not found")]
fn test_allocate_event() {
    let e = Env::default();
    let target_pool = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    let commitment_id = String::from_str(&e, "test_id");
    // This will panic because commitment doesn't exist
    // The test verifies that the function properly validates preconditions
    client.allocate(&commitment_id, &target_pool, &500);
}
