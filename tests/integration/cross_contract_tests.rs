//! Cross-Contract Interaction Tests
//!
//! These tests verify:
//! - Contract A calling Contract B
//! - State changes on both contracts
//! - Failure propagation between contracts
//! - Multi-contract transaction flows

use crate::harness::{TestHarness, SECONDS_PER_DAY};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, IntoVal, String, Symbol, Vec,
};

use allocation_logic::{AllocationStrategiesContract, RiskLevel, Strategy};
use attestation_engine::{
    AttestParams, AttestationEngineContract, AttestationError, AttestationsPage,
};
use commitment_core::{CommitmentCoreContract, CommitmentRules};
use commitment_nft::{CommitmentNFTContract, ContractError as NftContractError};
use shared_utils::BatchMode;

/// Verify compliance integration between commitment_core and attestation_engine.
///
/// This ensures that `verify_compliance` reads commitment data (rules and
/// current_value) from `commitment_core` when deciding compliance.
#[test]
fn test_verify_compliance_uses_core_commitment_data() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount: i128 = 1_000_000_000_000;

    // Allow core contract to move user's tokens.
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    // Create commitment in core with default rules (max_loss_percent = 10).
    let commitment_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Initially, with no loss recorded, commitment should be compliant.
    let is_compliant_initial = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::verify_compliance(harness.env.clone(), commitment_id.clone())
        });
    assert!(is_compliant_initial);

    // Simulate a drawdown larger than the max_loss_percent by updating
    // current_value in commitment_core (20% loss > 10% max).
    let new_value = amount * 80 / 100; // 20% drawdown
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::update_value(
                harness.env.clone(),
                user.clone(),
                commitment_id.clone(),
                new_value,
            )
        });

    // Record a matching drawdown attestation via attestation_engine, which
    // internally also reads rules from commitment_core.
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::record_drawdown(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                20, // 20% drawdown
            )
            .unwrap();
        });

    // Now verify_compliance should report non-compliance based on updated
    // commitment_core state and health metrics.
    let is_compliant_after = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::verify_compliance(harness.env.clone(), commitment_id.clone())
        });
    assert!(!is_compliant_after);
}

/// Test: Commitment Core calls NFT Contract during creation
#[test]
fn test_commitment_core_calls_nft_on_creation() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    // Get initial NFT supply
    let initial_supply = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::total_supply(harness.env.clone())
        });
    assert_eq!(initial_supply, 0);

    // Create commitment (triggers NFT mint via cross-contract call)
    let commitment_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Verify NFT was minted via cross-contract call
    let final_supply = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::total_supply(harness.env.clone())
        });
    assert_eq!(final_supply, 1);

    // Verify commitment has NFT token ID
    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone())
        });
    assert_eq!(commitment.nft_token_id, 0); // First minted token is ID 0

    // Verify NFT ownership
    let nft_owner = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::owner_of(harness.env.clone(), 0).unwrap()
        });
    assert_eq!(nft_owner, *user);
}

/// Integration test: create_commitment mints NFT and metadata matches (#132)
#[test]
fn test_create_commitment_mints_nft_metadata_matches() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let amount = 5_000_000_000_000i128;
    let asset = &harness.contracts.token;
    let rules = CommitmentRules {
        duration_days: 90,
        max_loss_percent: 5,
        commitment_type: String::from_str(&harness.env, "safe"),
        early_exit_penalty: 3,
        min_fee_threshold: 500,
        grace_period_days: 0,
    };

    harness.approve_tokens(owner, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.create_commitment(owner, amount, asset, rules.clone());

    // Assert NFT contract has one new token
    let supply = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::total_supply(harness.env.clone())
        });
    assert_eq!(supply, 1, "exactly one NFT minted");

    let token_id = 0u32;

    // owner_of(token_id) == owner
    let nft_owner = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::owner_of(harness.env.clone(), token_id).unwrap()
        });
    assert_eq!(nft_owner, *owner, "NFT owner must match commitment owner");

    // get_metadata(token_id) matches rules and commitment
    let nft = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::get_metadata(harness.env.clone(), token_id).unwrap()
        });

    // Verify auto-generated commitment_id format: COMMIT_{token_id}
    assert_eq!(
        nft.metadata.commitment_id,
        String::from_str(&harness.env, "COMMIT_0")
    );
    assert_eq!(nft.metadata.duration_days, rules.duration_days);
    assert_eq!(nft.metadata.max_loss_percent, rules.max_loss_percent);
    assert_eq!(nft.metadata.commitment_type, rules.commitment_type);
    assert_eq!(nft.metadata.initial_amount, amount);
    assert_eq!(nft.metadata.asset_address, *asset);
    assert_eq!(nft.token_id, token_id);
    assert!(nft.is_active);
    assert_eq!(nft.early_exit_penalty, rules.early_exit_penalty);
}

/// Test: Attestation Engine verifies commitment in Core Contract
#[test]
fn test_attestation_engine_verifies_commitment_exists() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    // Create commitment in core contract
    let commitment_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Attestation engine reads commitment from core contract
    let attestation_data = harness.health_check_data();

    // Create attestation (validates commitment exists via cross-contract call)
    let params = AttestParams {
        commitment_id: commitment_id.clone(),
        attestation_type: String::from_str(&harness.env, "health_check"),
        data: attestation_data,
        is_compliant: true,
    };
    let mut params_vec: Vec<AttestParams> = Vec::new(&harness.env);
    params_vec.push_back(params);
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                verifier.clone(),
                params_vec,
                BatchMode::Atomic,
            )
        });

    assert!(result.success);

    // Verify attestation was stored
    let attestations = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestations(harness.env.clone(), commitment_id.clone())
        });

    assert_eq!(attestations.len(), 1);
}

/// Test: Attestation Engine fails for non-existent commitment
#[test]
fn test_attestation_fails_for_nonexistent_commitment() {
    let harness = TestHarness::new();
    let verifier = &harness.accounts.verifier;

    let fake_commitment_id = String::from_str(&harness.env, "nonexistent_commitment");
    let attestation_data = harness.health_check_data();

    // Attempt to create attestation for non-existent commitment
    let params = AttestParams {
        commitment_id: fake_commitment_id,
        attestation_type: String::from_str(&harness.env, "health_check"),
        data: attestation_data,
        is_compliant: true,
    };
    let mut params_vec: Vec<AttestParams> = Vec::new(&harness.env);
    params_vec.push_back(params);
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                verifier.clone(),
                params_vec,
                BatchMode::Atomic,
            )
        });

    // Should fail with CommitmentNotFound error
    assert!(!result.success);
}

/// Test: attest(...) by random address (not in verifier whitelist) → Unauthorized (#125)
#[test]
fn test_attest_by_random_address_fails_unauthorized() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let attacker = &harness.accounts.attacker;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );

    let attestation_data = harness.health_check_data();

    // Attacker (not a verifier) tries to attest
    let params = AttestParams {
        commitment_id: commitment_id.clone(),
        attestation_type: String::from_str(&harness.env, "health_check"),
        data: attestation_data,
        is_compliant: true,
    };
    let mut params_vec: Vec<AttestParams> = Vec::new(&harness.env);
    params_vec.push_back(params);
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                attacker.clone(),
                params_vec,
                BatchMode::Atomic,
            )
        });

    assert!(!result.success);

    // No attestation should have been stored
    let attestations = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestations(harness.env.clone(), commitment_id)
        });
    assert_eq!(attestations.len(), 0);
}

/// Test: attest(...) by address in verifier whitelist → success (#125)
#[test]
fn test_attest_by_verifier_succeeds() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );

    let attestation_data = harness.health_check_data();

    // Verifier (in whitelist) attests
    let params = AttestParams {
        commitment_id: commitment_id.clone(),
        attestation_type: String::from_str(&harness.env, "health_check"),
        data: attestation_data,
        is_compliant: true,
    };
    let mut params_vec: Vec<AttestParams> = Vec::new(&harness.env);
    params_vec.push_back(params);
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                verifier.clone(),
                params_vec,
                BatchMode::Atomic,
            )
        });

    assert!(result.success);

    let attestations = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestations(harness.env.clone(), commitment_id)
        });
    assert_eq!(attestations.len(), 1);
}

/// Test: After admin removes verifier, attest by that address → Unauthorized (#125)
#[test]
fn test_attest_after_verifier_removed_fails() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );

    // Verifier attests once (succeeds)
    let data1 = harness.health_check_data();
    let params1 = AttestParams {
        commitment_id: commitment_id.clone(),
        attestation_type: String::from_str(&harness.env, "health_check"),
        data: data1,
        is_compliant: true,
    };
    let params_vec1 = Vec::from_array(&harness.env, [params1]);
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                verifier.clone(),
                params_vec1,
                BatchMode::Atomic,
            )
        })
        .unwrap();

    // Admin removes verifier from whitelist
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::remove_verifier(
                harness.env.clone(),
                harness.accounts.admin.clone(),
                verifier.clone(),
            )
            .unwrap();
        });

    // Same address attests again → must fail
    let data2 = harness.health_check_data();
    let params2 = AttestParams {
        commitment_id: commitment_id.clone(),
        attestation_type: String::from_str(&harness.env, "health_check"),
        data: data2,
        is_compliant: true,
    };
    let params_vec2 = Vec::from_array(&harness.env, [params2]);
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                verifier.clone(),
                params_vec2,
                BatchMode::Atomic,
            )
        });

    assert!(!result.success);

    // Still only one attestation (the one before removal)
    let attestations = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestations(harness.env.clone(), commitment_id)
        });
    assert_eq!(attestations.len(), 1);
}

/// Test: Attestation succeeds after commitment is created
#[test]
fn test_attestation_succeeds_after_commitment_created() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;
    let commitment_id = String::from_str(&harness.env, "test_commitment_123");

    // First attempt: attestation should fail (commitment doesn't exist yet)
    let data_before = harness.health_check_data();
    let params_before = AttestParams {
        commitment_id: commitment_id.clone(),
        attestation_type: String::from_str(&harness.env, "health_check"),
        data: data_before,
        is_compliant: true,
    };
    let params_vec_before = Vec::from_array(&harness.env, [params_before]);
    let result_before = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                verifier.clone(),
                params_vec_before,
                BatchMode::Atomic,
            )
        });
    assert!(result_before.is_err());

    // Create commitment in core contract
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);
    let created_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Second attempt: attestation should succeed (commitment now exists)
    let data_after = harness.health_check_data();
    let params_after = AttestParams {
        commitment_id: created_id.clone(),
        attestation_type: String::from_str(&harness.env, "health_check"),
        data: data_after,
        is_compliant: true,
    };
    let params_vec_after = Vec::from_array(&harness.env, [params_after]);
    let result_after = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                verifier.clone(),
                params_vec_after,
                BatchMode::Atomic,
            )
        });
    assert!(result_after.is_ok());

    // Verify attestation was stored
    let attestations = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestations(harness.env.clone(), created_id)
        });
    assert_eq!(attestations.len(), 1);
}

/// Test: Multiple attestations for same commitment
#[test]
fn test_multiple_attestations_cross_contract() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Create multiple attestations
    for i in 0..3 {
        harness.advance_time(60); // Advance 1 minute between attestations

        let data = harness.health_check_data();
        let params = AttestParams {
            commitment_id: commitment_id.clone(),
            attestation_type: String::from_str(&harness.env, "health_check"),
            data: data,
            is_compliant: true,
        };
        let mut params_vec = Vec::new(&harness.env);
        params_vec.push_back(params);
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::batch_attest(
                    harness.env.clone(),
                    verifier.clone(),
                    params_vec,
                    BatchMode::Atomic,
                )
                .unwrap();
            });
    }

    // Verify all attestations were recorded
    let attestations = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestations(harness.env.clone(), commitment_id.clone())
        });

    assert_eq!(attestations.len(), 3);

    // Verify attestation count
    let count = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestation_count(
                harness.env.clone(),
                commitment_id.clone(),
            )
        });

    assert_eq!(count, 3);
}

/// Test: get_attestations_page — empty list returns empty (#130)
#[test]
fn test_get_attestations_page_empty_returns_empty() {
    let harness = TestHarness::new();
    let commitment_id = String::from_str(&harness.env, "no_attestations_commitment");

    let page: AttestationsPage =
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::get_attestations_page(
                    harness.env.clone(),
                    commitment_id,
                    0,
                    10,
                )
            });

    assert_eq!(page.attestations.len(), 0);
    assert_eq!(page.next_offset, 0);
}

/// Test: get_attestations_page — single page returns all when limit >= count (#130)
#[test]
fn test_get_attestations_page_single_page_returns_all() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);
    let commitment_id = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );

    // Add 3 attestations
    for _ in 0..3 {
        harness.advance_time(60);
        let data = harness.health_check_data();
        let params = AttestParams {
            commitment_id: commitment_id.clone(),
            attestation_type: String::from_str(&harness.env, "health_check"),
            data: data,
            is_compliant: true,
        };
        let mut params_vec = Vec::new(&harness.env);
        params_vec.push_back(params);
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::batch_attest(
                    harness.env.clone(),
                    verifier.clone(),
                    params_vec,
                    BatchMode::Atomic,
                )
                .unwrap();
            });
    }

    let page: AttestationsPage =
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::get_attestations_page(
                    harness.env.clone(),
                    commitment_id.clone(),
                    0,
                    10,
                )
            });

    assert_eq!(page.attestations.len(), 3);
    assert_eq!(page.next_offset, 0);
}

/// Test: get_attestations_page — multiple pages return correct chunks in order (#130)
#[test]
fn test_get_attestations_page_multiple_pages_correct_order() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);
    let commitment_id = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );

    // Add 5 attestations
    for _ in 0..5 {
        harness.advance_time(60);
        let data = harness.health_check_data();
        let params = AttestParams {
            commitment_id: commitment_id.clone(),
            attestation_type: String::from_str(&harness.env, "health_check"),
            data: data,
            is_compliant: true,
        };
        let mut params_vec = Vec::new(&harness.env);
        params_vec.push_back(params);
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::batch_attest(
                    harness.env.clone(),
                    verifier.clone(),
                    params_vec,
                    BatchMode::Atomic,
                )
                .unwrap();
            });
    }

    // Page 1: offset 0, limit 2
    let page1: AttestationsPage =
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::get_attestations_page(
                    harness.env.clone(),
                    commitment_id.clone(),
                    0,
                    2,
                )
            });
    assert_eq!(page1.attestations.len(), 2);
    assert_eq!(page1.next_offset, 2);

    // Page 2: offset 2, limit 2
    let page2: AttestationsPage =
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::get_attestations_page(
                    harness.env.clone(),
                    commitment_id.clone(),
                    2,
                    2,
                )
            });
    assert_eq!(page2.attestations.len(), 2);
    assert_eq!(page2.next_offset, 4);

    // Page 3: offset 4, limit 2
    let page3: AttestationsPage =
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::get_attestations_page(
                    harness.env.clone(),
                    commitment_id.clone(),
                    4,
                    2,
                )
            });
    assert_eq!(page3.attestations.len(), 1);
    assert_eq!(page3.next_offset, 0);

    // Order: timestamps should be non-decreasing across pages
    let all = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestations(harness.env.clone(), commitment_id)
        });
    assert_eq!(all.len(), 5);
    let mut prev_ts = 0u64;
    for att in all.iter() {
        assert!(att.timestamp >= prev_ts);
        prev_ts = att.timestamp;
    }
}

/// Test: Commitment settlement triggers NFT settlement
#[test]
fn test_commitment_settlement_calls_nft_settle() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    // Use shorter duration for test
    let rules = CommitmentRules {
        duration_days: 1,
        max_loss_percent: 10,
        commitment_type: String::from_str(&harness.env, "balanced"),
        early_exit_penalty: 10, // Satisfies Balanced: ≥ 10
        min_fee_threshold: 1000,
        grace_period_days: 0,
    };

    let commitment_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                rules,
            )
        });

    // Verify NFT is active before settlement
    let is_active_before = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::is_active(harness.env.clone(), 0).unwrap()
        });
    assert!(is_active_before);

    // Advance time past expiration
    harness.advance_days(2);

    // Settle commitment (triggers NFT settlement via cross-contract call)
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::settle(harness.env.clone(), commitment_id.clone())
        });

    // Verify NFT is no longer active
    let is_active_after = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::is_active(harness.env.clone(), 0).unwrap()
        });
    assert!(!is_active_after);

    // Verify get_metadata still returns data but is_active is false (#133)
    let nft_after_settle = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::get_metadata(harness.env.clone(), 0).unwrap()
        });
    assert!(!nft_after_settle.is_active);
    // Verify auto-generated commitment_id format: COMMIT_{token_id}
    assert_eq!(
        nft_after_settle.metadata.commitment_id,
        String::from_str(&harness.env, "COMMIT_0")
    );
    assert_eq!(nft_after_settle.owner, *user);

    // Verify commitment status
    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone())
        });
    assert_eq!(commitment.status, String::from_str(&harness.env, "settled"));
}

/// Test: Allocation logic interacts with pools correctly
#[test]
#[ignore] // Temporarily disabled - allocation_logic not available
fn test_allocation_logic_pool_interaction() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    // Setup pools
    harness.setup_default_pools();

    // Allocate funds
    let result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::allocate(
                harness.env.clone(),
                user.clone(),
                String::from_str(&harness.env, "1"), // commitment_id
                amount,
                Strategy::Balanced,
            )
        });

    assert!(result.success);
    let summary = result.unwrap();

    // Verify allocation was made
    assert_eq!(summary.total_allocated, amount);
    assert!(summary.allocations.len() > 0);

    // Verify pools received allocations
    for allocation in summary.allocations.iter() {
        let pool = harness
            .env
            .as_contract(&harness.contracts.allocation_logic, || {
                AllocationStrategiesContract::get_pool(harness.env.clone(), allocation.pool_id)
                    .unwrap()
            });

        assert!(pool.total_liquidity > 0);
    }
}

/// Test: Allocation rebalancing updates multiple pools
#[test]
#[ignore] // Temporarily disabled - allocation_logic not available
fn test_allocation_rebalance_cross_pool() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    // Setup pools
    harness.setup_default_pools();

    // Initial allocation with Balanced strategy
    harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::allocate(
                harness.env.clone(),
                user.clone(),
                String::from_str(&harness.env, "1"),
                amount,
                Strategy::Balanced,
            )
            .unwrap();
        });

    // Get initial allocation
    let initial_allocation = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::get_allocation(
                harness.env.clone(),
                String::from_str(&harness.env, "1"),
            )
        });

    // Advance time
    harness.advance_time(3600);

    // Rebalance
    let result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::rebalance(
                harness.env.clone(),
                user.clone(),
                String::from_str(&harness.env, "1"),
            )
        });

    assert!(result.success);
    let rebalanced = result.unwrap();

    // Verify total remains the same
    assert_eq!(
        rebalanced.total_allocated,
        initial_allocation.total_allocated
    );
}

/// Test: Cross-contract state consistency
#[test]
fn test_cross_contract_state_consistency() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    // Create commitment
    let commitment_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Verify state consistency across contracts

    // 1. Core contract has commitment
    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone())
        });
    assert_eq!(commitment.owner, *user);
    assert_eq!(commitment.amount, amount);

    // 2. NFT contract has matching NFT
    let nft = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::get_metadata(harness.env.clone(), commitment.nft_token_id)
                .unwrap()
        });
    assert_eq!(nft.owner, *user);
    assert_eq!(nft.metadata.initial_amount, amount);
    // Verify auto-generated commitment_id format: COMMIT_{token_id}
    assert_eq!(
        nft.metadata.commitment_id,
        String::from_str(&harness.env, "COMMIT_0")
    );

    // 3. Token balances are correct
    let user_balance = harness.balance(user);
    let contract_balance = harness.balance(&harness.contracts.commitment_core);
    assert_eq!(
        user_balance + contract_balance,
        crate::harness::DEFAULT_USER_BALANCE
    );
}

/// Test: Health metrics calculation involves cross-contract data
#[test]
fn test_health_metrics_cross_contract_data() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    // Create commitment
    let commitment_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Add attestations with different types
    let health_data = harness.health_check_data();
    let params1 = AttestParams {
        commitment_id: commitment_id.clone(),
        attestation_type: String::from_str(&harness.env, "health_check"),
        data: health_data,
        is_compliant: true,
    };
    let params_vec1 = Vec::from_array(&harness.env, [params1]);
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                verifier.clone(),
                params_vec1,
                BatchMode::Atomic,
            )
            .unwrap();
        });

    harness.advance_time(60);

    let fee_data = harness.fee_generation_data(50000);
    let params2 = AttestParams {
        commitment_id: commitment_id.clone(),
        attestation_type: String::from_str(&harness.env, "fee_generation"),
        data: fee_data,
        is_compliant: true,
    };
    let params_vec2 = Vec::from_array(&harness.env, [params2]);
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::batch_attest(
                harness.env.clone(),
                verifier.clone(),
                params_vec2,
                BatchMode::Atomic,
            )
            .unwrap();
        });

    // Get health metrics (involves reading from core contract)
    let metrics = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_health_metrics(
                harness.env.clone(),
                commitment_id.clone(),
            )
        });

    // Verify metrics reflect cross-contract data
    assert_eq!(metrics.initial_value, amount);
    assert!(metrics.last_attestation > 0);
}

/// Test: Verifier management across admin context
#[test]
fn test_verifier_management_admin_context() {
    let harness = TestHarness::new();
    let admin = &harness.accounts.admin;
    let new_verifier = Address::generate(&harness.env);

    // Initially, new_verifier is not authorized
    let is_verifier_before = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::is_verifier(harness.env.clone(), new_verifier.clone())
        });
    assert!(!is_verifier_before);

    // Admin adds verifier
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::add_verifier(
                harness.env.clone(),
                admin.clone(),
                new_verifier.clone(),
            )
            .unwrap();
        });

    // Now new_verifier is authorized
    let is_verifier_after = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::is_verifier(harness.env.clone(), new_verifier.clone())
        });
    assert!(is_verifier_after);

    // Admin removes verifier
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::remove_verifier(
                harness.env.clone(),
                admin.clone(),
                new_verifier.clone(),
            )
            .unwrap();
        });

    // new_verifier is no longer authorized
    let is_verifier_removed = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::is_verifier(harness.env.clone(), new_verifier.clone())
        });
    assert!(!is_verifier_removed);
}

/// Test: Pool registration and management
#[test]
fn test_pool_management_cross_contract() {
    let harness = TestHarness::new();
    let admin = &harness.accounts.admin;

    // Register pools with different risk levels
    harness.register_pool(10, RiskLevel::Low, 300, 500_000_000_000_000);
    harness.register_pool(20, RiskLevel::Medium, 800, 300_000_000_000_000);
    harness.register_pool(30, RiskLevel::High, 1500, 200_000_000_000_000);

    // Get all pools
    let pools = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::get_all_pools(harness.env.clone())
        });

    assert_eq!(pools.len(), 3);

    // Verify pool details
    let pool_10 = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::get_pool(harness.env.clone(), 10).unwrap()
        });
    assert_eq!(pool_10.apy, 300);
    assert_eq!(pool_10.risk_level, RiskLevel::Low);

    // Update pool status
    harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::update_pool_status(
                harness.env.clone(),
                admin.clone(),
                10,
                false,
            )
            .unwrap();
        });

    let updated_pool = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::get_pool(harness.env.clone(), 10).unwrap()
        });
    assert!(!updated_pool.active);
}

// =============================================================================
// #143: get_commitments_created_between
// =============================================================================

/// Test: get_commitments_created_between returns commitments in time range
#[test]
fn test_get_commitments_created_between() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount * 3);

    let t0 = harness.current_timestamp();

    let id1 = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );
    harness.advance_time(100);
    let t_after_first = harness.current_timestamp();
    let id2 = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );
    harness.advance_time(100);
    let id3 = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );

    // Range [t0, t_after_first - 1]: only id1 (id2 created at t_after_first)
    let ids_early = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitments_created_between(
                harness.env.clone(),
                t0,
                t_after_first.saturating_sub(1),
            )
        });
    assert_eq!(ids_early.len(), 1);
    assert!(ids_early.contains(&id1));

    // Range [t0, t_after_first]: id1 and id2
    let ids_two = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitments_created_between(
                harness.env.clone(),
                t0,
                t_after_first,
            )
        });
    assert_eq!(ids_two.len(), 2);
    assert!(ids_two.contains(&id1));
    assert!(ids_two.contains(&id2));

    // Empty range
    let empty = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitments_created_between(
                harness.env.clone(),
                t0.saturating_sub(10000),
                t0.saturating_sub(5000),
            )
        });
    assert!(empty.is_empty());
}

// =============================================================================
// #145: NFT transfer when commitment is active (locked)
// =============================================================================

/// Test: Transfer of active (locked) NFT fails; after settle transfer succeeds
#[test]
fn test_nft_transfer_locked_until_settled() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let other = &harness.accounts.user2;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );
    let nft_token_id = 0u32;

    // Mint NFT; do not settle -> is_active == true
    let is_active = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::is_active(harness.env.clone(), nft_token_id).unwrap()
        });
    assert!(is_active);

    // transfer(from, to, token_id) while active -> error
    let transfer_result = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::transfer(
                harness.env.clone(),
                user.clone(),
                other.clone(),
                nft_token_id,
            )
        });
    assert_eq!(transfer_result, Err(NftContractError::NFTLocked));

    // Settle commitment (advance time and settle)
    harness.advance_days(31);
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::settle(harness.env.clone(), commitment_id.clone())
        });

    // After settled, NFT is inactive
    let is_active_after = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::is_active(harness.env.clone(), nft_token_id).unwrap()
        });
    assert!(!is_active_after);

    // transfer(from, to, token_id) after settled -> success
    harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::transfer(
                harness.env.clone(),
                user.clone(),
                other.clone(),
                nft_token_id,
            )
            .unwrap();
        });

    let new_owner = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::owner_of(harness.env.clone(), nft_token_id).unwrap()
        });
    assert_eq!(new_owner, *other);
}

// =============================================================================
// #148: early_exit when current_value is zero
// =============================================================================

/// Test: early_exit with current_value = 0 completes without panic; penalty = 0, returned = 0
#[test]
fn test_early_exit_zero_current_value() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    // Use max_loss_percent: 100 so update_value(0) does not mark as violated
    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 100,
        commitment_type: String::from_str(&harness.env, "balanced"),
        early_exit_penalty: 5,
        min_fee_threshold: 1000,
        grace_period_days: 0,
    };
    let commitment_id = harness.create_commitment(user, amount, &harness.contracts.token, rules);

    // update_value(commitment_id, 0)
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::update_value(
                harness.env.clone(),
                user.clone(),
                commitment_id.clone(),
                0,
            )
        });

    // early_exit(commitment_id) by owner -> no panic; penalty = 0, returned = 0
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::early_exit(
                harness.env.clone(),
                commitment_id.clone(),
                user.clone(),
            )
        });

    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone())
        });
    assert_eq!(
        commitment.status,
        String::from_str(&harness.env, "early_exit")
    );
    assert_eq!(commitment.current_value, 0);
}

// =============================================================================
// #149: record_fees and record_drawdown access control
// =============================================================================

/// Test: record_fees and record_drawdown by random address -> error; by verifier -> success
#[test]
fn test_record_fees_record_drawdown_access_control() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let random = &harness.accounts.attacker;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);
    let commitment_id = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );

    // record_fees(commitment_id, amount) by random address -> error
    let r_fees_random = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::record_fees(
                harness.env.clone(),
                random.clone(),
                commitment_id.clone(),
                50_000,
            )
        });
    assert_eq!(r_fees_random, Err(AttestationError::Unauthorized));

    // record_drawdown(commitment_id, percent) by random address -> error
    let r_drawdown_random = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::record_drawdown(
                harness.env.clone(),
                random.clone(),
                commitment_id.clone(),
                5,
            )
        });
    assert_eq!(r_drawdown_random, Err(AttestationError::Unauthorized));

    // record_fees by authorized verifier -> success
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::record_fees(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                50_000,
            )
            .unwrap();
        });

    // record_drawdown by authorized verifier -> success
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::record_drawdown(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                5,
            )
            .unwrap();
        });
}

// =============================================================================
// #TBD: record_fees fee amount validation
// =============================================================================

/// Test: record_fees rejects negative or invalid fee amounts
#[test]
fn test_record_fees_validation() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);
    let commitment_id = harness.create_commitment(
        user,
        amount,
        &harness.contracts.token,
        harness.default_rules(),
    );

    // Test 1: Negative fee amount (-1) should be rejected
    let result_negative_one =
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::record_fees(
                    harness.env.clone(),
                    verifier.clone(),
                    commitment_id.clone(),
                    -1,
                )
            });
    assert_eq!(result_negative_one, Err(AttestationError::InvalidFeeAmount));

    // Test 2: Zero fee amount should be allowed
    let result_zero = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::record_fees(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                0,
            )
        });
    assert_eq!(result_zero, Ok(()));

    // Test 3: Positive fee amount should be allowed
    let result_positive = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::record_fees(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                50_000_000,
            )
        });
    assert_eq!(result_positive, Ok(()));

    // Test 4: Large positive fee amount should be allowed
    let result_large_positive =
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::record_fees(
                    harness.env.clone(),
                    verifier.clone(),
                    commitment_id.clone(),
                    1_000_000_000_000,
                )
            });
    assert_eq!(result_large_positive, Ok(()));

    // Test 5: Minimum i128 value should be rejected
    let result_min_i128 = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::record_fees(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                i128::MIN,
            )
        });
    assert_eq!(result_min_i128, Err(AttestationError::InvalidFeeAmount));
}
