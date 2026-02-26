#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, Map, String};

#[test]
fn test_initialize_and_getters() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);

    let init = e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone())
    });
    assert_eq!(init, Ok(()));

    let stored_admin = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_admin(e.clone()).unwrap()
    });
    let stored_core = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_core_contract(e.clone()).unwrap()
    });

    assert_eq!(stored_admin, admin);
    assert_eq!(stored_core, core);
}

#[test]
fn test_initialize_twice_fails() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    let second = e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone())
    });
    assert_eq!(second, Err(AttestationError::AlreadyInitialized));
}


// ============================================
// verify_compliance Tests
// ============================================

/// Helper to create a mock commitment with specific status
fn create_mock_commitment_with_status(
    e: &Env,
    commitment_id: &str,
    status: &str,
    amount: i128,
    current_value: i128,
    max_loss_percent: u32,
) -> Commitment {
    let owner = Address::generate(e);
    let asset_address = Address::generate(e);
    
    Commitment {
        commitment_id: String::from_str(e, commitment_id),
        owner,
        nft_token_id: 1,
        rules: CommitmentRules {
            duration_days: 30,
            max_loss_percent,
            commitment_type: String::from_str(e, "safe"),
            early_exit_penalty: 5,
            min_fee_threshold: 100_0000000,
            grace_period_days: 0,
        },
        amount,
        asset_address,
        created_at: 1000,
        expires_at: 1000 + (30 * 86400),
        current_value,
        status: String::from_str(e, status),
    }
}

#[test]
fn test_verify_compliance_settled_commitment_returns_true() {
    let e = Env::default();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    
    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_settled");
    
    // Initialize attestation engine
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
    });
    
    // Create a settled commitment in core contract
    let commitment = create_mock_commitment_with_status(&e, "test_commitment_settled", "settled", 1000, 1050, 10);
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });
    
    // Verify compliance for settled commitment should return true
    let is_compliant = e.as_contract(&attestation_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    });
    
    assert!(is_compliant, "Settled commitment should be compliant");
}

#[test]
fn test_verify_compliance_violated_commitment_returns_false() {
    let e = Env::default();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    
    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_violated");
    
    // Initialize attestation engine
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
    });
    
    // Create a violated commitment in core contract
    let commitment = create_mock_commitment_with_status(&e, "test_commitment_violated", "violated", 1000, 850, 10);
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });
    
    // Verify compliance for violated commitment should return false
    let is_compliant = e.as_contract(&attestation_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    });
    
    assert!(!is_compliant, "Violated commitment should not be compliant");
}

#[test]
fn test_verify_compliance_early_exit_returns_false() {
    let e = Env::default();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    
    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_early_exit");
    
    // Initialize attestation engine
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
    });
    
    // Create an early_exit commitment in core contract
    let commitment = create_mock_commitment_with_status(&e, "test_commitment_early_exit", "early_exit", 1000, 980, 10);
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });
    
    // Verify compliance for early_exit commitment should return false
    let is_compliant = e.as_contract(&attestation_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    });
    
    assert!(!is_compliant, "Early exit commitment should not be compliant");
}

#[test]
fn test_verify_compliance_active_commitment_within_rules_returns_true() {
    let e = Env::default();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    
    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_active_compliant");
    
    // Initialize attestation engine
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
    });
    
    // Create an active commitment with 5% loss (within 10% limit)
    let commitment = create_mock_commitment_with_status(&e, "test_commitment_active_compliant", "active", 1000, 950, 10);
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });
    
    // Verify compliance for active commitment within rules
    let is_compliant = e.as_contract(&attestation_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    });
    
    assert!(is_compliant, "Active commitment within rules should be compliant");
}

#[test]
fn test_verify_compliance_active_commitment_exceeds_loss_returns_false() {
    let e = Env::default();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    
    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_active_noncompliant");
    
    // Initialize attestation engine
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
    });
    
    // Create an active commitment with 15% loss (exceeds 10% limit)
    let commitment = create_mock_commitment_with_status(&e, "test_commitment_active_noncompliant", "active", 1000, 850, 10);
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });
    
    // Verify compliance for active commitment exceeding loss limit
    let is_compliant = e.as_contract(&attestation_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    });
    
    assert!(!is_compliant, "Active commitment exceeding loss limit should not be compliant");
}

#[test]
fn test_verify_compliance_nonexistent_commitment_returns_false() {
    let e = Env::default();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    
    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "nonexistent_commitment");
    
    // Initialize attestation engine
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
    });
    
    // Don't create any commitment - test with nonexistent ID
    let is_compliant = e.as_contract(&attestation_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    });
    
    assert!(!is_compliant, "Nonexistent commitment should return false");
}

#[test]
fn test_verify_compliance_uninitialized_contract_returns_false() {
    let e = Env::default();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let commitment_id = String::from_str(&e, "test_commitment");
    
    // Don't initialize the contract - test uninitialized state
    let is_compliant = e.as_contract(&attestation_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    });
    
    assert!(!is_compliant, "Uninitialized contract should return false");
}
#[test]
fn test_attest_without_initialize_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);

    let caller = Address::generate(&e);
    let commitment_id = String::from_str(&e, "c_uninitialized");
    let attestation_type = String::from_str(&e, "health_check");
    let data = Map::<String, String>::new(&e);

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            caller.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
    });

    assert_eq!(result, Err(AttestationError::Unauthorized));
}

#[test]
fn test_get_admin_not_initialized_returns_error() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);

    let result = e.as_contract(&contract_id, || AttestationEngineContract::get_admin(e.clone()));

    assert_eq!(result, Err(AttestationError::NotInitialized));
}

#[test]
fn test_get_core_contract_not_initialized_returns_error() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);

    let result =
        e.as_contract(&contract_id, || AttestationEngineContract::get_core_contract(e.clone()));

    assert_eq!(result, Err(AttestationError::NotInitialized));
}

#[test]
fn test_get_attestations_not_initialized_returns_empty() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let commitment_id = String::from_str(&e, "uninitialized");

    let attestations = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });

    assert_eq!(attestations.len(), 0);
}

#[test]
fn test_get_attestation_count_not_initialized_returns_zero() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let commitment_id = String::from_str(&e, "uninitialized");

    let count = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestation_count(e.clone(), commitment_id.clone())
    });

    assert_eq!(count, 0);
}

#[test]
fn test_get_stored_health_metrics_not_initialized_returns_none() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let commitment_id = String::from_str(&e, "uninitialized");

    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_stored_health_metrics(e.clone(), commitment_id.clone())
    });

    assert!(metrics.is_none());
}

#[test]
fn test_fee_queries_not_initialized_return_defaults() {
    let e = Env::default();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let asset = Address::generate(&e);

    let (fee_amount, fee_asset) =
        e.as_contract(&contract_id, || AttestationEngineContract::get_attestation_fee(e.clone()));
    assert_eq!(fee_amount, 0);
    assert!(fee_asset.is_none());

    let fee_recipient =
        e.as_contract(&contract_id, || AttestationEngineContract::get_fee_recipient(e.clone()));
    assert!(fee_recipient.is_none());

    let collected_fees = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_collected_fees(e.clone(), asset.clone())
    });
    assert_eq!(collected_fees, 0);
}

