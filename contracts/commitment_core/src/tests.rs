#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, String};

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

// Helper to store a commitment for testing
fn store_commitment(e: &Env, contract_id: &Address, commitment: &Commitment) {
    e.as_contract(contract_id, || {
        set_commitment(e, commitment);
    });
}

#[test]
fn test_initialize() {
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
#[should_panic(expected = "Invalid max loss percent")]
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
        CommitmentCoreContract::get_admin(e.clone())
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
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_1";
    
    // Create a commitment with no violations
    // Initial: 1000, Current: 950 (5% loss), Max loss: 10%, Duration: 30 days
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
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
    
    let has_violations = e.as_contract(&contract_id, || {
        CommitmentCoreContract::check_violations(e.clone(), String::from_str(&e, commitment_id))
    });
    
    assert!(!has_violations, "Should not have violations");
}

#[test]
fn test_check_violations_loss_limit_exceeded() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_2";
    
    // Create a commitment with loss limit violation
    // Initial: 1000, Current: 850 (15% loss), Max loss: 10%
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
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
    
    let has_violations = e.as_contract(&contract_id, || {
        CommitmentCoreContract::check_violations(e.clone(), String::from_str(&e, commitment_id))
    });
    
    assert!(has_violations, "Should have loss limit violation");
}

#[test]
fn test_check_violations_duration_expired() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_3";
    
    // Create a commitment that has expired
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
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
    
    let has_violations = e.as_contract(&contract_id, || {
        CommitmentCoreContract::check_violations(e.clone(), String::from_str(&e, commitment_id))
    });
    
    assert!(has_violations, "Should have duration violation");
}

#[test]
fn test_check_violations_both_violations() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_4";
    
    // Create a commitment with both violations
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
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
    
    let has_violations = e.as_contract(&contract_id, || {
        CommitmentCoreContract::check_violations(e.clone(), String::from_str(&e, commitment_id))
    });
    
    assert!(has_violations, "Should have both violations");
}

#[test]
fn test_get_violation_details_no_violations() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_5";
    
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
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
    
    let (has_violations, loss_violated, duration_violated, loss_percent, time_remaining) = 
        e.as_contract(&contract_id, || {
            CommitmentCoreContract::get_violation_details(e.clone(), String::from_str(&e, commitment_id))
        });
    
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
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_6";
    
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
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
    
    let commitment_id_str = String::from_str(&e, commitment_id);
    let (has_violations, loss_violated, duration_violated, loss_percent, _time_remaining) = 
        e.as_contract(&contract_id, || {
            CommitmentCoreContract::get_violation_details(e.clone(), commitment_id_str.clone())
        });
    
    assert!(has_violations, "Should have violations");
    assert!(loss_violated, "Loss should be violated");
    assert!(!duration_violated, "Duration should not be violated");
    assert_eq!(loss_percent, 15, "Loss percent should be 15%");
}

#[test]
fn test_get_violation_details_duration_violation() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_7";
    
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
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
    
    let (has_violations, loss_violated, duration_violated, _loss_percent, time_remaining) = 
        e.as_contract(&contract_id, || {
            CommitmentCoreContract::get_violation_details(e.clone(), String::from_str(&e, commitment_id))
        });
    
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
    let commitment_id = "nonexistent";
    
    e.as_contract(&contract_id, || {
        CommitmentCoreContract::check_violations(e.clone(), String::from_str(&e, commitment_id))
    });
}

#[test]
fn test_check_violations_edge_case_exact_loss_limit() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_8";
    
    // Test exactly at the loss limit (should not violate)
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
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
    
    let has_violations = e.as_contract(&contract_id, || {
        CommitmentCoreContract::check_violations(e.clone(), String::from_str(&e, commitment_id))
    });
    
    // Exactly at limit should not violate (uses > not >=)
    assert!(!has_violations, "Exactly at limit should not violate");
}

#[test]
fn test_check_violations_edge_case_exact_expiry() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_9";
    
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
        &owner,
        1000,
        950,
        10,
        30,
        created_at,
    );
    
    store_commitment(&e, &contract_id, &commitment);
    
    // Set time to exactly expires_at
    e.ledger().with_mut(|l| {
        l.timestamp = commitment.expires_at;
    });
    
    let has_violations = e.as_contract(&contract_id, || {
        CommitmentCoreContract::check_violations(e.clone(), String::from_str(&e, commitment_id))
    });
    
    // At expiry time, should be violated (uses >=)
    assert!(has_violations, "At expiry time should violate");
}

#[test]
fn test_check_violations_zero_amount() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let owner = Address::generate(&e);
    let commitment_id = "test_commitment_10";
    
    // Edge case: zero amount (should not cause division by zero)
    let created_at = 1000u64;
    let commitment = create_test_commitment(
        &e,
        commitment_id,
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
    
    let has_violations = e.as_contract(&contract_id, || {
        CommitmentCoreContract::check_violations(e.clone(), String::from_str(&e, commitment_id))
    });
    
    // Should not panic and should only check duration
    assert!(!has_violations, "Zero amount should not cause issues");
}
