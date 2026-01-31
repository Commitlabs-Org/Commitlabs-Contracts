#![cfg(test)]

use super::*;
use commitment_core::{
    Commitment as CoreCommitment, CommitmentCoreContract, CommitmentRules as CoreCommitmentRules,
    DataKey,
};
use soroban_sdk::{
    symbol_short, testutils::Address as _, testutils::Events, testutils::Ledger as _, vec, Address,
    Env, IntoVal, Map, String,
};

fn store_core_commitment(
    e: &Env,
    commitment_core_id: &Address,
    commitment_id: &str,
    owner: &Address,
    amount: i128,
    current_value: i128,
    max_loss_percent: u32,
    duration_days: u32,
    created_at: u64,
) {
    let expires_at = created_at + (duration_days as u64 * 86400);
    let commitment = CoreCommitment {
        commitment_id: String::from_str(e, commitment_id),
        owner: owner.clone(),
        nft_token_id: 1,
        rules: CoreCommitmentRules {
            duration_days,
            max_loss_percent,
            commitment_type: String::from_str(e, "balanced"),
            early_exit_penalty: 10,
            min_fee_threshold: 1000,
            grace_period_days: 3,
        },
        amount,
        asset_address: Address::generate(e),
        created_at,
        expires_at,
        current_value,
        status: String::from_str(e, "active"),
    };

    e.as_contract(commitment_core_id, || {
        e.storage().instance().set(
            &DataKey::Commitment(commitment.commitment_id.clone()),
            &commitment,
        );
    });
}

// Helper function to set up test environment with registered commitment_core contract
fn setup_test_env() -> (Env, Address, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let commitment_core_id = e.register_contract(None, MockCoreContract);
    let _contract_id = e.register_contract(None, AttestationEngineContract);

    e.as_contract(&_contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin, commitment_core_id);
    });
}

#[test]
fn test_attest() {
    let e = Env::default();
    let verified_by = Address::generate(&e);
    let core_id = e.register_contract(None, MockCoreContract);
    let _contract_id = e.register_contract(None, AttestationEngineContract);

    e.as_contract(&_contract_id, || {
        AttestationEngineContract::initialize(e.clone(), Address::generate(&e), core_id.clone());
    });

    let commitment_id = String::from_str(&e, "c1");
    let owner = Address::generate(&e);

    let rules = CommitmentRules {
        duration_days: 10,
        max_loss_percent: 20,
        commitment_type: String::from_str(&e, "safe"),
        early_exit_penalty: 0,
        min_fee_threshold: 0,
    };
    let commitment = Commitment {
        commitment_id: commitment_id.clone(),
        owner,
        nft_token_id: 1,
        rules,
        amount: 1_000,
        asset_address: Address::generate(&e),
        created_at: 0,
        expires_at: 100,
        current_value: 1_000,
        status: String::from_str(&e, "active"),
    };

    e.as_contract(&core_id, || {
        MockCoreContract::set_commitment(e.clone(), commitment_id.clone(), commitment);
        MockCoreContract::set_violations(e.clone(), commitment_id.clone(), false);
    });

    let data = Map::<String, String>::new(&e);
    e.as_contract(&_contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            commitment_id.clone(),
            String::from_str(&e, "health_check"),
            data,
            verified_by,
        );
    });

    let atts = e.as_contract(&_contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id)
    });
    assert!(atts.len() == 1);
}

#[test]
fn test_verify_compliance() {
    let e = Env::default();
    // Set a deterministic ledger timestamp for duration checks.
    e.ledger().with_mut(|li| {
        li.timestamp = 50;
    });

    let core_id = e.register_contract(None, MockCoreContract);
    let _contract_id = e.register_contract(None, AttestationEngineContract);
    e.as_contract(&_contract_id, || {
        AttestationEngineContract::initialize(e.clone(), Address::generate(&e), core_id.clone());
    });

    let commitment_id = String::from_str(&e, "c1");
    let owner = Address::generate(&e);

    let base_rules = CommitmentRules {
        duration_days: 10,
        max_loss_percent: 20,
        commitment_type: String::from_str(&e, "safe"),
        early_exit_penalty: 0,
        min_fee_threshold: 100,
    };

    // Happy path: in-range drawdown, not expired, fees meet threshold, no violations.
    let mut commitment = Commitment {
        commitment_id: commitment_id.clone(),
        owner: owner.clone(),
        nft_token_id: 1,
        rules: base_rules.clone(),
        amount: 1_000,
        asset_address: Address::generate(&e),
        created_at: 0,
        expires_at: 100,
        current_value: 900, // 10% drawdown
        status: String::from_str(&e, "active"),
    };
    e.as_contract(&core_id, || {
        MockCoreContract::set_commitment(e.clone(), commitment_id.clone(), commitment.clone());
        MockCoreContract::set_violations(e.clone(), commitment_id.clone(), false);
    });
    e.as_contract(&_contract_id, || {
        AttestationEngineContract::record_fees(e.clone(), commitment_id.clone(), 100);
    });

    assert!(e.as_contract(&_contract_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    }));

    // Loss limit exceeded
    commitment.current_value = 700; // 30% drawdown
    e.as_contract(&core_id, || {
        MockCoreContract::set_commitment(e.clone(), commitment_id.clone(), commitment.clone());
    });
    assert!(!e.as_contract(&_contract_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    }));

    // Duration expired
    commitment.current_value = 900;
    commitment.expires_at = 40;
    e.as_contract(&core_id, || {
        MockCoreContract::set_commitment(e.clone(), commitment_id.clone(), commitment.clone());
    });
    assert!(!e.as_contract(&_contract_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id.clone())
    }));

    // Fee threshold not met
    commitment.expires_at = 100;
    e.as_contract(&core_id, || {
        MockCoreContract::set_commitment(e.clone(), commitment_id.clone(), commitment.clone());
    });
    // Reset engine fees by using a new commitment id
    let commitment_id2 = String::from_str(&e, "c2");
    commitment.commitment_id = commitment_id2.clone();
    e.as_contract(&core_id, || {
        MockCoreContract::set_commitment(e.clone(), commitment_id2.clone(), commitment.clone());
        MockCoreContract::set_violations(e.clone(), commitment_id2.clone(), false);
    });
    assert!(!e.as_contract(&_contract_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id2.clone())
    }));

    // Active violations
    e.as_contract(&core_id, || {
        MockCoreContract::set_violations(e.clone(), commitment_id2.clone(), true);
    });
    assert!(!e.as_contract(&_contract_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id2)
    }));

    // Edge: duration_days == 0 bypasses duration check
    let commitment_id3 = String::from_str(&e, "c3");
    let rules_no_duration = CommitmentRules {
        duration_days: 0,
        ..base_rules
    };
    let commitment3 = Commitment {
        commitment_id: commitment_id3.clone(),
        owner,
        nft_token_id: 3,
        rules: rules_no_duration,
        amount: 0, // edge: amount==0 -> drawdown=0
        asset_address: Address::generate(&e),
        created_at: 0,
        expires_at: 0,
        current_value: 0,
        status: String::from_str(&e, "active"),
    };
    e.as_contract(&core_id, || {
        MockCoreContract::set_commitment(e.clone(), commitment_id3.clone(), commitment3);
        MockCoreContract::set_violations(e.clone(), commitment_id3.clone(), false);
    });
    // fees not met but threshold is 100 -> still should fail; make threshold 0
    let mut commitment3b = e.as_contract(&core_id, || {
        MockCoreContract::get_commitment(e.clone(), commitment_id3.clone())
    });
    commitment3b.rules.min_fee_threshold = 0;
    e.as_contract(&core_id, || {
        MockCoreContract::set_commitment(e.clone(), commitment_id3.clone(), commitment3b);
    });
    assert!(e.as_contract(&_contract_id, || {
        AttestationEngineContract::verify_compliance(e.clone(), commitment_id3)
    }));

    // Register and initialize commitment_core contract
    let commitment_core_id = e.register_contract(None, CommitmentCoreContract);
    let nft_contract = Address::generate(&e);

    // Initialize commitment_core contract
    e.as_contract(&commitment_core_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });

    // Register attestation_engine contract
    let contract_id = e.register_contract(None, AttestationEngineContract);

    // Initialize attestation_engine contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), commitment_core_id.clone())
            .unwrap();
    });

    (e, admin, commitment_core_id, contract_id)
}

#[test]
fn test_initialize() {
    let (e, admin, commitment_core, contract_id) = setup_test_env();

    // Verify initialization by checking that we can call other functions
    // (indirect verification through storage access)
    let commitment_id = String::from_str(&e, "test");
    let _attestations = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id)
    });
}

#[test]
fn test_get_attestations_empty() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment_1");

    // Get attestations
    let attestations = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id)
    });

    assert_eq!(attestations.len(), 0);
}

#[test]
fn test_get_health_metrics_basic() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment_1");

    // Seed a commitment in the core contract so get_commitment succeeds
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment_1",
        &owner,
        1000,
        950,
        10,
        30,
        1000,
    );

    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id.clone())
    });

    assert_eq!(metrics.commitment_id, commitment_id);
    // Verify all fields are present
    assert!(metrics.compliance_score <= 100);
}

#[test]
fn test_get_health_metrics_drawdown_calculation() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment_1");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment_1",
        &owner,
        1000,
        900,
        10,
        30,
        1000,
    );
    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id)
    });

    // Verify drawdown calculation handles edge cases
    // initial=1000, current=900 => 10% drawdown
    assert_eq!(metrics.drawdown_percent, 10);
}

#[test]
fn test_get_health_metrics_zero_initial_value() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment_1");
    let owner = Address::generate(&e);
    // Explicitly store a zero-amount commitment to exercise the division-by-zero path
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment_1",
        &owner,
        0,
        0,
        10,
        30,
        1000,
    );
    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id)
    });

    // Should handle zero initial value gracefully (drawdown = 0)
    // This tests edge case handling
    assert!(metrics.drawdown_percent >= 0);
    assert_eq!(metrics.initial_value, 0);
}

#[test]
fn test_calculate_compliance_score_base() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment_1");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment_1",
        &owner,
        1000,
        950,
        10,
        30,
        1000,
    );
    let score = e.as_contract(&contract_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id)
    });

    // Score should be clamped between 0 and 100
    assert!(score <= 100);
}

#[test]
fn test_calculate_compliance_score_clamping() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment_1");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment_1",
        &owner,
        1000,
        950,
        10,
        30,
        1000,
    );
    let score = e.as_contract(&contract_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id)
    });

    // Verify score is clamped between 0 and 100
    assert!(score <= 100);
}

#[test]
fn test_get_health_metrics_includes_compliance_score() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment_1");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment_1",
        &owner,
        1000,
        950,
        10,
        30,
        1000,
    );
    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id)
    });

    // Verify compliance_score is included and valid
    assert!(metrics.compliance_score <= 100);
}

#[test]
fn test_get_health_metrics_last_attestation() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment_1");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment_1",
        &owner,
        1000,
        950,
        10,
        30,
        1000,
    );
    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id)
    });

    // With no attestations, last_attestation should be 0
    assert_eq!(metrics.last_attestation, 0);
}

#[test]
fn test_all_three_functions_work_together() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment_1");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment_1",
        &owner,
        1000,
        950,
        10,
        30,
        1000,
    );

    // Test all three functions work
    let attestations = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });
    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id.clone())
    });
    let score = e.as_contract(&contract_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id.clone())
    });

    // Verify they all return valid data
    assert_eq!(attestations.len(), 0); // No attestations stored yet
    assert_eq!(metrics.commitment_id, commitment_id);
    assert!(score <= 100);
    assert_eq!(metrics.compliance_score, score); // Should match
}

#[test]
fn test_get_attestations_returns_empty_vec_when_none_exist() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    // Test with different commitment IDs
    let commitment_id1 = String::from_str(&e, "commitment_1");
    let commitment_id2 = String::from_str(&e, "commitment_2");

    let attestations1 = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id1)
    });
    let attestations2 = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id2)
    });

    assert_eq!(attestations1.len(), 0);
    assert_eq!(attestations2.len(), 0);
}

#[test]
fn test_health_metrics_structure() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );
    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id.clone())
    });

    // Verify all required fields are present
    assert_eq!(metrics.commitment_id, commitment_id);
    assert_eq!(metrics.current_value, 1000);
    assert_eq!(metrics.initial_value, 1000);
    assert_eq!(metrics.drawdown_percent, 0);
    assert_eq!(metrics.fees_generated, 0);
    assert_eq!(metrics.volatility_exposure, 0);
    assert_eq!(metrics.last_attestation, 0);
    assert!(metrics.compliance_score <= 100);
}

#[test]
fn test_attest_and_get_metrics() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    // Set ledger timestamp to non-zero
    e.ledger().with_mut(|li| li.timestamp = 12345);

    let commitment_id = String::from_str(&e, "test_commitment_wf");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment_wf",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );
    let attestation_type = String::from_str(&e, "general");
    let mut data = Map::new(&e);
    data.set(
        String::from_str(&e, "note"),
        String::from_str(&e, "test attestation"),
    );

    // Record an attestation
    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
        .unwrap();
    });

    // Get attestations and verify
    let attestations = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });

    assert_eq!(attestations.len(), 1);
    assert_eq!(
        attestations.get(0).unwrap().attestation_type,
        attestation_type
    );

    // Get health metrics and verify last_attestation is updated
    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id.clone())
    });

    assert!(metrics.last_attestation > 0);
}

#[test]
#[should_panic(expected = "Reentrancy detected")]
fn test_attest_reentrancy_protection() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    let attestation_type = String::from_str(&e, "health_check");
    let data = Map::new(&e);

    // Manually set reentrancy guard to simulate reentrancy
    e.as_contract(&contract_id, || {
        e.storage()
            .instance()
            .set(&super::DataKey::ReentrancyGuard, &true);
    });

    // Try to attest, should panic
    e.as_contract(&contract_id, || {
        let _ = AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        );
    });
}

// ============================================================================
// Access Control Tests
// ============================================================================

#[test]
fn test_add_verifier_success() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    let verifier = Address::generate(&e);

    // Add verifier as admin
    e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone())
            .unwrap();
    });

    // Verify the verifier is now authorized
    let is_verifier = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(is_verifier);
}

#[test]
fn test_add_verifier_unauthorized() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let non_admin = Address::generate(&e);
    let verifier = Address::generate(&e);

    // Try to add verifier as non-admin
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), non_admin.clone(), verifier.clone())
    });

    assert_eq!(result, Err(AttestationError::Unauthorized));
}

#[test]
fn test_remove_verifier_success() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    let verifier = Address::generate(&e);

    // Add verifier
    e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone())
            .unwrap();
    });

    // Remove verifier
    e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier.clone())
            .unwrap();
    });

    // Verify verifier is no longer authorized
    let is_verifier = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(!is_verifier);
}

#[test]
fn test_attest_unauthorized_caller() {
    let (e, _admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment");
    let non_verifier = Address::generate(&e);
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    let attestation_type = String::from_str(&e, "health_check");
    let data = Map::new(&e);

    // Try to attest as non-verifier
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            non_verifier.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
    });

    assert_eq!(result, Err(AttestationError::Unauthorized));
}

#[test]
fn test_attest_authorized_verifier() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // record_fees requires caller (admin)
    client.record_fees(&admin, &commitment_id, &100);

    // Add verifier
    e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone())
            .unwrap();
    });

    assert_eq!(last_event.0, contract_id);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("FeeRec").into_val(&e),
            commitment_id.into_val(&e)
        ]
    );

    // Use invalid attestation type
    let attestation_type = String::from_str(&e, "invalid_type");
    let data = Map::new(&e);

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
    });

    assert_eq!(result, Err(AttestationError::InvalidAttestationType));
}

#[test]
fn test_attest_invalid_data_violation() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // violation type requires "violation_type" and "severity" fields
    let attestation_type = String::from_str(&e, "violation");
    let data = Map::new(&e); // Missing required fields

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            false,
        )
    });

    assert_eq!(last_event.0, contract_id);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("Drawdown").into_val(&e),
            commitment_id.into_val(&e)
        ]
    );

    // fee_generation requires "fee_amount" field
    let attestation_type = String::from_str(&e, "fee_generation");
    let data = Map::new(&e); // Missing required field

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
    });

    assert_eq!(result, Err(AttestationError::InvalidAttestationData));
}

#[test]
fn test_attest_invalid_data_drawdown() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // drawdown requires "drawdown_percent" field
    let attestation_type = String::from_str(&e, "drawdown");
    let data = Map::new(&e); // Missing required field

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
    });

    assert_eq!(result, Err(AttestationError::InvalidAttestationData));
}

// ============================================================================
// Attestation Recording Tests
// ============================================================================

#[test]
fn test_attest_health_check_success() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    e.ledger().with_mut(|li| li.timestamp = 10000);

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    let attestation_type = String::from_str(&e, "health_check");
    let data = Map::new(&e);

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
    });

    assert!(result.is_ok());

    let attestations = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });

    assert_eq!(attestations.len(), 1);
    assert!(attestations.get(0).unwrap().is_compliant);
}

#[test]
fn test_attest_violation_success() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    e.ledger().with_mut(|li| li.timestamp = 10000);

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    let attestation_type = String::from_str(&e, "violation");
    let mut data = Map::new(&e);
    data.set(
        String::from_str(&e, "violation_type"),
        String::from_str(&e, "excessive_drawdown"),
    );
    data.set(
        String::from_str(&e, "severity"),
        String::from_str(&e, "high"),
    );

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            false,
        )
    });

    assert!(result.is_ok());

    let attestations = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });

    assert_eq!(attestations.len(), 1);
    assert!(!attestations.get(0).unwrap().is_compliant);
}

#[test]
fn test_attest_fee_generation_success() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    e.ledger().with_mut(|li| li.timestamp = 10000);

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    let attestation_type = String::from_str(&e, "fee_generation");
    let mut data = Map::new(&e);
    data.set(
        String::from_str(&e, "fee_amount"),
        String::from_str(&e, "100"),
    );

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
    });

    assert!(result.is_ok());
}

#[test]
fn test_attest_drawdown_success() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    e.ledger().with_mut(|li| li.timestamp = 10000);

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    let attestation_type = String::from_str(&e, "drawdown");
    let mut data = Map::new(&e);
    data.set(
        String::from_str(&e, "drawdown_percent"),
        String::from_str(&e, "5"),
    );

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
    });

    assert!(result.is_ok());
}

#[test]
fn test_multiple_attestations() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    e.ledger().with_mut(|li| li.timestamp = 10000);

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // Record multiple attestations
    for i in 0..3 {
        e.ledger()
            .with_mut(|li| li.timestamp = 10000 + (i as u64 * 100));
        let data = Map::new(&e);
        e.as_contract(&contract_id, || {
            AttestationEngineContract::attest(
                e.clone(),
                admin.clone(),
                commitment_id.clone(),
                String::from_str(&e, "health_check"),
                data,
                true,
            )
            .unwrap();
        });
    }

    let attestations = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });

    assert_eq!(attestations.len(), 3);

    // Verify counter
    let count = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestation_count(e.clone(), commitment_id.clone())
    });
    assert_eq!(count, 3);
}

// ============================================================================
// Health Metrics Update Tests
// ============================================================================

#[test]
fn test_health_metrics_updated_after_attestation() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    e.ledger().with_mut(|li| li.timestamp = 10000);

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    let data = Map::new(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            String::from_str(&e, "health_check"),
            data,
            true,
        )
        .unwrap();
    });

    // Check stored health metrics
    let stored_metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_stored_health_metrics(e.clone(), commitment_id.clone())
    });

    assert!(stored_metrics.is_some());
    let metrics = stored_metrics.unwrap();
    assert_eq!(metrics.last_attestation, 10000);
}

#[test]
fn test_compliance_score_decreases_on_violation() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    e.ledger().with_mut(|li| li.timestamp = 10000);

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // Record a high severity violation
    let mut data = Map::new(&e);
    data.set(
        String::from_str(&e, "violation_type"),
        String::from_str(&e, "excessive_drawdown"),
    );
    data.set(
        String::from_str(&e, "severity"),
        String::from_str(&e, "high"),
    );

    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            String::from_str(&e, "violation"),
            data,
            false,
        )
        .unwrap();
    });

    // Check stored health metrics - compliance score should be reduced by 30 (high severity)
    let stored_metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_stored_health_metrics(e.clone(), commitment_id.clone())
    });

    assert!(stored_metrics.is_some());
    let metrics = stored_metrics.unwrap();
    // Base score is 100, high severity penalty is 30
    assert_eq!(metrics.compliance_score, 70);
}

#[test]
fn test_fees_accumulated_correctly() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    e.ledger().with_mut(|li| li.timestamp = 10000);

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // Record fee generation
    let mut data = Map::new(&e);
    data.set(
        String::from_str(&e, "fee_amount"),
        String::from_str(&e, "100"),
    );

    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            String::from_str(&e, "fee_generation"),
            data.clone(),
            true,
        )
        .unwrap();
    });

    // Record another fee
    e.ledger().with_mut(|li| li.timestamp = 20000);
    let mut data2 = Map::new(&e);
    data2.set(
        String::from_str(&e, "fee_amount"),
        String::from_str(&e, "50"),
    );

    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            String::from_str(&e, "fee_generation"),
            data2,
            true,
        )
        .unwrap();
    });

    // Check stored health metrics - fees should be accumulated
    let stored_metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_stored_health_metrics(e.clone(), commitment_id.clone())
    });

    assert!(stored_metrics.is_some());
    let metrics = stored_metrics.unwrap();
    assert_eq!(metrics.fees_generated, 150);
}

#[test]
fn test_last_attestation_updated() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    let commitment_id = String::from_str(&e, "test_commitment");
    let owner = Address::generate(&e);

    store_core_commitment(
        &e,
        &_commitment_core,
        "test_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // Record first attestation at time 10000
    e.ledger().with_mut(|li| li.timestamp = 10000);
    let data = Map::new(&e);
    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            String::from_str(&e, "health_check"),
            data.clone(),
            true,
        )
        .unwrap();
    });

    let metrics1 = e
        .as_contract(&contract_id, || {
            AttestationEngineContract::get_stored_health_metrics(e.clone(), commitment_id.clone())
        })
        .unwrap();
    assert_eq!(metrics1.last_attestation, 10000);

    // Record second attestation at time 20000
    e.ledger().with_mut(|li| li.timestamp = 20000);
    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            String::from_str(&e, "health_check"),
            data.clone(),
            true,
        )
        .unwrap();
    });

    let metrics2 = e
        .as_contract(&contract_id, || {
            AttestationEngineContract::get_stored_health_metrics(e.clone(), commitment_id.clone())
        })
        .unwrap();
    assert_eq!(metrics2.last_attestation, 20000);
}

// ============================================================================
// Initialize Tests
// ============================================================================

#[test]
fn test_initialize_already_initialized() {
    let (e, admin, commitment_core, contract_id) = setup_test_env();

    // Try to initialize again
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), commitment_core.clone())
    });

    assert_eq!(result, Err(AttestationError::AlreadyInitialized));
}

#[test]
fn test_get_admin() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    let stored_admin = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_admin(e.clone()).unwrap()
    });

    assert_eq!(stored_admin, admin);
}

#[test]
fn test_get_core_contract() {
    let (e, _admin, commitment_core, contract_id) = setup_test_env();

    let stored_core = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_core_contract(e.clone()).unwrap()
    });

    assert_eq!(stored_core, commitment_core);
}

// ============================================================================
// Event Verification Tests
// ============================================================================

#[test]
fn test_attest_event() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    let verified_by = admin.clone();

    let commitment_id = String::from_str(&e, "test_id");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "test_id",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    let attestation_type = String::from_str(&e, "health_check");
    let data = Map::new(&e);

    client.attest(
        &verified_by,
        &commitment_id,
        &attestation_type,
        &data,
        &true,
    );

    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, contract_id);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            Symbol::new(&e, "AttestationRecorded").into_val(&e),
            commitment_id.into_val(&e),
            verified_by.into_val(&e)
        ]
    );
}

#[test]
#[should_panic(expected = "Rate limit exceeded")]
fn test_attest_rate_limit_enforced() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();

    let verifier = admin.clone();

    // Configure rate limit: 1 attestation per 60 seconds for function "attest"
    e.as_contract(&contract_id, || {
        AttestationEngineContract::set_rate_limit(
            e.clone(),
            admin.clone(),
            Symbol::new(&e, "attest"),
            60,
            1,
        )
        .unwrap();
    });

    let commitment_id = String::from_str(&e, "rl_attest");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "rl_attest",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    let attestation_type = String::from_str(&e, "health_check");
    let data = Map::new(&e);

    // First attestation should succeed
    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        )
        .unwrap();
    });

    // Second attestation within same window should panic
    e.as_contract(&contract_id, || {
        let _ = AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            true,
        );
    });
}

#[test]
fn test_protocol_statistics_aggregation() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    e.ledger().with_mut(|li| li.timestamp = 10000);

    let commitment_id = String::from_str(&e, "stats_commitment");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &_commitment_core,
        "stats_commitment",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // Record a fee generation attestation so that fees and counters update
    let mut data = Map::new(&e);
    data.set(
        String::from_str(&e, "fee_amount"),
        String::from_str(&e, "100"),
    );

    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            String::from_str(&e, "fee_generation"),
            data,
            true,
        )
        .unwrap();
    });

    let (total_commitments, total_attestations, total_violations, total_fees) = e
        .as_contract(&contract_id, || {
            AttestationEngineContract::get_protocol_statistics(e.clone())
        });

    // We seeded a commitment directly in core storage; protocol stats read the
    // core's aggregate counter which remains at zero in this isolated test.
    assert_eq!(total_commitments, 0);
    assert_eq!(total_attestations, 1);
    assert_eq!(total_violations, 0);
    assert_eq!(total_fees, 100);
}

#[test]
fn test_record_fees_event() {
    let (e, admin, commitment_core, contract_id) = setup_test_env();
    e.mock_all_auths();
    let client = AttestationEngineContractClient::new(&e, &contract_id);

    let commitment_id = String::from_str(&e, "test_id");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &commitment_core,
        "test_id",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // record_fees requires caller (admin)
    client.record_fees(&admin, &commitment_id, &100);

    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, contract_id);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            Symbol::new(&e, "FeeRecorded").into_val(&e),
            commitment_id.into_val(&e)
        ]
    );
    let event_data: (i128, u64) = last_event.2.into_val(&e);
    assert_eq!(event_data.0, 100);
}

#[test]
fn test_record_drawdown_event() {
    let (e, admin, commitment_core, contract_id) = setup_test_env();
    e.mock_all_auths();
    let client = AttestationEngineContractClient::new(&e, &contract_id);

    // Need to store a commitment first because record_drawdown fetches it
    let commitment_id = String::from_str(&e, "test_id");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &commitment_core,
        "test_id",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    // record_drawdown requires caller (admin) and drawdown_percent
    client.record_drawdown(&admin, &commitment_id, &5); // 5% drawdown

    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, contract_id);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            Symbol::new(&e, "DrawdownRecorded").into_val(&e),
            commitment_id.into_val(&e)
        ]
    );
    let event_data: (i128, bool, u64) = last_event.2.into_val(&e);
    // (drawdown_percent, is_compliant, timestamp)
    assert_eq!(event_data.0, 5);
    assert_eq!(event_data.1, true);
}

#[test]
fn test_calculate_compliance_score_event() {
    let (e, _admin, commitment_core, contract_id) = setup_test_env();
    let client = AttestationEngineContractClient::new(&e, &contract_id);

    // Need to store a commitment first
    let commitment_id = String::from_str(&e, "test_id");
    let owner = Address::generate(&e);
    store_core_commitment(
        &e,
        &commitment_core,
        "test_id",
        &owner,
        1000,
        1000,
        10,
        30,
        1000,
    );

    client.calculate_compliance_score(&commitment_id);

    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, contract_id);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("ScoreUpd").into_val(&e),
            commitment_id.into_val(&e)
        ]
    );
    let event_data: (u32, u64) = last_event.2.into_val(&e);
    assert_eq!(event_data.0, 100);
}
