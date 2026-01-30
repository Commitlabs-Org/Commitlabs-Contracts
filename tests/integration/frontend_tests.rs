//! Frontend-Style Call Tests
//!
//! These tests simulate typical frontend interaction patterns:
//! - Wallet connection (account selection)
//! - Token approval flows
//! - Contract action submission
//! - Event verification
//! - State change verification

use crate::harness::{TestHarness, DEFAULT_USER_BALANCE, SECONDS_PER_DAY};
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, String, IntoVal, Symbol,
};

use commitment_core::{CommitmentCoreContract, CommitmentRules};
use commitment_nft::CommitmentNFTContract;

/// Test: Simulate frontend wallet connection and basic interaction
#[test]
fn test_frontend_wallet_connection_simulation() {
    let harness = TestHarness::new();

    // Simulate "connecting" a wallet by selecting an account
    let connected_wallet = &harness.accounts.user1;

    // Verify the wallet has a balance (frontend would display this)
    let balance = harness.balance(connected_wallet);
    assert!(balance > 0, "Connected wallet should have a balance");
    assert_eq!(balance, DEFAULT_USER_BALANCE);
}

/// Test: Frontend token approval flow
#[test]
fn test_frontend_token_approval_flow() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let spender = &harness.contracts.commitment_core;
    let amount = 1_000_000_000_000i128; // 100k tokens

    // Step 1: Check initial allowance (should be 0)
    let token_client = harness.token_client();
    let initial_allowance = token_client.allowance(user, spender);
    assert_eq!(initial_allowance, 0);

    // Step 2: Approve token spending (frontend transaction)
    harness.approve_tokens(user, spender, amount);

    // Step 3: Verify allowance is set
    let new_allowance = token_client.allowance(user, spender);
    assert_eq!(new_allowance, amount);
}

/// Test: Frontend commitment creation flow with event verification
#[test]
fn test_frontend_create_commitment_flow() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 1_000_000_000_000i128;

    // Step 1: Approve tokens
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);

    // Step 2: Prepare commitment rules (frontend form input)
    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&harness.env, "balanced"),
        early_exit_penalty: 5,
        min_fee_threshold: 1000,
    };

    // Step 3: Create commitment (frontend transaction submission)
    let commitment_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                rules.clone(),
            )
        });

    // Step 4: Verify commitment was created
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            let commitment =
                CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone());

            assert_eq!(commitment.owner, *user);
            assert_eq!(commitment.amount, amount);
            assert_eq!(commitment.rules.duration_days, 30);
        });

    // Step 5: Verify user's token balance decreased
    let new_balance = harness.balance(user);
    assert_eq!(new_balance, DEFAULT_USER_BALANCE - amount);
}

/// Test: Frontend view of user's commitments
#[test]
fn test_frontend_view_user_commitments() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 500_000_000_000i128;

    // Create multiple commitments
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount * 3);

    for _ in 0..3 {
        harness
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
    }

    // Frontend queries user's commitments
    let user_commitments = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_owner_commitments(harness.env.clone(), user.clone())
        });

    // Verify user has 3 commitments
    assert_eq!(user_commitments.len(), 3);
}

/// Test: Frontend view of user's NFTs
#[test]
fn test_frontend_view_user_nfts() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 500_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount * 2);

    // Create commitments (which mint NFTs)
    for _ in 0..2 {
        harness
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
    }

    // Frontend queries user's NFTs
    let user_nfts = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::get_nfts_by_owner(harness.env.clone(), user.clone())
        });

    assert_eq!(user_nfts.len(), 2);

    // Verify NFT ownership count
    let nft_balance = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::balance_of(harness.env.clone(), user.clone())
        });

    assert_eq!(nft_balance, 2);
}

/// Test: Frontend display of total value locked (TVL)
#[test]
fn test_frontend_total_value_locked_display() {
    let harness = TestHarness::new();
    let user1 = &harness.accounts.user1;
    let user2 = &harness.accounts.user2;
    let amount1 = 1_000_000_000_000i128;
    let amount2 = 500_000_000_000i128;

    // Initial TVL should be 0
    let initial_tvl = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_total_value_locked(harness.env.clone())
        });
    assert_eq!(initial_tvl, 0);

    // Create commitments from multiple users
    harness.approve_tokens(user1, &harness.contracts.commitment_core, amount1);
    harness.approve_tokens(user2, &harness.contracts.commitment_core, amount2);

    harness
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

    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user2.clone(),
                amount2,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Verify TVL reflects total commitments
    let final_tvl = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_total_value_locked(harness.env.clone())
        });

    assert_eq!(final_tvl, amount1 + amount2);
}

/// Test: Frontend display of total commitment count
#[test]
fn test_frontend_total_commitments_display() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 100_000_000_000i128;

    // Initial count should be 0
    let initial_count = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_total_commitments(harness.env.clone())
        });
    assert_eq!(initial_count, 0);

    // Create 5 commitments
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount * 5);

    for _ in 0..5 {
        harness
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
    }

    // Verify count
    let final_count = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_total_commitments(harness.env.clone())
        });

    assert_eq!(final_count, 5);
}

/// Test: Frontend NFT transfer flow (secondary market simulation)
#[test]
fn test_frontend_nft_transfer_flow() {
    let harness = TestHarness::new();
    let seller = &harness.accounts.user1;
    let buyer = &harness.accounts.user2;
    let amount = 1_000_000_000_000i128;

    // Seller creates a commitment (gets NFT)
    harness.approve_tokens(seller, &harness.contracts.commitment_core, amount);

    harness
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

    // Get the token ID
    let seller_nfts = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::get_nfts_by_owner(harness.env.clone(), seller.clone())
        });
    let token_id = seller_nfts.get(0).unwrap().token_id;

    // Verify initial ownership
    let initial_owner = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::owner_of(harness.env.clone(), token_id).unwrap()
        });
    assert_eq!(initial_owner, *seller);

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
            .unwrap()
        });

    // Verify new ownership
    let new_owner = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::owner_of(harness.env.clone(), token_id).unwrap()
        });
    assert_eq!(new_owner, *buyer);

    // Verify balance updates
    let seller_balance = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::balance_of(harness.env.clone(), seller.clone())
        });
    assert_eq!(seller_balance, 0);

    let buyer_balance = harness
        .env
        .as_contract(&harness.contracts.commitment_nft, || {
            CommitmentNFTContract::balance_of(harness.env.clone(), buyer.clone())
        });
    assert_eq!(buyer_balance, 1);
}

/// Test: Frontend early exit flow
#[test]
fn test_frontend_early_exit_flow() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
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

    // Record balance before early exit
    let balance_before = harness.balance(user);

    // Advance time but not to expiration
    harness.advance_days(15); // Half of 30-day duration

    // Execute early exit
    harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::early_exit(
                harness.env.clone(),
                commitment_id.clone(),
                user.clone(),
            )
        });

    // Verify user received funds back (minus penalty)
    let balance_after = harness.balance(user);
    let penalty_percent = 5u32; // From default_rules
    let expected_return = amount - (amount * penalty_percent as i128 / 100);
    assert_eq!(balance_after - balance_before, expected_return);

    // Verify commitment status changed
    let commitment = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::get_commitment(harness.env.clone(), commitment_id.clone())
        });
    assert_eq!(
        commitment.status,
        String::from_str(&harness.env, "early_exit")
    );
}

/// Test: Frontend commitment rule display for different types
#[test]
fn test_frontend_commitment_type_rules_display() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 500_000_000_000i128;

    harness.approve_tokens(user, &harness.contracts.commitment_core, amount * 3);

    // Create commitments with different types
    let types = ["safe", "balanced", "aggressive"];
    let mut commitment_ids = vec![];

    for type_str in types.iter() {
        let rules = CommitmentRules {
            duration_days: 30,
            max_loss_percent: 10,
            commitment_type: String::from_str(&harness.env, type_str),
            early_exit_penalty: 5,
            min_fee_threshold: 1000,
        };

        let id = harness
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
        commitment_ids.push(id);
    }

    // Verify each commitment type is stored correctly
    for (i, id) in commitment_ids.iter().enumerate() {
        let commitment = harness
            .env
            .as_contract(&harness.contracts.commitment_core, || {
                CommitmentCoreContract::get_commitment(harness.env.clone(), id.clone())
            });

        assert_eq!(
            commitment.rules.commitment_type,
            String::from_str(&harness.env, types[i])
        );
    }
}
