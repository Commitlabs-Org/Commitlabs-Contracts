
#[test]
fn test_attest_invalid_types() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let client = AttestationEngineContractClient::new(&e, &attestation_id);

    let admin = Address::generate(&e);
    let commitment_id = String::from_str(&e, "commitment_invalid_type");

    client.initialize(&admin, &core_id);
    client.add_verifier(&admin, &admin);

    let commitment = create_mock_commitment_with_status_internal(
        &e,
        "commitment_invalid_type",
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

    let data = Map::new(&e);

    // Empty attestation_type
    let empty_type = String::from_str(&e, "");
    let result = client.try_attest(&admin, &commitment_id, &empty_type, &data, &true);
    assert!(result.is_err());

    // Unknown attestation_type
    let unknown_type = String::from_str(&e, "unknown");
    let result = client.try_attest(&admin, &commitment_id, &unknown_type, &data, &true);
    assert!(result.is_err());

    // Allowed types with required data
    // health_check: no required fields
    let att_type = String::from_str(&e, "health_check");
    let result = client.try_attest(&admin, &commitment_id, &att_type, &Map::new(&e), &true);
    assert!(result.is_ok(), "attest should succeed for allowed type: health_check");

    // violation: requires "violation_type" and "severity"
    let att_type = String::from_str(&e, "violation");
    let mut data = Map::new(&e);
    data.set(String::from_str(&e, "violation_type"), String::from_str(&e, "foo"));
    data.set(String::from_str(&e, "severity"), String::from_str(&e, "high"));
    let result = client.try_attest(&admin, &commitment_id, &att_type, &data, &true);
    assert!(result.is_ok(), "attest should succeed for allowed type: violation");

    // fee_generation: requires "fee_amount"
    let att_type = String::from_str(&e, "fee_generation");
    let mut data = Map::new(&e);
    data.set(String::from_str(&e, "fee_amount"), String::from_str(&e, "100"));
    let result = client.try_attest(&admin, &commitment_id, &att_type, &data, &true);
    assert!(result.is_ok(), "attest should succeed for allowed type: fee_generation");

    // drawdown: requires "drawdown_percent"
    let att_type = String::from_str(&e, "drawdown");
    let mut data = Map::new(&e);
    data.set(String::from_str(&e, "drawdown_percent"), String::from_str(&e, "5"));
    let result = client.try_attest(&admin, &commitment_id, &att_type, &data, &true);
    assert!(result.is_ok(), "attest should succeed for allowed type: drawdown");
}
use super::*;


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

fn setup_initialized_engine_with_core(e: &Env) -> (Address, Address) {
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let admin = Address::generate(e);

    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin, core_id.clone()).unwrap();
    });

    (attestation_id, core_id)
}

#[test]
fn test_get_health_metrics_cross_reads_commitment_core_state() {
    let e = Env::default();
    let (attestation_id, core_id) = setup_initialized_engine_with_core(&e);
    let commitment_id = String::from_str(&e, "cross_read_core_metrics");

    let commitment =
        create_mock_commitment_with_status(&e, "cross_read_core_metrics", "active", 2_000, 1_700, 20);
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let metrics = e.as_contract(&attestation_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id.clone())
    });

    assert_eq!(metrics.commitment_id, commitment_id);
    assert_eq!(metrics.initial_value, 2_000);
    assert_eq!(metrics.current_value, 1_700);
    assert_eq!(metrics.drawdown_percent, 15);
    assert_eq!(metrics.fees_generated, 0);
    assert_eq!(metrics.last_attestation, 0);
}

#[test]
fn test_get_health_metrics_ignores_stale_cached_values_for_core_read_fields() {
    let e = Env::default();
    let (attestation_id, core_id) = setup_initialized_engine_with_core(&e);
    let commitment_id = String::from_str(&e, "cross_read_with_cached_metrics");

    let commitment = create_mock_commitment_with_status(
        &e,
        "cross_read_with_cached_metrics",
        "active",
        1_500,
        1_200,
        25,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let verifier = Address::generate(&e);
    let mut data = Map::new(&e);
    data.set(String::from_str(&e, "fee_amount"), String::from_str(&e, "45"));

    let mut attestations = Vec::new(&e);
    attestations.push_back(Attestation {
        commitment_id: commitment_id.clone(),
        timestamp: 777,
        attestation_type: String::from_str(&e, "fee_generation"),
        data,
        is_compliant: true,
        verified_by: verifier,
    });

    e.as_contract(&attestation_id, || {
        e.storage().persistent().set(
            &DataKey::HealthMetrics(commitment_id.clone()),
            &HealthMetrics {
                commitment_id: commitment_id.clone(),
                current_value: 999,
                initial_value: 999,
                drawdown_percent: 99,
                fees_generated: 999,
                volatility_exposure: 99,
                last_attestation: 999,
                compliance_score: 88,
            },
        );
        e.storage()
            .persistent()
            .set(&DataKey::Attestations(commitment_id.clone()), &attestations);
    });

    let metrics = e.as_contract(&attestation_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id.clone())
    });

    assert_eq!(metrics.initial_value, 1_500);
    assert_eq!(metrics.current_value, 1_200);
    assert_eq!(metrics.drawdown_percent, 20);
    assert_eq!(metrics.fees_generated, 45);
    assert_eq!(metrics.last_attestation, 777);
    assert_eq!(metrics.compliance_score, 88);
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
// Verifier Allowlist Abuse Cases
// ============================================

#[test]
fn test_add_verifier_success() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);
    let verifier = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    assert_eq!(result, Ok(()));

    let is_listed = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(is_listed, "Verifier should be listed after add");
}

#[test]
fn test_add_verifier_duplicate_is_idempotent() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);
    let verifier = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // First add — normal path
    let r1 = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    assert_eq!(r1, Ok(()));

    // Second add — abuse path: idempotent, emits VerifAddAbuse event
    let r2 = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    assert_eq!(r2, Ok(()));

    // Verifier must still be listed
    let still_listed = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(still_listed, "Verifier should remain listed after duplicate add");
}

#[test]
fn test_add_verifier_unauthorized() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);
    let non_admin = Address::generate(&e);
    let verifier = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::add_verifier(e.clone(), non_admin.clone(), verifier.clone())
    });
    assert_eq!(result, Err(AttestationError::Unauthorized));

    // Verifier must not have been added
    let is_listed = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(!is_listed, "Verifier must not be listed after unauthorized add attempt");
}

#[test]
fn test_remove_verifier_success() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);
    let verifier = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    assert_eq!(result, Ok(()));

    let is_listed = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(!is_listed, "Verifier should not be listed after remove");
}

#[test]
fn test_remove_verifier_not_listed_is_idempotent() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);
    let verifier = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
    });

    // verifier was never added; remove is idempotent, emits VerifRmAbuse event
    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier.clone())
    });
    assert_eq!(result, Ok(()));

    let is_listed = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(!is_listed, "Verifier should remain unlisted after no-op remove");
}

#[test]
fn test_remove_verifier_unauthorized() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);
    let non_admin = Address::generate(&e);
    let verifier = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let result = e.as_contract(&contract_id, || {
        AttestationEngineContract::remove_verifier(e.clone(), non_admin.clone(), verifier.clone())
    });
    assert_eq!(result, Err(AttestationError::Unauthorized));

    // Verifier must still be listed
    let still_listed = e.as_contract(&contract_id, || {
        AttestationEngineContract::is_verifier(e.clone(), verifier.clone())
    });
    assert!(still_listed, "Verifier must remain listed after unauthorized remove attempt");
}

#[test]
#[should_panic(expected = "Rate limit exceeded")]
fn test_add_verifier_rate_limit_exceeded() {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().with_mut(|l| l.timestamp = 1000);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
        // 1 add_verifier allowed per 3600-second window
        AttestationEngineContract::set_rate_limit(
            e.clone(),
            admin.clone(),
            Symbol::new(&e, "add_verif"),
            3600u64,
            1u32,
        )
        .unwrap();
    });

    let verifier1 = Address::generate(&e);
    let verifier2 = Address::generate(&e);

    e.as_contract(&contract_id, || {
        // First call — within limit
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier1.clone())
            .unwrap();
        // Second call — exceeds limit, must panic
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier2.clone())
            .unwrap();
    });
}

#[test]
#[should_panic(expected = "Rate limit exceeded")]
fn test_remove_verifier_rate_limit_exceeded() {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().with_mut(|l| l.timestamp = 1000);
    let contract_id = e.register_contract(None, AttestationEngineContract);
    let admin = Address::generate(&e);
    let core = Address::generate(&e);
    let verifier1 = Address::generate(&e);
    let verifier2 = Address::generate(&e);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier1.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier2.clone()).unwrap();
        // 1 remove_verifier allowed per 3600-second window
        AttestationEngineContract::set_rate_limit(
            e.clone(),
            admin.clone(),
            Symbol::new(&e, "rm_verif"),
            3600u64,
            1u32,
        )
        .unwrap();
    });

    e.as_contract(&contract_id, || {
        // First remove — within limit
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier1.clone())
            .unwrap();
        // Second remove — exceeds limit, must panic
        AttestationEngineContract::remove_verifier(e.clone(), admin.clone(), verifier2.clone())
            .unwrap();
    });
}

// ============================================================================
// Comprehensive Attestation Types Tests
// ============================================================================

#[test]
fn test_attestation_types_health_check_validation() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "health_check_test");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "health_check_test",
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

    // Test health check with optional data
    let mut health_data = Map::new(&e);
    health_data.set("status".into(), "healthy".into());
    health_data.set("notes".into(), "All systems operational".into());

    let result = e.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&e, "health_check"),
            health_data,
            true,
        )
    });
    assert_eq!(result, Ok(()));

    // Verify attestation was recorded
    let attestations = e.as_contract(&attestation_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });
    assert_eq!(attestations.len(), 1);
    assert_eq!(
        attestations.get(0).unwrap().attestation_type,
        String::from_str(&e, "health_check")
    );
}

#[test]
fn test_attestation_types_violation_validation() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "violation_test");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "violation_test",
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

    // Test violation with required data
    let mut violation_data = Map::new(&e);
    violation_data.set("violation_type".into(), "rule_breach".into());
    violation_data.set("severity".into(), "medium".into());
    violation_data.set("description".into(), "Exceeded daily limit".into());

    let result = e.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&e, "violation"),
            violation_data,
            false,
        )
    });
    assert_eq!(result, Ok(()));

    // Verify attestation was recorded
    let attestations = e.as_contract(&attestation_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });
    assert_eq!(attestations.len(), 1);
    assert_eq!(
        attestations.get(0).unwrap().attestation_type,
        String::from_str(&e, "violation")
    );
}

#[test]
fn test_attestation_types_violation_missing_required_data_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "violation_missing_data");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "violation_missing_data",
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

    // Test violation with missing severity
    let mut incomplete_data = Map::new(&e);
    incomplete_data.set("violation_type".into(), "rule_breach".into());
    // Missing "severity" field

    let result = e.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&e, "violation"),
            incomplete_data,
            false,
        )
    });
    assert_eq!(result, Err(AttestationError::InvalidAttestationData));
}

#[test]
fn test_attestation_types_fee_generation_validation() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "fee_test");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "fee_test",
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

    // Test fee generation with required data
    let mut fee_data = Map::new(&e);
    fee_data.set("fee_amount".into(), "500000".into());
    fee_data.set("fee_type".into(), "performance".into());

    let result = e.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&e, "fee_generation"),
            fee_data,
            true,
        )
    });
    assert_eq!(result, Ok(()));

    // Verify attestation was recorded
    let attestations = e.as_contract(&attestation_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });
    assert_eq!(attestations.len(), 1);
    assert_eq!(
        attestations.get(0).unwrap().attestation_type,
        String::from_str(&e, "fee_generation")
    );
}

#[test]
fn test_attestation_types_drawdown_validation() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "drawdown_test");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "drawdown_test",
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

    // Test drawdown with required data
    let mut drawdown_data = Map::new(&e);
    drawdown_data.set("drawdown_percent".into(), "15".into());
    drawdown_data.set("trigger_event".into(), "market_crash".into());

    let result = e.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&e, "drawdown"),
            drawdown_data,
            false, // 15% exceeds 10% limit
        )
    });
    assert_eq!(result, Ok(()));

    // Verify attestation was recorded
    let attestations = e.as_contract(&attestation_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });
    assert_eq!(attestations.len(), 1);
    assert_eq!(
        attestations.get(0).unwrap().attestation_type,
        String::from_str(&e, "drawdown")
    );
}

#[test]
fn test_attestation_types_invalid_type_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "invalid_type_test");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "invalid_type_test",
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

    // Test invalid attestation type
    let data = Map::new(&e);
    let result = e.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&e, "invalid_type"),
            data,
            true,
        )
    });
    assert_eq!(result, Err(AttestationError::InvalidAttestationType));
}

// ============================================================================
// Comprehensive Compliance Scoring Tests
// ============================================================================

#[test]
fn test_compliance_scoring_perfect_score() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "perfect_score");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "perfect_score",
        "active",
        1000,
        1000, // No loss
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Record compliant health checks
    for i in 0..3 {
        let mut health_data = Map::new(&e);
        health_data.set("check_number".into(), (i + 1).to_string().into());
        
        e.as_contract(&attestation_id, || {
            AttestationEngineContract::attest(
                e.clone(),
                verifier.clone(),
                commitment_id.clone(),
                String::from_str(&e, "health_check"),
                health_data,
                true,
            )
        }).unwrap();
    }

    let score = e.as_contract(&attestation_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id.clone())
    });

    // Should be 100 + 3 (compliant bonus) + 10 (duration bonus) = 113, capped at 100
    assert_eq!(score, 100);
}

#[test]
fn test_compliance_scoring_with_violations() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "violations_score");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "violations_score",
        "active",
        1000,
        1000,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Record a high severity violation
    let mut violation_data = Map::new(&e);
    violation_data.set("violation_type".into(), "rule_breach".into());
    violation_data.set("severity".into(), "high".into());

    e.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&e, "violation"),
            violation_data,
            false,
        )
    }).unwrap();

    // Record a medium severity violation
    let mut violation_data2 = Map::new(&e);
    violation_data2.set("violation_type".into(), "delay".into());
    violation_data2.set("severity".into(), "medium".into());

    e.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&e, "violation"),
            violation_data2,
            false,
        )
    }).unwrap();

    let score = e.as_contract(&attestation_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id.clone())
    });

    // Base 100 - 30 (high) - 20 (medium) + 10 (duration) = 60
    assert_eq!(score, 60);
}

#[test]
fn test_compliance_scoring_with_drawdown_exceeding_threshold() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "drawdown_score");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    // Create commitment with 20% current drawdown (exceeding 10% threshold)
    let commitment = create_mock_commitment_with_status(
        &e,
        "drawdown_score",
        "active",
        1000,
        800, // 20% loss
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    let score = e.as_contract(&attestation_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id.clone())
    });

    // Base 100 - 10 (over threshold: 20-10) + 10 (duration) = 100
    assert_eq!(score, 100);
}

#[test]
fn test_compliance_scoring_with_fee_performance() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "fee_performance_score");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "fee_performance_score",
        "active",
        1000,
        1000,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Record substantial fee generation (exceeding threshold)
    let fee_amount = commitment.rules.min_fee_threshold * 2; // 200% of threshold
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::record_fees(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            fee_amount,
        )
    }).unwrap();

    let score = e.as_contract(&attestation_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id.clone())
    });

    // Base 100 + 100 (fee bonus capped) + 10 (duration) = 210, capped at 100
    assert_eq!(score, 100);
}

#[test]
fn test_compliance_scoring_minimum_score() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "minimum_score");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    // Create commitment with severe drawdown
    let commitment = create_mock_commitment_with_status(
        &e,
        "minimum_score",
        "active",
        1000,
        500, // 50% loss
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // Record multiple high severity violations
    for _ in 0..5 {
        let mut violation_data = Map::new(&e);
        violation_data.set("violation_type".into(), "critical_breach".into());
        violation_data.set("severity".into(), "high".into());

        e.as_contract(&attestation_id, || {
            AttestationEngineContract::attest(
                e.clone(),
                verifier.clone(),
                commitment_id.clone(),
                String::from_str(&e, "violation"),
                violation_data,
                false,
            )
        }).unwrap();
    }

    let score = e.as_contract(&attestation_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id.clone())
    });

    // Base 100 - 150 (5 * 30 high violations) - 40 (over threshold: 50-10) + 10 (duration) = -80, clamped to 0
    assert_eq!(score, 0);
}

#[test]
fn test_compliance_scoring_stored_metrics_priority() {
    let e = Env::default();
    e.mock_all_auths();
    let attestation_id = e.register_contract(None, AttestationEngineContract);
    let core_id = e.register_contract(None, commitment_core::CommitmentCoreContract);

    let admin = Address::generate(&e);
    let verifier = Address::generate(&e);
    let commitment_id = String::from_str(&e, "stored_metrics_test");

    // Setup
    e.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_id.clone()).unwrap();
        AttestationEngineContract::add_verifier(e.clone(), admin.clone(), verifier.clone()).unwrap();
    });

    let commitment = create_mock_commitment_with_status(
        &e,
        "stored_metrics_test",
        "active",
        1000,
        1000,
        10,
    );
    e.as_contract(&core_id, || {
        e.storage().instance().set(
            &commitment_core::DataKey::Commitment(commitment_id.clone()),
            &commitment,
        );
    });

    // First, record some attestations to generate a score
    let mut violation_data = Map::new(&e);
    violation_data.set("violation_type".into(), "test".into());
    violation_data.set("severity".into(), "medium".into());

    e.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&e, "violation"),
            violation_data,
            false,
        )
    }).unwrap();

    // Get initial score
    let initial_score = e.as_contract(&attestation_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id.clone())
    });

    // Now manually set stored metrics with a different score
    let stored_metrics = HealthMetrics {
        commitment_id: commitment_id.clone(),
        current_value: 1000,
        initial_value: 1000,
        drawdown_percent: 0,
        fees_generated: 0,
        volatility_exposure: 0,
        last_attestation: e.ledger().timestamp(),
        compliance_score: 25, // Different from calculated score
    };

    e.as_contract(&attestation_id, || {
        let key = super::DataKey::HealthMetrics(commitment_id.clone());
        e.storage().persistent().set(&key, &stored_metrics);
    });

    // Score should return stored value, not recalculate
    let stored_score = e.as_contract(&attestation_id, || {
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id.clone())
    });

    assert_eq!(stored_score, 25);
    assert_ne!(stored_score, initial_score);
}
