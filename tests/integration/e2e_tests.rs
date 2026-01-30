//! End-to-End Flow Tests
//!
//! These tests verify complete user journeys involving:
//! - Multiple contracts
//! - Token interactions
//! - Oracle reads
//! - Final settlement/state verification

use crate::harness::{TestHarness, DEFAULT_USER_BALANCE, SECONDS_PER_DAY};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, String,
};

use commitment_core::{CommitmentCoreContract, CommitmentRules};
use commitment_nft::CommitmentNFTContract;
use attestation_engine::AttestationEngineContract;
use allocation_logic::{AllocationStrategiesContract, RiskLevel, Strategy};
use mock_oracle::MockOracleContract;

/// Test: Complete commitment lifecycle (create -> monitor -> settle)
#[test]
fn test_e2e_complete_commitment_lifecycle() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // ========== PHASE 1: SETUP ==========
    let initial_balance = harness.balance(user);

    // Set oracle price for the token
    harness.set_oracle_price(&harness.contracts.token, 100_000_000, 8);

    // Setup allocation pools
    harness.setup_default_pools();

    // ========== PHASE 2: COMMITMENT CREATION ==========
    // Approve tokens
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    // Create commitment with 7-day duration
    let rules = CommitmentRules {
        duration_days: 7,
        max_loss_percent: 10,
        commitment_type: String::from_str(&harness.env, "balanced"),
        early_exit_penalty: 5,
        min_fee_threshold: 1000,
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

    // Verify commitment created
    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone())
        });
    assert_eq!(commitment.owner, *user);
    assert_eq!(commitment.status, String::from_str(&harness.env, "active"));

    // Verify NFT minted
    let nft_balance = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::balance_of(harness.env.clone(), user.clone())
        });
    assert_eq!(nft_balance, 1);

    // Verify tokens locked
    assert_eq!(harness.balance(user), initial_balance - amount);

    // ========== PHASE 3: MONITORING PERIOD ==========
    // Simulate periodic health checks over the commitment period
    for day in 1..=6 {
        harness.advance_days(1);

        // Verifier submits health check attestation
        let health_data = harness.health_check_data();
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::attest(
                    harness.env.clone(),
                    verifier.clone(),
                    commitment_id.clone(),
                    String::from_str(&harness.env, "health_check"),
                    health_data,
                    true,
                )
                .unwrap();
            });
    }

    // Verify attestations recorded
    let attestation_count = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestation_count(
                harness.env.clone(),
                commitment_id.clone(),
            )
        });
    assert_eq!(attestation_count, 6);

    // ========== PHASE 4: SETTLEMENT ==========
    // Advance to expiration
    harness.advance_days(2); // Total: 8 days (past 7-day duration)

    // Verify commitment is expired
    let is_expired = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::is_expired(harness.env.clone(), commitment.nft_token_id).unwrap()
        });
    assert!(is_expired);

    // Settle commitment
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::settle(harness.env.clone(), commitment_id.clone())
        });

    // ========== PHASE 5: VERIFICATION ==========
    // Verify commitment status changed
    let settled_commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone())
        });
    assert_eq!(
        settled_commitment.status,
        String::from_str(&harness.env, "settled")
    );

    // Verify NFT is inactive
    let nft_active = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::is_active(harness.env.clone(), commitment.nft_token_id).unwrap()
        });
    assert!(!nft_active);

    // Verify tokens returned to user
    assert_eq!(harness.balance(user), initial_balance);

    // Verify TVL decreased
    let tvl = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_total_value_locked(harness.env.clone())
        });
    assert_eq!(tvl, 0);
}

/// Test: Early exit flow with penalty
#[test]
fn test_e2e_early_exit_with_penalty() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;
    let early_exit_penalty = 10u32; // 10%

    let initial_balance = harness.balance(user);

    // Create commitment
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 15,
        commitment_type: String::from_str(&harness.env, "aggressive"),
        early_exit_penalty,
        min_fee_threshold: 500,
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

    // Advance some time but not to expiration
    harness.advance_days(10);

    // Record balance before early exit
    let balance_before_exit = harness.balance(user);

    // Execute early exit
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::early_exit(harness.env.clone(), commitment_id.clone(), user.clone())
        });

    // Verify status
    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone())
        });
    assert_eq!(
        commitment.status,
        String::from_str(&harness.env, "early_exit")
    );

    // Verify penalty was applied
    let expected_penalty = amount * early_exit_penalty as i128 / 100;
    let expected_return = amount - expected_penalty;
    let balance_after_exit = harness.balance(user);

    assert_eq!(balance_after_exit - balance_before_exit, expected_return);
    assert_eq!(
        balance_after_exit,
        initial_balance - expected_penalty
    );
}

/// Test: Multiple users creating commitments simultaneously
#[test]
fn test_e2e_multiple_users_concurrent_commitments() {
    let harness = TestHarness::new();
    let user1 = &harness.accounts.user1;
    let user2 = &harness.accounts.user2;
    let amount1 = 500_000_000_000i128;
    let amount2 = 750_000_000_000i128;

    // Both users create commitments
    harness.approve_tokens(user1, &harness.contracts.commitment_core, amount1);
    harness.approve_tokens(user2, &harness.contracts.commitment_core, amount2);

    let commitment_id1 = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user1.clone(),
                amount1,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    let commitment_id2 = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user2.clone(),
                amount2,
                harness.contracts.token.clone(),
                harness.safe_rules(),
            )
        });

    // Verify both commitments exist
    let total_commitments = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_total_commitments(harness.env.clone())
        });
    assert_eq!(total_commitments, 2);

    // Verify TVL is sum of both
    let tvl = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_total_value_locked(harness.env.clone())
        });
    assert_eq!(tvl, amount1 + amount2);

    // Verify each user has their own NFT
    let user1_nfts = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::balance_of(harness.env.clone(), user1.clone())
        });
    let user2_nfts = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::balance_of(harness.env.clone(), user2.clone())
        });
    assert_eq!(user1_nfts, 1);
    assert_eq!(user2_nfts, 1);

    // Each user can retrieve their own commitments
    let user1_commits = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_owner_commitments(harness.env.clone(), user1.clone())
        });
    let user2_commits = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_owner_commitments(harness.env.clone(), user2.clone())
        });
    assert_eq!(user1_commits.len(), 1);
    assert_eq!(user2_commits.len(), 1);
}

/// Test: Commitment with allocation to pools
#[test]
fn test_e2e_commitment_with_allocation() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    // Setup pools
    harness.setup_default_pools();

    // Create commitment
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

    // Allocate funds using balanced strategy
    let allocation_result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::allocate(
                harness.env.clone(),
                user.clone(),
                1u64,
                amount,
                Strategy::Balanced,
            )
        });

    assert!(allocation_result.is_ok());
    let summary = allocation_result.unwrap();
    assert_eq!(summary.total_allocated, amount);

    // Verify allocation details
    let allocation = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::get_allocation(harness.env.clone(), 1u64)
        });
    assert_eq!(allocation.strategy, Strategy::Balanced);
    assert!(allocation.allocations.len() > 0);
}

/// Test: Violation detection and handling flow
#[test]
fn test_e2e_violation_detection_flow() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Create commitment with low loss tolerance
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 5, // Very low tolerance
        commitment_type: String::from_str(&harness.env, "safe"),
        early_exit_penalty: 3,
        min_fee_threshold: 100,
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

    // Advance time
    harness.advance_days(5);

    // Submit violation attestation
    let violation_data = harness.violation_data("loss_exceeded", "high");
    harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::attest(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                String::from_str(&harness.env, "violation"),
                violation_data,
                false, // Not compliant
            )
            .unwrap();
        });

    // Verify attestation recorded
    let attestations = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestations(harness.env.clone(), commitment_id.clone())
        });
    assert_eq!(attestations.len(), 1);
    assert!(!attestations.get(0).unwrap().is_compliant);

    // Get health metrics showing violation
    let metrics = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
        });

    // Compliance score should have decreased
    assert!(metrics.compliance_score < 100);
}

/// Test: NFT secondary market transfer with commitment
#[test]
fn test_e2e_nft_transfer_between_users() {
    let harness = TestHarness::new();
    let seller = &harness.accounts.user1;
    let buyer = &harness.accounts.user2;
    let amount = 1_000_000_000_000i128;

    // Seller creates commitment
    harness.approve_tokens(seller, &harness.contracts.commitment_core, amount);

    let commitment_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                seller.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Get token ID
    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone())
        });
    let token_id = commitment.nft_token_id;

    // Transfer NFT to buyer
    harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::transfer(
                harness.env.clone(),
                seller.clone(),
                buyer.clone(),
                token_id,
            )
            .unwrap();
        });

    // Verify NFT ownership changed
    let new_owner = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::owner_of(harness.env.clone(), token_id).unwrap()
        });
    assert_eq!(new_owner, *buyer);

    // Verify seller has no NFTs
    let seller_balance = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::balance_of(harness.env.clone(), seller.clone())
        });
    assert_eq!(seller_balance, 0);

    // Verify buyer has NFT
    let buyer_balance = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::balance_of(harness.env.clone(), buyer.clone())
        });
    assert_eq!(buyer_balance, 1);
}

/// Test: Fee generation and tracking
#[test]
fn test_e2e_fee_generation_tracking() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;

    // Create commitment
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

    // Simulate fee generation over time
    let fee_amounts = vec![10000i128, 15000, 20000, 25000];
    for fee in &fee_amounts {
        harness.advance_days(1);

        let fee_data = harness.fee_generation_data(*fee);
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::attest(
                    harness.env.clone(),
                    verifier.clone(),
                    commitment_id.clone(),
                    String::from_str(&harness.env, "fee_generation"),
                    fee_data,
                    true,
                )
                .unwrap();
            });
    }

    // Verify attestations recorded
    let count = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestation_count(
                harness.env.clone(),
                commitment_id.clone(),
            )
        });
    assert_eq!(count, 4);
}

/// Test: Oracle price impact on monitoring
#[test]
fn test_e2e_oracle_price_monitoring() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    // Set initial oracle price
    harness.set_oracle_price(&harness.contracts.token, 100_000_000, 8); // $1.00

    // Create commitment
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

    // Simulate price changes
    let prices = vec![
        100_000_000i128, // $1.00
        105_000_000,     // $1.05
        98_000_000,      // $0.98
        102_000_000,     // $1.02
    ];

    for price in prices {
        harness.advance_time(3600); // 1 hour
        harness.set_oracle_price(&harness.contracts.token, price, 8);

        // Read price
        let read_price = harness
            .env
            .as_contract(&harness.contracts.mock_oracle, || {
                MockOracleContract::get_price(harness.env.clone(), harness.contracts.token.clone())
                    .unwrap()
            });
        assert_eq!(read_price, price);
    }
}

/// Test: Complete rebalancing flow
#[test]
fn test_e2e_allocation_rebalancing_flow() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    // Setup pools
    harness.setup_default_pools();

    // Initial allocation
    let initial_result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::allocate(
                harness.env.clone(),
                user.clone(),
                1u64,
                amount,
                Strategy::Balanced,
            )
        });
    assert!(initial_result.is_ok());

    let initial_summary = initial_result.unwrap();
    assert_eq!(initial_summary.total_allocated, amount);

    // Advance time
    harness.advance_days(7);

    // Rebalance
    let rebalance_result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::rebalance(harness.env.clone(), user.clone(), 1u64)
        });
    assert!(rebalance_result.is_ok());

    let rebalanced = rebalance_result.unwrap();
    assert_eq!(rebalanced.total_allocated, amount);

    // Verify allocation is still intact
    let final_allocation = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::get_allocation(harness.env.clone(), 1u64)
        });
    assert_eq!(final_allocation.total_allocated, amount);
}
