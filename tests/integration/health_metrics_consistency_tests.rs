// Health Metrics Consistency Tests for Issue #150
// Tests for get_health_metrics consistency after multiple fee and drawdown records

#![cfg(test)]

use crate::harness::{ TestHarness, SECONDS_PER_DAY };
use soroban_sdk::{ testutils::{ Address as _, Ledger }, Address, Env, Map, String };

use attestation_engine::AttestationEngineContract;
use commitment_core::{ CommitmentCoreContract, CommitmentRules };
use commitment_nft::CommitmentNFTContract;

// ============================================
// Fee Aggregation Tests
// ============================================

#[test]
fn test_multiple_record_fees_cumulative_sum() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Record multiple fees
    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            10_0000000
        )
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            20_0000000
        )
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            5_0000000
        )
    });

    // Verify cumulative sum: 10 + 20 + 5 = 35
    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });
    assert_eq!(metrics.fees_generated, 35_0000000);
}

#[test]
fn test_record_fees_zero_amount() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Record zero fee
    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            0
        )
    });

    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });
    assert_eq!(metrics.fees_generated, 0);
}

#[test]
fn test_record_fees_large_amounts() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Record large fees to test overflow protection
    let large_fee1 = i128::MAX / 4;
    let large_fee2 = i128::MAX / 4;

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            large_fee1
        )
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            large_fee2
        )
    });

    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });

    // Should handle large numbers without overflow
    assert!(metrics.fees_generated > 0);
}

// ============================================
// Drawdown Aggregation Tests
// ============================================

#[test]
fn test_multiple_record_drawdown_latest_value() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Record multiple drawdowns
    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            5
        ).unwrap()
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            10
        ).unwrap()
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            3
        ).unwrap()
    });

    // Verify latest drawdown value is stored (not cumulative)
    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });
    assert_eq!(metrics.drawdown_percent, 3);
}

#[test]
fn test_record_drawdown_compliance_check() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Record compliant drawdown (within 10% threshold)
    let result = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            harness.accounts.admin.clone(), // Use admin instead of verifier
            commitment_id.clone(),
            5
        )
    });

    // Check if record_drawdown succeeded
    match result {
        Ok(_) => println!("record_drawdown succeeded"),
        Err(e) => println!("record_drawdown failed: {:?}", e),
    }

    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });

    // Debug: print actual values
    println!("Actual drawdown_percent: {}", metrics.drawdown_percent);
    println!("Actual fees_generated: {}", metrics.fees_generated);
    println!("Actual compliance_score: {}", metrics.compliance_score);

    assert_eq!(metrics.drawdown_percent, 5);

    // Verify compliance is still true
    let is_compliant = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::verify_compliance(harness.env.clone(), commitment_id.clone())
    });
    assert!(is_compliant);
}

#[test]
fn test_record_drawdown_non_compliant() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Record non-compliant drawdown (exceeds 10% threshold)
    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            15
        ).unwrap()
    });

    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });
    assert_eq!(metrics.drawdown_percent, 15);

    // Verify compliance is false
    let is_compliant = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::verify_compliance(harness.env.clone(), commitment_id.clone())
    });
    assert!(!is_compliant);
}

// ============================================
// Compliance Score Update Tests
// ============================================

#[test]
fn test_compliance_score_updates_after_fees() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Initial compliance score should be 100
    let initial_metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });
    assert_eq!(initial_metrics.compliance_score, 100);

    // Record fees (compliant action)
    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            10_0000000
        )
    });

    let metrics_after_fees = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });

    // Compliance score should increase or stay the same for compliant fee generation
    assert!(metrics_after_fees.compliance_score >= 100);
    assert!(metrics_after_fees.compliance_score <= 100); // Capped at 100
}

#[test]
fn test_compliance_score_updates_after_drawdown() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Record compliant drawdown
    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            5
        ).unwrap()
    });

    let metrics_after_compliant = harness.env.as_contract(
        &harness.contracts.attestation_engine,
        || {
            AttestationEngineContract::get_health_metrics(
                harness.env.clone(),
                commitment_id.clone()
            )
        }
    );

    // Should maintain high compliance score for compliant drawdown
    assert!(metrics_after_compliant.compliance_score >= 90);

    // Record non-compliant drawdown
    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            15
        ).unwrap()
    });

    let metrics_after_non_compliant = harness.env.as_contract(
        &harness.contracts.attestation_engine,
        || {
            AttestationEngineContract::get_health_metrics(
                harness.env.clone(),
                commitment_id.clone()
            )
        }
    );

    // Compliance score should decrease for non-compliant drawdown
    assert!(
        metrics_after_non_compliant.compliance_score < metrics_after_compliant.compliance_score
    );
}

#[test]
fn test_compliance_score_with_violation_attestation() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Record a violation attestation
    let mut data = Map::new(&harness.env);
    data.set(
        String::from_str(&harness.env, "violation_type"),
        String::from_str(&harness.env, "protocol_breach")
    );
    data.set(String::from_str(&harness.env, "severity"), String::from_str(&harness.env, "high"));

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::attest(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            String::from_str(&harness.env, "violation"),
            data.clone(),
            false // Non-compliant
        )
    });

    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });

    // Compliance score should decrease significantly for high severity violation
    assert!(metrics.compliance_score <= 70); // 100 - 30 (high severity penalty)
}

// ============================================
// Mixed Operations Tests
// ============================================

#[test]
fn test_mixed_fees_and_drawdown_operations() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Mix of operations
    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            10_0000000
        )
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            5
        ).unwrap()
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            20_0000000
        )
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            8
        ).unwrap()
    });

    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });

    // Verify cumulative fees
    assert_eq!(metrics.fees_generated, 30_0000000);

    // Verify latest drawdown
    assert_eq!(metrics.drawdown_percent, 8);

    // Verify compliance score is reasonable
    assert!(metrics.compliance_score >= 50);
    assert!(metrics.compliance_score <= 100);
}

#[test]
fn test_health_metrics_persistence() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Record some operations
    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            15_0000000
        )
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            7
        )
    });

    // Get metrics first time
    let metrics1 = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });

    // Get metrics again (should be consistent)
    let metrics2 = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });

    assert_eq!(metrics1.fees_generated, metrics2.fees_generated);
    assert_eq!(metrics1.drawdown_percent, metrics2.drawdown_percent);
    assert_eq!(metrics1.compliance_score, metrics2.compliance_score);
    assert_eq!(metrics1.commitment_id, metrics2.commitment_id);
}

// ============================================
// Edge Cases Tests
// ============================================

#[test]
fn test_empty_attestations_health_metrics() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    // Approve tokens and create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    // Get health metrics without any attestations
    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });

    assert_eq!(metrics.fees_generated, 0);
    assert_eq!(metrics.compliance_score, 100); // Default compliance score
    assert_eq!(metrics.commitment_id, commitment_id);
}

#[test]
fn test_single_attestation_types() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Test single fee record
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let commitment_id = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_fees(
            harness.env.clone(),
            verifier.clone(),
            commitment_id.clone(),
            25_0000000
        )
    });

    let metrics = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
    });
    assert_eq!(metrics.fees_generated, 25_0000000);

    // Reset with new commitment for drawdown test
    let user2 = &harness.accounts.user2;
    harness.approve_tokens(user2, &harness.contracts.commitment_core, amount);

    let commitment_id2 = harness.env.as_contract(&harness.contracts.commitment_core, || {
        CommitmentCoreContract::create_commitment(
            harness.env.clone(),
            user2.clone(),
            amount,
            harness.contracts.token.clone(),
            harness.default_rules()
        )
    });

    harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::record_drawdown(
            harness.env.clone(),
            verifier.clone(),
            commitment_id2.clone(),
            12
        ).unwrap()
    });

    let metrics2 = harness.env.as_contract(&harness.contracts.attestation_engine, || {
        AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id2.clone())
    });
    assert_eq!(metrics2.drawdown_percent, 12);
}
