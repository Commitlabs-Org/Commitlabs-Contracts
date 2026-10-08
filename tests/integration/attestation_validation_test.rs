//! Standalone test for attestation validation
//! Tests that attestations fail for nonexistent commitments

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, Map, String,
};

use attestation_engine::{AttestationEngineContract, AttestationError};
use commitment_core::CommitmentCoreContract;

/// Unique nonzero 32-byte evidence id per call (test-only helper).
fn ev_seq(e: &Env) -> BytesN<32> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(1);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&n.to_be_bytes());
    BytesN::from_array(e, &b)
}


#[test]
fn test_attest_nonexistent_commitment_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let verifier = Address::generate(&env);
    
    // Register contracts
    let core_id = env.register(CommitmentCoreContract, ());
    let attestation_id = env.register(AttestationEngineContract, ());

    // Initialize attestation engine with core contract reference
    env.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(
            env.clone(),
            admin.clone(),
            core_id.clone(),
        );
    });

    // Add verifier
    env.as_contract(&attestation_id, || {
        AttestationEngineContract::add_verifier(
            env.clone(),
            admin.clone(),
            verifier.clone(),
        );
    });

    // Register attestation type
    env.as_contract(&attestation_id, || {
        AttestationEngineContract::register_attestation_type(
            env.clone(),
            admin.clone(),
            String::from_str(&env, "health_check"),
        );
    });

    // Try to attest for nonexistent commitment
    let fake_commitment_id = String::from_str(&env, "nonexistent_123");
    let data = Map::new(&env);
    
    let result = env.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            env.clone(),
            verifier.clone(),
            fake_commitment_id,
            String::from_str(&env, "health_check"),
            data,
            true,
                ev_seq(&env.clone()),
        )
    });

    // Should fail with CommitmentNotFound error
    assert_eq!(result, Err(AttestationError::CommitmentNotFound));
}

#[test]
fn test_attest_succeeds_after_commitment_created() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let verifier = Address::generate(&env);
    let token = Address::generate(&env);
    
    // Register contracts
    let core_id = env.register(CommitmentCoreContract, ());
    let attestation_id = env.register(AttestationEngineContract, ());

    // Initialize core contract
    env.as_contract(&core_id, || {
        CommitmentCoreContract::initialize(
            env.clone(),
            admin.clone(),
            Address::generate(&env), // nft_contract
        );
    });

    // Initialize attestation engine
    env.as_contract(&attestation_id, || {
        AttestationEngineContract::initialize(
            env.clone(),
            admin.clone(),
            core_id.clone(),
        );
    });

    // Add verifier
    env.as_contract(&attestation_id, || {
        AttestationEngineContract::add_verifier(
            env.clone(),
            admin.clone(),
            verifier.clone(),
        );
    });

    // Register attestation type
    env.as_contract(&attestation_id, || {
        AttestationEngineContract::register_attestation_type(
            env.clone(),
            admin.clone(),
            String::from_str(&env, "health_check"),
        );
    });

    // Create a commitment in core contract
    let amount = 1_000_000i128;
    let rules = commitment_core::CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&env, "balanced"),
        early_exit_penalty: 10,
        min_fee_threshold: 1000,
        grace_period_days: 0,
    };

    let commitment_id = env.as_contract(&core_id, || {
        CommitmentCoreContract::create_commitment(
            env.clone(),
            user.clone(),
            amount,
            token.clone(),
            rules,
        )
    });

    // Now attestation should succeed
    let data = Map::new(&env);
    
    let result = env.as_contract(&attestation_id, || {
        AttestationEngineContract::attest(
            env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&env, "health_check"),
            data,
            true,
                ev_seq(&env.clone()),
        )
    });

    // Should succeed
    assert!(result.is_ok());

    // Verify attestation was stored
    let attestations = env.as_contract(&attestation_id, || {
        AttestationEngineContract::get_attestations(env.clone(), commitment_id)
    });

    assert_eq!(attestations.len(), 1);
}
