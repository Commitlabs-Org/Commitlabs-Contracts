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

// ============================================
// add_verifier/remove_verifier Admin Tests
// ============================================

#[test]
fn test_add_verifier_admin_success() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Add verifier as admin
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    
    assert_eq!(result, Ok(()));

    // Verify verifier was added
    let is_verifier = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(is_verifier);

    // Verify verifier is authorized
    let is_authorized = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_authorized(e.clone(), verifier.clone())
    });
    assert!(is_authorized);
}

#[test]
fn test_add_verifier_non_admin_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let non_admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Try to add verifier as non-admin
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), non_admin.clone(), verifier.clone())
    });
    
    assert_eq!(result, Err(AttestationError::Unauthorized));

    // Verify verifier was not added
    let is_verifier = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(!is_verifier);
}

#[test]
fn test_add_verifier_uninitialized_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);

    // Try to add verifier without initialization
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    
    assert_eq!(result, Err(AttestationError::NotInitialized));
}

#[test]
fn test_add_verifier_emits_event() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Add verifier and check for event
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    
    assert_eq!(result, Ok(()));

    // Check that event was emitted
    let events = e.events().all();
    assert_eq!(events.len(), 1);
}

#[test]
fn test_add_verifier_admin_always_authorized() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Admin should be authorized even without being explicitly added as verifier
    let admin_is_authorized = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_authorized(e.clone(), admin.clone())
    });
    assert!(admin_is_authorized);

    let admin_is_verifier = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), admin.clone())
    });
    assert!(admin_is_verifier);
}

#[test]
fn test_remove_verifier_admin_success() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Add verifier first
    e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    // Verify verifier was added
    let is_verifier_before = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(is_verifier_before);

    // Remove verifier as admin
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    
    assert_eq!(result, Ok(()));

    // Verify verifier was removed
    let is_verifier_after = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(!is_verifier_after);

    // Verify verifier is no longer authorized
    let is_authorized_after = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_authorized(e.clone(), verifier.clone())
    });
    assert!(!is_authorized_after);
}

#[test]
fn test_remove_verifier_non_admin_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let non_admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Add verifier first
    e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    // Try to remove verifier as non-admin
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), non_admin.clone(), verifier.clone())
    });
    
    assert_eq!(result, Err(AttestationError::Unauthorized));

    // Verify verifier was not removed
    let is_verifier = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(is_verifier);
}

#[test]
fn test_remove_verifier_uninitialized_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);

    // Try to remove verifier without initialization
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    
    assert_eq!(result, Err(AttestationError::NotInitialized));
}

#[test]
fn test_remove_verifier_emits_event() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Add verifier first
    e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    // Clear events from adding verifier
    e.events().all();

    // Remove verifier and check for event
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    
    assert_eq!(result, Ok(()));

    // Check that event was emitted
    let events = e.events().all();
    assert_eq!(events.len(), 1);
    // Event was emitted - structure verified by event count
}

#[test]
fn test_remove_nonexistent_verifier_succeeds() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Try to remove verifier that was never added - should succeed
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    
    assert_eq!(result, Ok(()));

    // Verify verifier is not authorized
    let is_verifier = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(!is_verifier);
}

#[test]
fn test_add_remove_multiple_verifiers() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let verifier1 = Address::generate(&e);
    let verifier2 = Address::generate(&e);
    let verifier3 = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Add multiple verifiers
    e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier1.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier2.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier3.clone()).unwrap();
    });

    // Verify all verifiers are authorized
    for verifier in [verifier1.clone(), verifier2.clone(), verifier3.clone()] {
        let is_authorized = e.as_contract(&contract_id, || {
            AttestationEngineContract::is_authorized(e.clone(), verifier.clone())
        });
        assert!(is_authorized);
    }

    // Remove one verifier
    e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier2.clone()).unwrap();
    });

    // Verify correct authorization state
    let verifier1_authorized = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_authorized(e.clone(), verifier1.clone())
    });
    assert!(verifier1_authorized);

    let verifier2_authorized = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_authorized(e.clone(), verifier2.clone())
    });
    assert!(!verifier2_authorized);

    let verifier3_authorized = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_authorized(e.clone(), verifier3.clone())
    });
    assert!(verifier3_authorized);

    // Admin should still be authorized
    let admin_authorized = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_authorized(e.clone(), admin.clone())
    });
    assert!(admin_authorized);
}

#[test]
fn test_add_authorized_contract_alias() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let contract_address = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Test add_authorized_contract alias
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_authorized_contract(e.clone(), admin.clone(), contract_address.clone())
    });
    
    assert_eq!(result, Ok(()));

    // Verify contract is authorized
    let is_authorized = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_authorized(e.clone(), contract_address.clone())
    });
    assert!(is_authorized);
}

#[test]
fn test_remove_authorized_contract_alias() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    
    let admin = Address::generate(&e);
    let contract_address = Address::generate(&e);
    let core = Address::generate(&e);

    // Initialize contract
    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // Add contract first
    e.as_contract(&contract_id, || {
        AttestationEngineContract::add_authorized_contract(e.clone(), admin.clone(), contract_address.clone()).unwrap();
    });

    // Test remove_authorized_contract alias
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_authorized_contract(e.clone(), admin.clone(), contract_address.clone())
    });
    
    assert_eq!(result, Ok(()));

    // Verify contract is no longer authorized
    let is_authorized = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_authorized(e.clone(), contract_address.clone())
    });
    assert!(!is_authorized);
}
