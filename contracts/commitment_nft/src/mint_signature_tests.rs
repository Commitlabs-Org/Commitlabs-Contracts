//! Tests for issue #216: mint signature alignment with `commitment_core`.
//!
//! # What this module verifies
//!
//! `commitment_core::call_nft_mint` invokes `commitment_nft::mint` via
//! `e.invoke_contract` with arguments pushed in this exact order:
//!
//! ```text
//! caller, owner, commitment_id, duration_days, max_loss_percent,
//! commitment_type, initial_amount, asset_address, early_exit_penalty
//! ```
//!
//! Any positional mismatch silently passes the wrong value to the wrong field
//! because Soroban's dynamic dispatch does not check argument names — only
//! types and positions. This module makes the alignment explicit and
//! deterministic.
//!
//! # Invariants tested
//! 1. `early_exit_penalty` is stored in `CommitmentNFT::early_exit_penalty`.
//! 2. `early_exit_penalty` is stored in `CommitmentMetadata::early_exit_penalty`.
//! 3. Both fields hold the **same** value after mint.
//! 4. Argument order matches `commitment_core::call_nft_mint` push order.
//! 5. Boundary values (0, 100) are accepted and round-trip correctly.
//! 6. `early_exit_penalty` is independent of `max_loss_percent` (different fields,
//!    different positions — a swap would be caught here).
//! 7. Multiple mints with different penalty values each store their own value.

#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup(e: &Env) -> (Address, CommitmentNFTContractClient<'_>) {
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(e, &contract_id);
    let admin = Address::generate(e);
    client.initialize(&admin);
    (admin, client)
}

/// Mint with explicit penalty value; returns (token_id, nft).
fn mint_with_penalty(
    e: &Env,
    client: &CommitmentNFTContractClient,
    caller: &Address,
    owner: &Address,
    asset: &Address,
    penalty: u32,
) -> (u32, CommitmentNFT) {
    let token_id = client.mint(
        caller,
        owner,
        &String::from_str(e, "ignored_id"),
        &30,
        &20,
        &String::from_str(e, "balanced"),
        &1_000,
        asset,
        &penalty,
    );
    let nft = client.get_metadata(&token_id);
    (token_id, nft)
}

// ---------------------------------------------------------------------------
// Invariant 1 & 2: early_exit_penalty stored in both locations
// ---------------------------------------------------------------------------

/// Invariant: early_exit_penalty is stored in CommitmentNFT::early_exit_penalty.
#[test]
fn invariant_early_exit_penalty_stored_in_nft_top_level() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    let (_, nft) = mint_with_penalty(&e, &client, &admin, &owner, &asset, 15);
    assert_eq!(
        nft.early_exit_penalty, 15,
        "CommitmentNFT::early_exit_penalty must equal the minted value"
    );
}

/// Invariant: early_exit_penalty is stored in CommitmentMetadata::early_exit_penalty.
#[test]
fn invariant_early_exit_penalty_stored_in_metadata() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    let (_, nft) = mint_with_penalty(&e, &client, &admin, &owner, &asset, 15);
    assert_eq!(
        nft.metadata.early_exit_penalty, 15,
        "CommitmentMetadata::early_exit_penalty must equal the minted value"
    );
}

// ---------------------------------------------------------------------------
// Invariant 3: both fields hold the same value
// ---------------------------------------------------------------------------

/// Invariant: CommitmentNFT::early_exit_penalty == CommitmentMetadata::early_exit_penalty.
#[test]
fn invariant_penalty_fields_are_consistent() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    for penalty in [0u32, 5, 10, 15, 50, 100] {
        let (_, nft) = mint_with_penalty(&e, &client, &admin, &owner, &asset, penalty);
        assert_eq!(
            nft.early_exit_penalty,
            nft.metadata.early_exit_penalty,
            "Top-level and metadata early_exit_penalty must be equal for penalty={}",
            penalty
        );
    }
}

// ---------------------------------------------------------------------------
// Invariant 4: argument order matches commitment_core::call_nft_mint
// ---------------------------------------------------------------------------

/// Invariant: positional argument order is (caller, owner, commitment_id,
/// duration_days, max_loss_percent, commitment_type, initial_amount,
/// asset_address, early_exit_penalty).
///
/// We verify this by using distinct values for max_loss_percent (20) and
/// early_exit_penalty (10) and asserting each lands in the correct field.
/// A positional swap between these two would be caught immediately.
#[test]
fn invariant_argument_order_penalty_not_swapped_with_max_loss() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test"),
        &30,   // duration_days
        &20,   // max_loss_percent  ← position 5
        &String::from_str(&e, "balanced"),
        &1_000,
        &asset,
        &10,   // early_exit_penalty ← position 9 (last)
    );
    let nft = client.get_metadata(&token_id);

    assert_eq!(nft.metadata.max_loss_percent, 20, "max_loss_percent must be 20");
    assert_eq!(nft.early_exit_penalty, 10, "early_exit_penalty must be 10");
    assert_eq!(nft.metadata.early_exit_penalty, 10, "metadata.early_exit_penalty must be 10");
    assert_ne!(
        nft.metadata.max_loss_percent, nft.early_exit_penalty,
        "max_loss_percent and early_exit_penalty must not be equal (would hide a swap)"
    );
}

/// Invariant: duration_days and early_exit_penalty are distinct fields at
/// distinct positions. A swap would store 30 in penalty and 10 in duration.
#[test]
fn invariant_argument_order_penalty_not_swapped_with_duration() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test"),
        &30,  // duration_days ← position 4
        &20,
        &String::from_str(&e, "balanced"),
        &1_000,
        &asset,
        &10,  // early_exit_penalty ← position 9
    );
    let nft = client.get_metadata(&token_id);

    assert_eq!(nft.metadata.duration_days, 30, "duration_days must be 30");
    assert_eq!(nft.early_exit_penalty, 10, "early_exit_penalty must be 10");
}

// ---------------------------------------------------------------------------
// Invariant 5: boundary values round-trip correctly
// ---------------------------------------------------------------------------

/// Invariant: penalty = 0 is accepted and stored correctly.
#[test]
fn invariant_penalty_zero_accepted() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    let (_, nft) = mint_with_penalty(&e, &client, &admin, &owner, &asset, 0);
    assert_eq!(nft.early_exit_penalty, 0);
    assert_eq!(nft.metadata.early_exit_penalty, 0);
}

/// Invariant: penalty = 100 is accepted and stored correctly.
#[test]
fn invariant_penalty_max_accepted() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    let (_, nft) = mint_with_penalty(&e, &client, &admin, &owner, &asset, 100);
    assert_eq!(nft.early_exit_penalty, 100);
    assert_eq!(nft.metadata.early_exit_penalty, 100);
}

// ---------------------------------------------------------------------------
// Invariant 6: early_exit_penalty is independent of max_loss_percent
// ---------------------------------------------------------------------------

/// Invariant: changing only early_exit_penalty does not affect max_loss_percent.
#[test]
fn invariant_penalty_independent_of_max_loss() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test"),
        &30,
        &25,   // max_loss_percent = 25
        &String::from_str(&e, "balanced"),
        &1_000,
        &asset,
        &50,   // early_exit_penalty = 50
    );
    let nft = client.get_metadata(&token_id);

    assert_eq!(nft.metadata.max_loss_percent, 25);
    assert_eq!(nft.early_exit_penalty, 50);
    assert_eq!(nft.metadata.early_exit_penalty, 50);
}

// ---------------------------------------------------------------------------
// Invariant 7: multiple mints each store their own penalty
// ---------------------------------------------------------------------------

/// Invariant: each minted NFT stores its own early_exit_penalty independently.
#[test]
fn invariant_multiple_mints_store_independent_penalties() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    let penalties = [5u32, 10, 15, 20, 50, 100];
    let mut token_ids = [0u32; 6];

    for (i, &p) in penalties.iter().enumerate() {
        let (tid, _) = mint_with_penalty(&e, &client, &admin, &owner, &asset, p);
        token_ids[i] = tid;
    }

    for (i, &p) in penalties.iter().enumerate() {
        let nft = client.get_metadata(&token_ids[i]);
        assert_eq!(
            nft.early_exit_penalty, p,
            "token {} must have early_exit_penalty={}",
            token_ids[i], p
        );
        assert_eq!(
            nft.metadata.early_exit_penalty, p,
            "token {} metadata must have early_exit_penalty={}",
            token_ids[i], p
        );
    }
}

// ---------------------------------------------------------------------------
// Auth: only authorized callers can mint
// ---------------------------------------------------------------------------

/// Invariant: unauthorized caller is rejected regardless of early_exit_penalty value.
#[test]
fn invariant_unauthorized_caller_rejected() {
    let e = Env::default();
    let (_, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);
    let attacker = Address::generate(&e);

    let result = client.try_mint(
        &attacker,
        &owner,
        &String::from_str(&e, "test"),
        &30,
        &20,
        &String::from_str(&e, "balanced"),
        &1_000,
        &asset,
        &10,
    );
    assert!(result.is_err(), "Unauthorized caller must be rejected");
}

/// Invariant: admin is always an authorized caller.
#[test]
fn invariant_admin_is_authorized_caller() {
    let e = Env::default();
    let (admin, client) = setup(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    let result = client.try_mint(
        &admin,
        &owner,
        &String::from_str(&e, "test"),
        &30,
        &20,
        &String::from_str(&e, "balanced"),
        &1_000,
        &asset,
        &10,
    );
    assert!(result.is_ok(), "Admin must be an authorized caller");
}
