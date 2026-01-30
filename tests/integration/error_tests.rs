//! Error and Edge Case Tests
//!
//! These tests verify:
//! - Unauthorized access attempts
//! - Invalid input handling
//! - Replay-like behavior
//! - Boundary values (0, max, empty)
//! - Expected error assertions

use crate::harness::{TestHarness, DEFAULT_USER_BALANCE, SECONDS_PER_DAY};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

use commitment_core::{CommitmentCoreContract, CommitmentError, CommitmentRules};
use commitment_nft::{CommitmentNFTContract, ContractError as NftError};
use attestation_engine::{AttestationEngineContract, AttestationError};
use allocation_logic::{AllocationStrategiesContract, Error as AllocationError, RiskLevel, Strategy};
use mock_oracle::{MockOracleContract, OracleError};

// ============================================================================
// Unauthorized Access Tests
// ============================================================================

/// Test: Non-admin cannot add verifier
#[test]
fn test_error_unauthorized_add_verifier() {
    let harness = TestHarness::new();
    let attacker = &harness.accounts.attacker;
    let new_verifier = Address::generate(&harness.env);

    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::add_verifier(
                harness.env.clone(),
                attacker.clone(),
                new_verifier.clone(),
            )
        });

    assert_eq!(result, Err(AttestationError::Unauthorized));
}

/// Test: Non-verifier cannot create attestation
#[test]
fn test_error_unauthorized_attestation() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let attacker = &harness.accounts.attacker;
    let amount = 1_000_000_000_000i128;

    // Create a commitment first
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

    // Attacker tries to create attestation
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::attest(
                harness.env.clone(),
                attacker.clone(),
                commitment_id.clone(),
                String::from_str(&harness.env, "health_check"),
                harness.health_check_data(),
                true,
            )
        });

    assert_eq!(result, Err(AttestationError::Unauthorized));
}

/// Test: Non-admin cannot register pool
#[test]
fn test_error_unauthorized_pool_registration() {
    let harness = TestHarness::new();
    let attacker = &harness.accounts.attacker;

    let result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::register_pool(
                harness.env.clone(),
                attacker.clone(),
                99,
                RiskLevel::High,
                5000,
                1_000_000_000_000_000,
            )
        });

    assert_eq!(result, Err(AllocationError::Unauthorized));
}

/// Test: Non-owner cannot early exit
#[test]
#[should_panic(expected = "Unauthorized")]
fn test_error_unauthorized_early_exit() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let attacker = &harness.accounts.attacker;
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

    // Attacker tries to early exit someone else's commitment
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::early_exit(
                harness.env.clone(),
                commitment_id.clone(),
                attacker.clone(),
            )
        });
}

/// Test: Non-owner cannot transfer NFT
#[test]
fn test_error_unauthorized_nft_transfer() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let attacker = &harness.accounts.attacker;
    let recipient = Address::generate(&harness.env);
    let amount = 1_000_000_000_000i128;

    // Owner creates commitment (gets NFT)
    harness.approve_tokens(owner, &harness.contracts.commitment_core, amount);

    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                owner.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Attacker tries to transfer owner's NFT
    let result = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::transfer(
                harness.env.clone(),
                attacker.clone(), // Wrong sender
                recipient.clone(),
                0,
            )
        });

    assert_eq!(result, Err(NftError::NotOwner));
}

// ============================================================================
// Invalid Input Tests
// ============================================================================

/// Test: Zero amount commitment fails
#[test]
#[should_panic(expected = "Invalid amount")]
fn test_error_zero_amount_commitment() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;

    harness.approve_tokens(user, &harness.contracts.commitment_core, 1000);

    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                0, // Zero amount
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });
}

/// Test: Invalid duration fails
#[test]
#[should_panic(expected = "Invalid duration")]
fn test_error_zero_duration_commitment() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let rules = CommitmentRules {
        duration_days: 0, // Invalid
        max_loss_percent: 10,
        commitment_type: String::from_str(&harness.env, "balanced"),
        early_exit_penalty: 5,
        min_fee_threshold: 1000,
    };

    harness
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
}

/// Test: Invalid max loss percent fails
#[test]
#[should_panic(expected = "Invalid percent")]
fn test_error_invalid_max_loss_percent() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 150, // Invalid (> 100)
        commitment_type: String::from_str(&harness.env, "balanced"),
        early_exit_penalty: 5,
        min_fee_threshold: 1000,
    };

    harness
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
}

/// Test: Invalid commitment type fails
#[test]
#[should_panic(expected = "Invalid commitment type")]
fn test_error_invalid_commitment_type() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&harness.env, "invalid_type"),
        early_exit_penalty: 5,
        min_fee_threshold: 1000,
    };

    harness
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
}

/// Test: Invalid attestation type fails
#[test]
fn test_error_invalid_attestation_type() {
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

    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::attest(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                String::from_str(&harness.env, "invalid_attestation_type"),
                harness.health_check_data(),
                true,
            )
        });

    assert_eq!(result, Err(AttestationError::InvalidAttestationType));
}

/// Test: Empty commitment ID fails
#[test]
fn test_error_empty_commitment_id_attestation() {
    let harness = TestHarness::new();
    let verifier = &harness.accounts.verifier;

    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::attest(
                harness.env.clone(),
                verifier.clone(),
                String::from_str(&harness.env, ""), // Empty ID
                String::from_str(&harness.env, "health_check"),
                harness.health_check_data(),
                true,
            )
        });

    assert_eq!(result, Err(AttestationError::InvalidCommitmentId));
}

/// Test: Zero amount allocation fails
#[test]
fn test_error_zero_amount_allocation() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;

    harness.setup_default_pools();

    let result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::allocate(
                harness.env.clone(),
                user.clone(),
                1u64,
                0, // Zero amount
                Strategy::Balanced,
            )
        });

    assert_eq!(result, Err(AllocationError::InvalidAmount));
}

/// Test: Negative amount allocation fails
#[test]
fn test_error_negative_amount_allocation() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;

    harness.setup_default_pools();

    let result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::allocate(
                harness.env.clone(),
                user.clone(),
                1u64,
                -1000, // Negative amount
                Strategy::Balanced,
            )
        });

    assert_eq!(result, Err(AllocationError::InvalidAmount));
}

// ============================================================================
// Replay/Duplicate Operation Tests
// ============================================================================

/// Test: Double allocation fails
#[test]
fn test_error_double_allocation() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 500_000_000_000i128;

    harness.setup_default_pools();

    // First allocation succeeds
    let result1 = harness
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
    assert!(result1.is_ok());

    // Second allocation with same commitment_id fails
    let result2 = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::allocate(
                harness.env.clone(),
                user.clone(),
                1u64, // Same commitment_id
                amount,
                Strategy::Balanced,
            )
        });
    assert_eq!(result2, Err(AllocationError::AlreadyInitialized));
}

/// Test: Double initialization fails
#[test]
fn test_error_double_initialization_attestation_engine() {
    let harness = TestHarness::new();
    let admin = &harness.accounts.admin;
    let core = &harness.contracts.commitment_core;

    // Already initialized in harness, try again
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::initialize(
                harness.env.clone(),
                admin.clone(),
                core.clone(),
            )
        });

    assert_eq!(result, Err(AttestationError::AlreadyInitialized));
}

/// Test: Double initialization of NFT contract fails
#[test]
fn test_error_double_initialization_nft() {
    let harness = TestHarness::new();
    let admin = &harness.accounts.admin;

    // Already initialized in harness, try again
    let result = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::initialize(harness.env.clone(), admin.clone())
        });

    assert_eq!(result, Err(NftError::AlreadyInitialized));
}

/// Test: Double settlement fails
#[test]
fn test_error_double_settlement() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

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

    // Advance past expiration
    harness.advance_days(2);

    // First settlement succeeds
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::settle(harness.env.clone(), commitment_id.clone())
        });

    // Second settlement should fail (commitment not active)
    let result = std::panic::catch_unwind(|| {
        harness
            .env
            .as_contract(&harness.contracts.commitment_core, || {
                CommitmentCoreContract::settle(harness.env.clone(), commitment_id.clone())
            })
    });
    assert!(result.is_err());
}

// ============================================================================
// Boundary Value Tests
// ============================================================================

/// Test: Maximum duration commitment
#[test]
fn test_boundary_max_duration() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 100_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let rules = CommitmentRules {
        duration_days: u32::MAX, // Maximum possible duration
        max_loss_percent: 10,
        commitment_type: String::from_str(&harness.env, "balanced"),
        early_exit_penalty: 5,
        min_fee_threshold: 1000,
    };

    // This should succeed (no explicit max duration)
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

    // Verify commitment was created
    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id)
        });
    assert_eq!(commitment.rules.duration_days, u32::MAX);
}

/// Test: Minimum valid amount (1 unit)
#[test]
fn test_boundary_minimum_amount() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1i128; // Minimum possible amount

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

    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id)
        });
    assert_eq!(commitment.amount, 1);
}

/// Test: Max loss percent at boundary (100%)
#[test]
fn test_boundary_max_loss_percent_100() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 100_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 100, // Maximum valid percent
        commitment_type: String::from_str(&harness.env, "aggressive"),
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

    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id)
        });
    assert_eq!(commitment.rules.max_loss_percent, 100);
}

/// Test: Zero max loss percent
#[test]
fn test_boundary_max_loss_percent_0() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 100_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 0, // Zero tolerance
        commitment_type: String::from_str(&harness.env, "safe"),
        early_exit_penalty: 0,
        min_fee_threshold: 0,
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

    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id)
        });
    assert_eq!(commitment.rules.max_loss_percent, 0);
}

// ============================================================================
// Resource Exhaustion Tests
// ============================================================================

/// Test: Get non-existent commitment fails
#[test]
#[should_panic(expected = "Commitment not found")]
fn test_error_get_nonexistent_commitment() {
    let harness = TestHarness::new();

    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(
                harness.env.clone(),
                String::from_str(&harness.env, "nonexistent_id"),
            )
        });
}

/// Test: Get non-existent NFT fails
#[test]
fn test_error_get_nonexistent_nft() {
    let harness = TestHarness::new();

    let result = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::get_metadata(harness.env.clone(), 99999)
        });

    assert_eq!(result, Err(NftError::TokenNotFound));
}

/// Test: Get non-existent pool fails
#[test]
fn test_error_get_nonexistent_pool() {
    let harness = TestHarness::new();

    let result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::get_pool(harness.env.clone(), 99999)
        });

    assert_eq!(result, Err(AllocationError::PoolNotFound));
}

/// Test: Settlement before expiration fails
#[test]
#[should_panic(expected = "Commitment has not expired yet")]
fn test_error_premature_settlement() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
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
                harness.default_rules(), // 30 days
            )
        });

    // Try to settle immediately (before expiration)
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::settle(harness.env.clone(), commitment_id.clone())
        });
}

/// Test: Allocation to no pools fails
#[test]
fn test_error_allocation_no_pools() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;

    // Don't register any pools

    let result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::allocate(
                harness.env.clone(),
                user.clone(),
                1u64,
                1_000_000_000_000,
                Strategy::Balanced,
            )
        });

    assert_eq!(result, Err(AllocationError::NoSuitablePools));
}

/// Test: Rebalance non-existent allocation fails
#[test]
fn test_error_rebalance_nonexistent() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;

    harness.setup_default_pools();

    let result = harness
        .env
        .as_contract(&harness.contracts.allocation_logic, || {
            AllocationStrategiesContract::rebalance(
                harness.env.clone(),
                user.clone(),
                99999u64, // Non-existent
            )
        });

    assert_eq!(result, Err(AllocationError::AllocationNotFound));
}
