#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Ledger as _, testutils::Events, Address, Env, String, symbol_short, vec, IntoVal, Map};
use commitment_core::{Commitment as CoreCommitment, CommitmentCoreContract, CommitmentRules as CoreCommitmentRules, DataKey};

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
        },
        amount,
        asset_address: Address::generate(e),
        created_at,
        expires_at,
        current_value,
        status: String::from_str(e, "active"),
    };

    e.as_contract(commitment_core_id, || {
        e.storage().instance().set(&DataKey::Commitment(commitment.commitment_id.clone()), &commitment);
    });
}

// Helper function to set up test environment with registered commitment_core contract
fn setup_test_env() -> (Env, Address, Address, Address) {
    let e = Env::default();
    let admin = Address::generate(&e);
    
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
        AttestationEngineContract::initialize(e.clone(), admin.clone(), commitment_core_id.clone());
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
    data.set(String::from_str(&e, "note"), String::from_str(&e, "test attestation"));
    
    // Record an attestation
    e.as_contract(&contract_id, || {
        AttestationEngineContract::attest(
            e.clone(),
            commitment_id.clone(),
            attestation_type.clone(),
            data.clone(),
            admin.clone(),
        );
    });
    
    // Get attestations and verify
    let attestations = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone())
    });
    
    assert_eq!(attestations.len(), 1);
    assert_eq!(attestations.get(0).unwrap().attestation_type, attestation_type);
    
    // Get health metrics and verify last_attestation is updated
    let metrics = e.as_contract(&contract_id, || {
        AttestationEngineContract::get_health_metrics(e.clone(), commitment_id.clone())
    });
    
    assert!(metrics.last_attestation > 0);
}

// Event Verification Tests

#[test]
fn test_attest_event() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    let client = AttestationEngineContractClient::new(&e, &contract_id);
    let verified_by = admin.clone();

    let commitment_id = String::from_str(&e, "test_id");
    let attestation_type = String::from_str(&e, "health_check");
    let data = Map::new(&e);

    client.attest(&commitment_id, &attestation_type, &data, &verified_by);

    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, contract_id);
    assert_eq!(
        last_event.1,
        vec![&e, symbol_short!("Attest").into_val(&e), commitment_id.into_val(&e), verified_by.into_val(&e)]
    );
    let event_data: (String, bool, u64) = last_event.2.into_val(&e);
    assert_eq!(event_data.0, attestation_type);
    assert_eq!(event_data.1, true);
}

#[test]
fn test_record_fees_event() {
    let (e, admin, _commitment_core, contract_id) = setup_test_env();
    let client = AttestationEngineContractClient::new(&e, &contract_id);

    let commitment_id = String::from_str(&e, "test_id");
    // record_fees requires caller (admin)
    client.record_fees(&admin, &commitment_id, &100);

    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, contract_id);
    assert_eq!(
        last_event.1,
        vec![&e, symbol_short!("FeeRec").into_val(&e), commitment_id.into_val(&e)]
    );
    let event_data: (i128, u64) = last_event.2.into_val(&e);
    assert_eq!(event_data.0, 100);
}

#[test]
fn test_record_drawdown_event() {
    let (e, admin, commitment_core, contract_id) = setup_test_env();
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

    // record_drawdown requires caller (admin) and current_value
    client.record_drawdown(&admin, &commitment_id, &950); // 5% drawdown

    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, contract_id);
    assert_eq!(
        last_event.1,
        vec![&e, symbol_short!("Drawdown").into_val(&e), commitment_id.into_val(&e)]
    );
    let event_data: (i128, i128, u64) = last_event.2.into_val(&e);
    // (current_value, drawdown_percent, timestamp)
    assert_eq!(event_data.0, 950);
    assert_eq!(event_data.1, 5);
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
        vec![&e, symbol_short!("ScoreUpd").into_val(&e), commitment_id.into_val(&e)]
    );
    let event_data: (u32, u64) = last_event.2.into_val(&e);
    assert_eq!(event_data.0, 100);
}
