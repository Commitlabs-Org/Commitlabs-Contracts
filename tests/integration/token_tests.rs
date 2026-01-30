//! Token/Asset Interaction Tests
//!
//! These tests verify:
//! - Token transfer flows
//! - Approve and transfer_from patterns
//! - Insufficient balance handling
//! - Allowance errors
//! - Decimal/rounding edge cases

use crate::harness::{TestHarness, DEFAULT_USER_BALANCE, SECONDS_PER_DAY};
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

/// Test: Basic token transfer
#[test]
fn test_token_basic_transfer() {
    let harness = TestHarness::new();
    let sender = &harness.accounts.user1;
    let recipient = &harness.accounts.user2;
    let amount = 1_000_000_000_000i128;

    let sender_balance_before = harness.balance(sender);
    let recipient_balance_before = harness.balance(recipient);

    // Transfer tokens
    harness.token_client().transfer(sender, recipient, &amount);

    // Verify balances
    let sender_balance_after = harness.balance(sender);
    let recipient_balance_after = harness.balance(recipient);

    assert_eq!(sender_balance_after, sender_balance_before - amount);
    assert_eq!(recipient_balance_after, recipient_balance_before + amount);
}

/// Test: Token approve and allowance
#[test]
fn test_token_approve_and_allowance() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let spender = &harness.accounts.user2;
    let amount = 500_000_000_000i128;

    // Initial allowance is 0
    let initial_allowance = harness.token_client().allowance(owner, spender);
    assert_eq!(initial_allowance, 0);

    // Approve spending
    harness.approve_tokens(owner, spender, amount);

    // Verify allowance
    let new_allowance = harness.token_client().allowance(owner, spender);
    assert_eq!(new_allowance, amount);
}

/// Test: Transfer from with allowance
#[test]
fn test_token_transfer_from() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let spender = &harness.accounts.user2;
    let recipient = Address::generate(&harness.env);
    let approve_amount = 500_000_000_000i128;
    let transfer_amount = 300_000_000_000i128;

    // Owner approves spender
    harness.approve_tokens(owner, spender, approve_amount);

    let owner_balance_before = harness.balance(owner);
    let recipient_balance_before = harness.balance(&recipient);

    // Spender transfers from owner to recipient
    harness
        .token_client()
        .transfer_from(spender, owner, &recipient, &transfer_amount);

    // Verify balances
    assert_eq!(harness.balance(owner), owner_balance_before - transfer_amount);
    assert_eq!(
        harness.balance(&recipient),
        recipient_balance_before + transfer_amount
    );

    // Verify allowance decreased
    let remaining_allowance = harness.token_client().allowance(owner, spender);
    assert_eq!(remaining_allowance, approve_amount - transfer_amount);
}

/// Test: Insufficient balance transfer fails
#[test]
#[should_panic]
fn test_token_insufficient_balance_fails() {
    let harness = TestHarness::new();
    let sender = &harness.accounts.user1;
    let recipient = &harness.accounts.user2;

    // Try to transfer more than balance
    let excessive_amount = DEFAULT_USER_BALANCE + 1;
    harness
        .token_client()
        .transfer(sender, recipient, &excessive_amount);
}

/// Test: Insufficient allowance transfer_from fails
#[test]
#[should_panic]
fn test_token_insufficient_allowance_fails() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let spender = &harness.accounts.user2;
    let recipient = Address::generate(&harness.env);

    // Approve small amount
    harness.approve_tokens(owner, spender, 100_000_000);

    // Try to transfer more than allowance
    harness
        .token_client()
        .transfer_from(spender, owner, &recipient, &500_000_000);
}

/// Test: Zero amount transfer
#[test]
fn test_token_zero_amount_transfer() {
    let harness = TestHarness::new();
    let sender = &harness.accounts.user1;
    let recipient = &harness.accounts.user2;

    let sender_balance_before = harness.balance(sender);
    let recipient_balance_before = harness.balance(recipient);

    // Transfer zero tokens
    harness.token_client().transfer(sender, recipient, &0);

    // Balances should be unchanged
    assert_eq!(harness.balance(sender), sender_balance_before);
    assert_eq!(harness.balance(recipient), recipient_balance_before);
}

/// Test: Approve zero clears allowance
#[test]
fn test_token_approve_zero_clears_allowance() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let spender = &harness.accounts.user2;

    // Set allowance
    harness.approve_tokens(owner, spender, 1_000_000_000_000);

    // Verify allowance is set
    assert!(harness.token_client().allowance(owner, spender) > 0);

    // Approve zero
    harness.approve_tokens(owner, spender, 0);

    // Allowance should be zero
    assert_eq!(harness.token_client().allowance(owner, spender), 0);
}

/// Test: Multiple transfers in sequence
#[test]
fn test_token_multiple_sequential_transfers() {
    let harness = TestHarness::new();
    let sender = &harness.accounts.user1;
    let recipient = &harness.accounts.user2;
    let amount = 100_000_000_000i128;

    let initial_sender = harness.balance(sender);
    let initial_recipient = harness.balance(recipient);

    // Multiple transfers
    for _ in 0..5 {
        harness.token_client().transfer(sender, recipient, &amount);
    }

    // Verify final balances
    assert_eq!(harness.balance(sender), initial_sender - (amount * 5));
    assert_eq!(harness.balance(recipient), initial_recipient + (amount * 5));
}

/// Test: Transfer to self
#[test]
fn test_token_transfer_to_self() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let amount = 100_000_000_000i128;

    let balance_before = harness.balance(user);

    // Transfer to self
    harness.token_client().transfer(user, user, &amount);

    // Balance should be unchanged
    assert_eq!(harness.balance(user), balance_before);
}

/// Test: Token minting by admin
#[test]
fn test_token_admin_minting() {
    let harness = TestHarness::new();
    let new_user = Address::generate(&harness.env);
    let mint_amount = 5_000_000_000_000i128;

    // Initial balance is 0
    assert_eq!(harness.balance(&new_user), 0);

    // Admin mints tokens
    harness.token_admin_client().mint(&new_user, &mint_amount);

    // Verify new balance
    assert_eq!(harness.balance(&new_user), mint_amount);
}

/// Test: Commitment token lock flow
#[test]
fn test_token_commitment_lock_flow() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let contract = &harness.contracts.commitment_core;
    let amount = 1_000_000_000_000i128;

    // Record initial balances
    let user_balance_before = harness.balance(user);
    let contract_balance_before = harness.balance(contract);

    // Approve token spending by contract
    harness.approve_tokens(user, contract, amount);

    // Simulate commitment creation token transfer
    harness.token_client().transfer_from(
        contract, // Spender (commitment_core)
        user,     // From (user)
        contract, // To (commitment_core holds locked funds)
        &amount,
    );

    // Verify balances after lock
    assert_eq!(harness.balance(user), user_balance_before - amount);
    assert_eq!(harness.balance(contract), contract_balance_before + amount);
}

/// Test: Commitment token release flow
#[test]
fn test_token_commitment_release_flow() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let contract = &harness.contracts.commitment_core;
    let amount = 1_000_000_000_000i128;

    // First lock tokens
    harness.approve_tokens(user, contract, amount);
    harness
        .token_client()
        .transfer_from(contract, user, contract, &amount);

    // Record balances before release
    let user_balance_before = harness.balance(user);
    let contract_balance_before = harness.balance(contract);

    // Simulate settlement (contract transfers back to user)
    // In actual tests, this is done via the contract method
    harness.token_client().transfer(contract, user, &amount);

    // Verify balances after release
    assert_eq!(harness.balance(user), user_balance_before + amount);
    assert_eq!(harness.balance(contract), contract_balance_before - amount);
}

/// Test: Small amount transfers (dust amounts)
#[test]
fn test_token_small_amount_transfers() {
    let harness = TestHarness::new();
    let sender = &harness.accounts.user1;
    let recipient = &harness.accounts.user2;

    // Transfer very small amounts
    let dust_amounts = vec![1i128, 10, 100, 1000];

    for amount in dust_amounts {
        let sender_before = harness.balance(sender);
        let recipient_before = harness.balance(recipient);

        harness.token_client().transfer(sender, recipient, &amount);

        assert_eq!(harness.balance(sender), sender_before - amount);
        assert_eq!(harness.balance(recipient), recipient_before + amount);
    }
}

/// Test: Large amount transfers (boundary values)
#[test]
fn test_token_large_amount_transfers() {
    let harness = TestHarness::new();
    let sender = &harness.accounts.user1;
    let recipient = &harness.accounts.user2;

    // Transfer entire balance minus 1
    let large_amount = DEFAULT_USER_BALANCE - 1;

    let sender_before = harness.balance(sender);
    let recipient_before = harness.balance(recipient);

    harness.token_client().transfer(sender, recipient, &large_amount);

    assert_eq!(harness.balance(sender), 1);
    assert_eq!(harness.balance(recipient), recipient_before + large_amount);
}

/// Test: Allowance update (increase)
#[test]
fn test_token_allowance_increase() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let spender = &harness.accounts.user2;

    let initial_amount = 100_000_000_000i128;
    let increase_amount = 200_000_000_000i128;

    // Set initial allowance
    harness.approve_tokens(owner, spender, initial_amount);
    assert_eq!(
        harness.token_client().allowance(owner, spender),
        initial_amount
    );

    // Increase allowance
    harness.approve_tokens(owner, spender, initial_amount + increase_amount);
    assert_eq!(
        harness.token_client().allowance(owner, spender),
        initial_amount + increase_amount
    );
}

/// Test: Allowance update (decrease)
#[test]
fn test_token_allowance_decrease() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let spender = &harness.accounts.user2;

    let initial_amount = 300_000_000_000i128;
    let new_amount = 100_000_000_000i128;

    // Set initial allowance
    harness.approve_tokens(owner, spender, initial_amount);

    // Decrease allowance
    harness.approve_tokens(owner, spender, new_amount);
    assert_eq!(harness.token_client().allowance(owner, spender), new_amount);
}

/// Test: Multiple spenders with different allowances
#[test]
fn test_token_multiple_spenders() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let spender1 = &harness.accounts.user2;
    let spender2 = &harness.accounts.attacker;

    let amount1 = 100_000_000_000i128;
    let amount2 = 200_000_000_000i128;

    // Set different allowances for different spenders
    harness.approve_tokens(owner, spender1, amount1);
    harness.approve_tokens(owner, spender2, amount2);

    // Verify independent allowances
    assert_eq!(harness.token_client().allowance(owner, spender1), amount1);
    assert_eq!(harness.token_client().allowance(owner, spender2), amount2);
}

/// Test: Full allowance consumption
#[test]
fn test_token_full_allowance_consumption() {
    let harness = TestHarness::new();
    let owner = &harness.accounts.user1;
    let spender = &harness.accounts.user2;
    let recipient = Address::generate(&harness.env);
    let amount = 100_000_000_000i128;

    // Approve exact amount
    harness.approve_tokens(owner, spender, amount);

    // Transfer entire allowance
    harness
        .token_client()
        .transfer_from(spender, owner, &recipient, &amount);

    // Allowance should be 0
    assert_eq!(harness.token_client().allowance(owner, spender), 0);
}

/// Test: Token balance after commitment with penalty
#[test]
fn test_token_balance_after_early_exit_penalty() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let contract = &harness.contracts.commitment_core;
    let amount = 1_000_000_000_000i128;
    let penalty_percent = 5u32;

    // Lock tokens
    harness.approve_tokens(user, contract, amount);
    harness
        .token_client()
        .transfer_from(contract, user, contract, &amount);

    let user_balance_before_release = harness.balance(user);

    // Calculate penalty and return amounts
    let penalty_amount = amount * penalty_percent as i128 / 100;
    let return_amount = amount - penalty_amount;

    // Simulate early exit with penalty (return partial amount)
    harness.token_client().transfer(contract, user, &return_amount);

    // Verify user received amount minus penalty
    assert_eq!(
        harness.balance(user),
        user_balance_before_release + return_amount
    );
}

/// Test: Token conservation (no tokens created/destroyed)
#[test]
fn test_token_conservation() {
    let harness = TestHarness::new();
    let user1 = &harness.accounts.user1;
    let user2 = &harness.accounts.user2;
    let contract = &harness.contracts.commitment_core;

    // Calculate total token supply in test environment
    let total_before =
        harness.balance(user1) + harness.balance(user2) + harness.balance(contract);

    // Perform various operations
    harness.approve_tokens(user1, contract, 500_000_000_000);
    harness
        .token_client()
        .transfer_from(contract, user1, contract, &500_000_000_000);
    harness.token_client().transfer(user2, user1, &100_000_000_000);
    harness
        .token_client()
        .transfer(contract, user2, &200_000_000_000);

    // Calculate total after operations
    let total_after = harness.balance(user1) + harness.balance(user2) + harness.balance(contract);

    // Total should be unchanged
    assert_eq!(total_before, total_after);
}
