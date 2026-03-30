#![cfg(test)]

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
