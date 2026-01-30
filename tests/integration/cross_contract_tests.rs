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
    Address, Env, String, Symbol, IntoVal, Vec,
};

use commitment_core::{CommitmentCoreContract, CommitmentRules};
use commitment_nft::CommitmentNFTContract;
use attestation_engine::AttestationEngineContract;
use allocation_logic::{AllocationStrategiesContract, RiskLevel, Strategy};

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
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::attest(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                String::from_str(&harness.env, "health_check"),
                attestation_data,
                true,
            )
        });

    assert!(result.is_ok());

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
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::attest(
                harness.env.clone(),
                verifier.clone(),
                fake_commitment_id,
                String::from_str(&harness.env, "health_check"),
                attestation_data,
                true,
            )
        });

    // Should fail because commitment doesn't exist
    assert!(result.is_err());
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
        harness
            .env
            .as_contract(&harness.contracts.attestation_engine, || {
                AttestationEngineContract::attest(
                    harness.env.clone(),
                    verifier.clone(),
                    commitment_id.clone(),
                    String::from_str(&harness.env, "health_check"),
                    data,
                    true,
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
                1u64, // commitment_id
                amount,
                Strategy::Balanced,
            )
        });

    assert!(result.is_ok());
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
                1u64,
                amount,
                Strategy::Balanced,
            )
            .unwrap();
        });

    // Get initial allocation
    let initial_allocation = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::get_allocation(harness.env.clone(), 1u64)
        });

    // Advance time
    harness.advance_time(3600);

    // Rebalance
    let result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::rebalance(harness.env.clone(), user.clone(), 1u64)
        });

    assert!(result.is_ok());
    let rebalanced = result.unwrap();

    // Verify total remains the same
    assert_eq!(rebalanced.total_allocated, initial_allocation.total_allocated);
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
    assert_eq!(nft.metadata.commitment_id, commitment_id);

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

    harness.advance_time(60);

    let fee_data = harness.fee_generation_data(50000);
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

    // Get health metrics (involves reading from core contract)
    let metrics = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_health_metrics(harness.env.clone(), commitment_id.clone())
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
