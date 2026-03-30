#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    Address, Env, Map, String, Vec,
};

fn create_mock_commitment_with_status_internal(
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
fn test_initialize_and_getters() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);

    client.initialize(&admin, &core);
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_core_contract(), core);
}

#[test]
fn test_initialize_twice_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);

    client.initialize(&admin, &core);
    let result = client.try_initialize(&admin, &core);
    assert!(result.is_err());
}

#[test]
fn test_verify_compliance_settled_commitment_returns_true() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_settled");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "test_commitment_settled",
        "settled",
        1000,
        1050,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    assert!(is_compliant);
}

#[test]
fn test_verify_compliance_violated_commitment_returns_false() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_violated");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "test_commitment_violated",
        "violated",
        1000,
        850,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    assert!(!is_compliant);
}

#[test]
fn test_verify_compliance_early_exit_returns_false() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_early_exit");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "test_commitment_early_exit",
        "early_exit",
        1000,
        980,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    assert!(!is_compliant);
}

#[test]
fn test_verify_compliance_active_commitment_within_rules_returns_true() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_active_compliant");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "test_commitment_active_compliant",
        "active",
        1000,
        950,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    assert!(is_compliant);
}

#[test]
fn test_verify_compliance_active_commitment_exceeds_loss_returns_false() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_active_noncompliant");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "test_commitment_active_noncompliant",
        "active",
        1000,
        850,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    assert!(!is_compliant);
}

#[test]
fn test_verify_compliance_nonexistent_commitment_returns_false() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "nonexistent_commitment");

    client.initialize(&admin, &core_id);

    let is_compliant = client.verify_compliance(&commitment_id);
    assert!(!is_compliant);
}

#[test]
fn test_attest_without_initialize_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let client = AttestationEngineContractClient::new(&e, &contract_id);

    let caller = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment");
    let attestation_type = String::from_str(&e, "health_check");
    let data = Map::new(&e);

    let result = client.try_attest(&caller, &commitment_id, &attestation_type, &data, &true);
    assert!(result.is_err());
}

#[test]
fn test_record_fees_records_attestation_and_metrics() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "commitment_fee");

    client.initialize(&admin, &core_id);
    client.add_verifier(&admin, &admin);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "commitment_fee",
        "active",
        1_000,
        1_000,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    client.record_fees(&admin, &commitment_id, &250);

    let attestations = client.get_attestations(&commitment_id);
    assert_eq!(attestations.len(), 1);

    let attestation = attestations.get(0).unwrap();
    assert_eq!(attestation.attestation_type, String::from_str(&e, "fee_generation"));
    assert!(attestation.is_compliant);

    let metrics = client.get_stored_health_metrics(&commitment_id).unwrap();
    assert_eq!(metrics.fees_generated, 250);
}

#[test]
fn test_record_drawdown_within_max_loss_records_drawdown() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "commitment_drawdown");

    client.initialize(&admin, &core_id);
    client.add_verifier(&admin, &admin);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "commitment_drawdown",
        "active",
        1_000,
        1_000,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    client.record_drawdown(&admin, &commitment_id, &5);

    let attestations = client.get_attestations(&commitment_id);
    assert_eq!(attestations.len(), 1);

    let attestation = attestations.get(0).unwrap();
    assert_eq!(attestation.attestation_type, String::from_str(&e, "drawdown"));
    assert!(attestation.is_compliant);

    let metrics = client.get_stored_health_metrics(&commitment_id).unwrap();
    assert_eq!(metrics.drawdown_percent, 5);
}

#[test]
fn test_get_attestations_page_logic() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test_commitment_pagination");

    client.initialize(&admin, &core_id);
    client.add_verifier(&admin, &admin);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "test_commitment_pagination",
        "active",
        1000,
        950,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // 1. Test empty attestations
    let page = client.get_attestations_page(&commitment_id, &0, &10);
    assert_eq!(page.attestations.len(), 0);
    assert_eq!(page.next_offset, 0);

    let start_ts = e.ledger().timestamp();
    // 2. Add 15 attestations with increasing timestamps
    for _ in 0..15u32 {
        let data = Map::new(&e);
        e.ledger().with_mut(|l| l.timestamp += 1);
        client.attest(&admin, &commitment_id, &String::from_str(&e, "health_check"), &data, &true);
    }

    // 3. Test first page: offset=0, limit=10
    let page1 = client.get_attestations_page(&commitment_id, &0, &10);
    assert_eq!(page1.attestations.len(), 10);
    assert_eq!(page1.next_offset, 10);

    // Verify ordering
    for i in 0..10u32 {
        let att = page1.attestations.get(i).unwrap();
        assert_eq!(att.timestamp, start_ts + (i as u64) + 1);
    }

    // 4. Test second page: offset=10, limit=10
    let page2 = client.get_attestations_page(&commitment_id, &10, &10);
    assert_eq!(page2.attestations.len(), 5);
    assert_eq!(page2.next_offset, 0);

    // Verify ordering
    for i in 0..5u32 {
        let att = page2.attestations.get(i).unwrap();
        assert_eq!(att.timestamp, start_ts + (i as u64) + 11);
    }

    // 5. Test MAX_PAGE_SIZE boundary
    for _ in 15..150u32 {
        let data = Map::new(&e);
        client.attest(&admin, &commitment_id, &String::from_str(&e, "health_check"), &data, &true);
    }

    let page_max = client.get_attestations_page(&commitment_id, &0, &200);
    assert_eq!(page_max.attestations.len(), 100);
    assert_eq!(page_max.next_offset, 100);

    // 6. Test edge cases
    let page_end = client.get_attestations_page(&commitment_id, &150, &10);
    assert_eq!(page_end.attestations.len(), 0);
    assert_eq!(page_end.next_offset, 0);

    let page_zero = client.get_attestations_page(&commitment_id, &0, &0);
    assert_eq!(page_zero.attestations.len(), 0);
    assert_eq!(page_zero.next_offset, 0);
}

// ========================================================================
// Edge Case Tests for calculate_compliance_score
// ========================================================================

#[test]
fn test_calculate_compliance_score_zero_initial_value() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "zero_initial_commitment");

    client.initialize(&admin, &core_id);

    // Create commitment with zero initial value
    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "zero_initial_commitment",
        "active",
        0, // Zero initial value
        1000, // Positive current value
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let score = client.calculate_compliance_score(&commitment_id);
    // Should handle zero division gracefully and return valid score
    assert!(score <= 100);
}

#[test]
fn test_calculate_compliance_score_negative_values() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "negative_values_commitment");

    client.initialize(&admin, &core_id);

    // Create commitment with negative current value
    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "negative_values_commitment",
        "active",
        1000,
        -500, // Negative current value
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let score = client.calculate_compliance_score(&commitment_id);
    // Should handle negative values without panicking
    assert!(score <= 100);
}

#[test]
fn test_calculate_compliance_score_empty_attestations() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "empty_attestations_commitment");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "empty_attestations_commitment",
        "active",
        1000,
        950,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // No attestations recorded
    let score = client.calculate_compliance_score(&commitment_id);
    // Should return base score with no violations
    assert!(score >= 90); // Should be close to 100 with duration bonus
}

#[test]
fn test_calculate_compliance_score_multiple_violations() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "multiple_violations_commitment");

    client.initialize(&admin, &core_id);
    client.add_verifier(&admin, &admin);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "multiple_violations_commitment",
        "active",
        1000,
        950,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Add multiple violation attestations
    for i in 0..6 {
        let mut data = Map::new(&e);
        data.set(String::from_str(&e, "violation_type"), String::from_str(&e, "rule_break"));
        data.set(String::from_str(&e, "severity"), String::from_str(&e, "high"));
        client.attest(&admin, &commitment_id, &String::from_str(&e, "violation"), &data, &false);
    }

    let score = client.calculate_compliance_score(&commitment_id);
    // With 6 violations (-20 each), score should be 0 (clamped at minimum)
    assert_eq!(score, 0);
}

#[test]
fn test_calculate_compliance_score_stored_metrics_priority() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "stored_metrics_commitment");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "stored_metrics_commitment",
        "active",
        1000,
        950,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Manually set stored health metrics with specific compliance score
    let stored_metrics = HealthMetrics {
        commitment_id: commitment_id.clone(),
        current_value: 950,
        initial_value: 1000,
        drawdown_percent: 5,
        fees_generated: 0,
        volatility_exposure: 0,
        last_attestation: e.ledger().timestamp(),
        compliance_score: 75, // Specific score to test priority
    };

    e.as_contract(&attestation_id, || {
        let key = DataKey::HealthMetrics(commitment_id.clone());
        e.storage().persistent().set(&key, &stored_metrics);
    });

    let score = client.calculate_compliance_score(&commitment_id);
    // Should return stored score, not recalculate
    assert_eq!(score, 75);
}

#[test]
fn test_calculate_compliance_score_extreme_drawdown() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "extreme_drawdown_commitment");

    client.initialize(&admin, &core_id);

    // Create commitment with extreme drawdown (90% loss)
    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "extreme_drawdown_commitment",
        "active",
        1000,
        100, // 90% loss
        10,  // 10% max loss threshold
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let score = client.calculate_compliance_score(&commitment_id);
    // Should heavily penalize for exceeding drawdown threshold
    assert!(score <= 50); // Should be significantly reduced
}

#[test]
fn test_calculate_compliance_score_zero_fee_threshold() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "zero_fee_threshold_commitment");

    client.initialize(&admin, &core_id);

    let mut commitment = create_mock_commitment_with_status_internal(
        &e,
        "zero_fee_threshold_commitment",
        "active",
        1000,
        950,
        10,
    );
    commitment.rules.min_fee_threshold = 0; // Zero fee threshold

    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let score = client.calculate_compliance_score(&commitment_id);
    // Should handle zero fee threshold without division by zero
    assert!(score <= 100);
}

#[test]
fn test_calculate_compliance_score_invalid_timestamps() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "invalid_timestamps_commitment");

    client.initialize(&admin, &core_id);

    let mut commitment = create_mock_commitment_with_status_internal(
        &e,
        "invalid_timestamps_commitment",
        "active",
        1000,
        950,
        10,
    );
    // Set invalid timestamps (expires before created)
    commitment.created_at = 1000;
    commitment.expires_at = 500; // Expires before created

    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let score = client.calculate_compliance_score(&commitment_id);
    // Should handle invalid timestamps gracefully
    assert!(score <= 100);
}

// ========================================================================
// Edge Case Tests for verify_compliance
// ========================================================================

#[test]
fn test_verify_compliance_zero_max_loss_percent() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "zero_max_loss_commitment");

    client.initialize(&admin, &core_id);

    let mut commitment = create_mock_commitment_with_status_internal(
        &e,
        "zero_max_loss_commitment",
        "active",
        1000,
        999, // Small loss
        0,   // Zero max loss percent
    );
    commitment.rules.max_loss_percent = 0;

    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    // Should be non-compliant since any loss > 0% exceeds threshold
    assert!(!is_compliant);
}

#[test]
fn test_verify_compliance_boundary_compliance_score() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "boundary_score_commitment");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "boundary_score_commitment",
        "active",
        1000,
        950,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Manually set health metrics with boundary compliance score (exactly 50)
    let boundary_metrics = HealthMetrics {
        commitment_id: commitment_id.clone(),
        current_value: 950,
        initial_value: 1000,
        drawdown_percent: 5,
        fees_generated: 0,
        volatility_exposure: 0,
        last_attestation: e.ledger().timestamp(),
        compliance_score: 50, // Exactly at boundary
    };

    e.as_contract(&attestation_id, || {
        let key = DataKey::HealthMetrics(commitment_id.clone());
        e.storage().persistent().set(&key, &boundary_metrics);
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    // Should be compliant with score exactly 50
    assert!(is_compliant);
}

#[test]
fn test_verify_compliance_below_boundary_score() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "below_boundary_commitment");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "below_boundary_commitment",
        "active",
        1000,
        950,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Manually set health metrics with score below boundary
    let below_boundary_metrics = HealthMetrics {
        commitment_id: commitment_id.clone(),
        current_value: 950,
        initial_value: 1000,
        drawdown_percent: 5,
        fees_generated: 0,
        volatility_exposure: 0,
        last_attestation: e.ledger().timestamp(),
        compliance_score: 49, // Just below boundary
    };

    e.as_contract(&attestation_id, || {
        let key = DataKey::HealthMetrics(commitment_id.clone());
        e.storage().persistent().set(&key, &below_boundary_metrics);
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    // Should be non-compliant with score below 50
    assert!(!is_compliant);
}

#[test]
fn test_verify_compliance_missing_health_metrics() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "no_metrics_commitment");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "no_metrics_commitment",
        "active",
        1000,
        950,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Don't set any health metrics - should use defaults
    let is_compliant = client.verify_compliance(&commitment_id);
    // Should handle missing metrics gracefully
    assert!(is_compliant); // Default metrics should be compliant
}

#[test]
fn test_verify_compliance_unknown_status() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "unknown_status_commitment");

    client.initialize(&admin, &core_id);

    let mut commitment = create_mock_commitment_with_status_internal(
        &e,
        "unknown_status_commitment",
        "active",
        1000,
        950,
        10,
    );
    commitment.status = String::from_str(&e, "unknown_status"); // Unknown status

    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    // Unknown status should default to false
    assert!(!is_compliant);
}

#[test]
fn test_verify_compliance_core_contract_not_initialized() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "no_core_contract_commitment");

    // Initialize without setting core contract
    client.initialize(&admin, &Address::generate(&e));

    // Manually clear the core contract to simulate uninitialized state
    e.as_contract(&attestation_id, || {
        e.storage().instance().remove(&DataKey::CoreContract);
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    // Should return false when core contract is not set
    assert!(!is_compliant);
}

// ========================================================================
// Additional Comprehensive Edge Case Tests
// ========================================================================

#[test]
fn test_calculate_compliance_score_overflow_protection() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "overflow_protection_commitment");

    client.initialize(&admin, &core_id);

    // Create commitment with very large values to test overflow protection
    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "overflow_protection_commitment",
        "active",
        i128::MAX / 2, // Very large initial value
        i128::MIN / 2, // Very negative current value
        100,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let score = client.calculate_compliance_score(&commitment_id);
    // Should handle extreme values without panicking
    assert!(score <= 100);
}

#[test]
fn test_calculate_compliance_score_boundary_values() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "boundary_values_commitment");

    client.initialize(&admin, &core_id);

    // Test exact boundary conditions
    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "boundary_values_commitment",
        "active",
        1000,
        900, // Exactly 10% loss (at threshold)
        10,  // 10% max loss
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let score = client.calculate_compliance_score(&commitment_id);
    // Should be exactly at threshold for drawdown penalty
    assert!(score >= 90 && score <= 100); // Should be close to max score
}

#[test]
fn test_verify_compliance_edge_case_timestamps() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "edge_timestamps_commitment");

    client.initialize(&admin, &core_id);

    let mut commitment = create_mock_commitment_with_status_internal(
        &e,
        "edge_timestamps_commitment",
        "active",
        1000,
        950,
        10,
    );
    // Set timestamps to create edge cases
    commitment.created_at = 0;
    commitment.expires_at = u64::MAX;

    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    // Should handle extreme timestamp values
    assert!(is_compliant); // Should be compliant with good metrics
}

#[test]
fn test_verify_compliance_zero_values() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "zero_values_commitment");

    client.initialize(&admin, &core_id);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "zero_values_commitment",
        "active",
        0,    // Zero amount
        0,    // Zero current value
        0,    // Zero max loss percent
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let is_compliant = client.verify_compliance(&commitment_id);
    // Should handle zero values gracefully
    assert!(is_compliant); // Default metrics should make it compliant
}

#[test]
fn test_calculate_compliance_score_mixed_attestations() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "mixed_attestations_commitment");

    client.initialize(&admin, &core_id);
    client.add_verifier(&admin, &admin);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "mixed_attestations_commitment",
        "active",
        1000,
        950,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Add mixed attestations: some compliant, some violations
    let mut data = Map::new(&e);
    data.set(String::from_str(&e, "violation_type"), String::from_str(&e, "rule_break"));
    data.set(String::from_str(&e, "severity"), String::from_str(&e, "medium"));
    client.attest(&admin, &commitment_id, &String::from_str(&e, "violation"), &data, &false);
    
    let data2 = Map::new(&e);
    client.attest(&admin, &commitment_id, &String::from_str(&e, "health_check"), &data2, &true);

    let score = client.calculate_compliance_score(&commitment_id);
    // Should have reduced score due to violation but not zero
    assert!(score >= 70 && score <= 90); // One violation (-20) from base 100
}
