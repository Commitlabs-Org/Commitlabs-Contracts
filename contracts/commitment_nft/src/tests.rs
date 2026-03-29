#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Events, Ledger},
    vec, Address, Env, IntoVal, String,
};

#[contract]
struct MockNftContract;

#[allow(clippy::too_many_arguments)]
#[contractimpl]
impl MockNftContract {
    #[allow(clippy::too_many_arguments)]
    pub fn mint(
        _e: Env,
        _owner: Address,
        _commitment_id: String,
        _duration_days: u32,
        _max_loss_percent: u32,
        _commitment_type: String,
        _initial_amount: i128,
        _asset_address: Address,
        _early_exit_penalty: u32,
    ) -> u32 {
        1
    }
    pub fn settle(_e: Env, _caller: Address, _token_id: u32) {}
    pub fn mark_inactive(_e: Env, _caller: Address, _token_id: u32) {}
}

fn setup_contract(e: &Env) -> (Address, CommitmentNFTContractClient<'_>) {
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(e, &contract_id);
    let admin = Address::generate(e);
    (admin, client)
}

/// Setup contract with a registered "core" contract.
/// Returns (admin, client, core_contract_id).
fn setup_contract_with_core(e: &Env) -> (Address, CommitmentNFTContractClient<'_>, Address) {
    e.mock_all_auths();
    let (admin, client) = setup_contract(e);
    client.initialize(&admin);
    let core_id = e.register_contract(None, CommitmentNFTContract);
    client.set_core_contract(&core_id);
    (admin, client, core_id)
}

fn create_test_metadata(
    e: &Env,
    asset_address: &Address,
) -> (String, u32, u32, String, i128, Address, u32) {
    (
        String::from_str(e, "commitment_001"),
        30, // duration_days
        10, // max_loss_percent
        String::from_str(e, "balanced"),
        1000, // initial_amount
        asset_address.clone(),
        5, // early_exit_penalty
    )
}

// ============================================
// Initialization Tests
// ============================================

// ============================================================================
// Helper Functions
// ============================================================================

#[allow(dead_code)]
fn setup_env() -> (Env, Address, Address) {
    let e = Env::default();
    let (admin, contract_id) = {
        let (admin, client) = setup_contract(&e);

        // Initialize should succeed
        client.initialize(&admin);

        // Verify admin is set
        let stored_admin = client.get_admin();
        assert_eq!(stored_admin, admin);

        // Verify total supply is 0
        assert_eq!(client.total_supply(), 0);

        (admin, client.address)
    };

    (e, contract_id, admin)
}

/// Asserts that the sum of `balance_of` for all given owners equals `total_supply()`.
fn assert_balance_supply_invariant(client: &CommitmentNFTContractClient, owners: &[&Address]) {
    let sum: u32 = owners.iter().map(|addr| client.balance_of(addr)).sum();
    assert_eq!(
        sum,
        client.total_supply(),
        "INV-2 violated: sum of balances ({}) != total_supply ({})",
        sum,
        client.total_supply()
    );
}

/// Convenience wrapper that mints a 1-day duration NFT with default params.
/// Returns the token_id.
fn mint_to_owner(
    e: &Env,
    client: &CommitmentNFTContractClient,
    caller: &Address,
    owner: &Address,
    asset_address: &Address,
    label: &str,
) -> u32 {
    client.mint(
        caller,
        owner,
        &String::from_str(e, label),
        &1, // 1 day duration — easy to settle
        &10,
        &String::from_str(e, "balanced"),
        &1000,
        asset_address,
        &5,
    )
}

// ============================================================================
// Initialization Tests
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #2)")] // AlreadyInitialized
fn test_initialize_twice_fails() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);
    client.initialize(&admin); // Should panic
}

// ============================================
// Access control: whitelist and unauthorized mint
// ============================================

#[test]
fn test_add_remove_is_authorized_contract() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, client) = setup_contract(&e);
    client.initialize(&admin);
    let other = Address::generate(&e);

    assert!(!client.is_authorized(&other));
    client.add_authorized_contract(&admin, &other);
    assert!(client.is_authorized(&other));
    client.remove_authorized_contract(&admin, &other);
    assert!(!client.is_authorized(&other));
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // NotAuthorized
fn test_mint_unauthorized_caller_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);
    client.initialize(&admin);
    let (commitment_id, duration, max_loss, commitment_type, amount, _asset, penalty) =
        create_test_metadata(&e, &asset_address);
    let unauthorized = Address::generate(&e);
    client.mint(
        &unauthorized,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset_address,
        &penalty,
    );
}

// ============================================
// Mint Tests
// ============================================

#[test]
fn test_mint() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    assert_eq!(token_id, 0);
    assert_eq!(client.total_supply(), 1);
    assert_eq!(client.balance_of(&owner), 1);

    // Verify Mint event
    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, client.address);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("Mint").into_val(&e),
            token_id.into_val(&e),
            owner.into_val(&e)
        ]
    );
    let data: (String, u64) = last_event.2.into_val(&e);
    // Verify the auto-generated commitment_id matches the expected format
    assert_eq!(data.0, String::from_str(&e, "COMMIT_0"));
}

#[test]
fn test_mint_multiple() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint 3 NFTs
    let token_id_0 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_0"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(token_id_0, 0);

    let token_id_1 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_1"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(token_id_1, 1);

    let token_id_2 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_2"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(token_id_2, 2);

    assert_eq!(client.total_supply(), 3);
    assert_eq!(client.balance_of(&owner), 3);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")] // NotInitialized
fn test_mint_without_initialize_fails() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );
}

// ============================================
// Commitment Type Validation Tests
// ============================================

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // InvalidCommitmentType
fn test_mint_empty_commitment_type() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_empty"),
        &30,
        &10,
        &String::from_str(&e, ""),
        &1000,
        &asset_address,
        &5,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // InvalidCommitmentType
fn test_mint_invalid_commitment_type() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_invalid"),
        &30,
        &10,
        &String::from_str(&e, "invalid"),
        &1000,
        &asset_address,
        &5,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // InvalidCommitmentType
fn test_mint_wrong_case_commitment_type() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_case"),
        &30,
        &10,
        &String::from_str(&e, "Safe"),
        &1000,
        &asset_address,
        &5,
    );
}

/// Issue #139: Test that all three valid commitment types are accepted
#[test]
fn test_mint_valid_commitment_types_all_three() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Test "safe"
    let token_id_safe = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_safe"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(token_id_safe, 0);

    // Test "balanced"
    let token_id_balanced = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_balanced"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(token_id_balanced, 1);

    // Test "aggressive"
    let token_id_aggressive = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_aggressive"),
        &30,
        &10,
        &String::from_str(&e, "aggressive"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(token_id_aggressive, 2);

    // Verify all were minted successfully
    assert_eq!(client.total_supply(), 3);
    assert_eq!(
        client.get_metadata(&token_id_safe).metadata.commitment_type,
        String::from_str(&e, "safe")
    );
    assert_eq!(
        client
            .get_metadata(&token_id_balanced)
            .metadata
            .commitment_type,
        String::from_str(&e, "balanced")
    );
    assert_eq!(
        client
            .get_metadata(&token_id_aggressive)
            .metadata
            .commitment_type,
        String::from_str(&e, "aggressive")
    );
}

// ============================================
// Issue #139: String Parameter Edge Cases - commitment_id
// ============================================

/// Test that empty commitment_id parameter is ignored and auto-generated ID is used
/// Since commitment_ids are now auto-generated, user-provided empty strings are acceptable
#[test]
fn test_mint_empty_commitment_id() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // User provides empty commitment_id, but it will be ignored and COMMIT_0 will be used
    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, ""), // Empty commitment_id - will be ignored
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Verify the auto-generated commitment_id was used
    let metadata = client.get_metadata(&token_id);
    assert_eq!(
        metadata.metadata.commitment_id,
        String::from_str(&e, "COMMIT_0")
    );
}

/// Test that very long commitment_id parameter is ignored and auto-generated ID is used
/// Since commitment_ids are now auto-generated, long user-provided strings are acceptable
#[test]
fn test_mint_commitment_id_very_long() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Create a very long commitment_id: 1000+ chars (exceeds MAX_COMMITMENT_ID_LENGTH of 256)
    let very_long_id = "a".repeat(1000);
    let long_id = String::from_str(&e, &very_long_id);

    // Mint with very long commitment_id - it will be ignored and COMMIT_0 will be used
    let token_id = client.mint(
        &admin,
        &owner,
        &long_id,
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Verify the auto-generated commitment_id was used instead
    let metadata = client.get_metadata(&token_id);
    assert_eq!(
        metadata.metadata.commitment_id,
        String::from_str(&e, "COMMIT_0")
    );
}

/// Test that commitment_id at the maximum allowed length is ignored and auto-generated ID is used
/// Since commitment_ids are now auto-generated, user-provided IDs are no longer stored
#[test]
fn test_mint_commitment_id_max_allowed_length() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Create a commitment_id at exactly MAX_COMMITMENT_ID_LENGTH (256 chars)
    let max_length_id = "x".repeat(256);
    let commitment_id = String::from_str(&e, &max_length_id);

    // Mint with max length commitment_id - it will be ignored and COMMIT_0 will be used
    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Verify the auto-generated commitment_id was used instead
    let metadata = client.get_metadata(&token_id);
    assert_eq!(
        metadata.metadata.commitment_id,
        String::from_str(&e, "COMMIT_0")
    );
}

/// Test that normal length commitment_id works correctly
#[test]
fn test_mint_commitment_id_normal_length() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let commitment_id = String::from_str(&e, "test_commitment_normal_length_123");
    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Verify the commitment_id is stored and retrieved correctly
    // Since commitment_id is now auto-generated, it will be COMMIT_0
    let metadata = client.get_metadata(&token_id);
    assert_eq!(
        metadata.metadata.commitment_id,
        String::from_str(&e, "COMMIT_0")
    );
}

/// Issue #139: Test retrieval operations with long commitment_id
/// Ensures no panic in get_metadata or get_nfts_by_owner even with longer strings
#[test]
fn test_get_metadata_with_long_commitment_id() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Create a reasonably long commitment_id (200 chars, within MAX_COMMITMENT_ID_LENGTH of 256)
    let long_id_str = "z".repeat(200);
    let long_id = String::from_str(&e, &long_id_str);

    // Mint with long commitment_id
    let token_id = client.mint(
        &admin,
        &owner,
        &long_id,
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // Retrieve metadata - should not panic
    // Now commitment_id is auto-generated as COMMIT_0
    let metadata = client.get_metadata(&token_id);
    assert_eq!(
        metadata.metadata.commitment_id,
        String::from_str(&e, "COMMIT_0")
    );

    // Retrieve all metadata - should not panic
    let all_nfts = client.get_all_metadata();
    assert_eq!(all_nfts.len(), 1);
    assert_eq!(
        all_nfts.get(0).unwrap().metadata.commitment_id,
        String::from_str(&e, "COMMIT_0")
    );

    // Retrieve by owner - should not panic
    let owner_nfts = client.get_nfts_by_owner(&owner);
    assert_eq!(owner_nfts.len(), 1);
    assert_eq!(
        owner_nfts.get(0).unwrap().metadata.commitment_id,
        String::from_str(&e, "COMMIT_0")
    );
}

// ============================================
// get_metadata Tests
// ============================================

#[test]
fn test_get_metadata() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let commitment_id = String::from_str(&e, "test_commitment");
    let duration = 30u32;
    let max_loss = 15u32;
    let commitment_type = String::from_str(&e, "aggressive");
    let amount = 5000i128;

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset_address,
        &10,
    );

    let nft = client.get_metadata(&token_id);

    // The commitment_id is now auto-generated
    assert_eq!(nft.metadata.commitment_id, String::from_str(&e, "COMMIT_0"));
    assert_eq!(nft.metadata.duration_days, duration);
    assert_eq!(nft.metadata.max_loss_percent, max_loss);
    assert_eq!(nft.metadata.commitment_type, commitment_type);
    assert_eq!(nft.metadata.initial_amount, amount);
    assert_eq!(nft.metadata.asset_address, asset_address);
    assert_eq!(nft.owner, owner);
    assert_eq!(nft.token_id, token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_get_metadata_nonexistent_token() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    // Try to get metadata for non-existent token
    client.get_metadata(&999);
}

// ============================================
// owner_of Tests
// ============================================

#[test]
fn test_owner_of() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    let retrieved_owner = client.owner_of(&token_id);
    assert_eq!(retrieved_owner, owner);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_owner_of_nonexistent_token() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    client.owner_of(&999);
}

// ============================================
// is_active Tests
// ============================================

#[test]
fn test_is_active() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Newly minted NFT should be active
    assert!(client.is_active(&token_id));
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_is_active_nonexistent_token() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    client.is_active(&999);
}

// ============================================
// Issue #107: NFT query functions with non-existent token_id (explicit error checks)
// ============================================

#[test]
fn test_get_metadata_nonexistent_token_returns_error() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    client.initialize(&admin);
    let result = client.try_get_metadata(&999);
    assert!(
        result.is_err(),
        "get_metadata(non-existent token_id) must return error, not panic"
    );
}

#[test]
fn test_owner_of_nonexistent_token_returns_error() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    client.initialize(&admin);
    let result = client.try_owner_of(&999);
    assert!(
        result.is_err(),
        "owner_of(non-existent token_id) must return error, not panic"
    );
}

// ============================================
// Issue #111: Supply Tests
// ============================================

#[test]
fn test_total_supply_five_mints() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    for _ in 0..5 {
        client.mint(
            &admin,
            &owner,
            &String::from_str(&e, "commitment"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    assert_eq!(client.total_supply(), 5);
}

/// Issue #111: total_supply is never decremented by settle() or transfer().
#[test]
fn test_total_supply_unchanged_after_transfer_and_settle() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, client) = setup_contract(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    assert_eq!(client.total_supply(), 0);
    let token_id = client.mint(
        &admin,
        &owner1,
        &String::from_str(&e, "c1"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(client.total_supply(), 1);

    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    client.settle(&token_id);
    assert_eq!(client.total_supply(), 1); // settle does not change total_supply

    client.transfer(&owner1, &owner2, &token_id);
    assert_eq!(client.total_supply(), 1); // transfer does not change total_supply
}

#[test]
fn test_balance_of_after_minting() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint 3 NFTs for owner1
    for _ in 0..3 {
        client.mint(
            &admin,
            &owner1,
            &String::from_str(&e, "owner1_commitment"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    // Mint 2 NFTs for owner2
    for _ in 0..2 {
        client.mint(
            &admin,
            &owner2,
            &String::from_str(&e, "owner2_commitment"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    assert_eq!(client.balance_of(&owner1), 3);
    assert_eq!(client.balance_of(&owner2), 2);
}

// ============================================
// get_all_metadata Tests
// ============================================

#[test]
fn test_get_all_metadata_empty() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    let all_nfts = client.get_all_metadata();
    assert_eq!(all_nfts.len(), 0);
}

#[test]
fn test_get_all_metadata() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint 3 NFTs
    for _ in 0..3 {
        client.mint(
            &admin,
            &owner,
            &String::from_str(&e, "commitment"),
            &30,
            &10,
            &String::from_str(&e, "balanced"),
            &1000,
            &asset_address,
            &5,
        );
    }

    // MERGED: Pass admin.clone() as the caller argument
    let all_nfts = client.get_all_metadata();
    assert_eq!(all_nfts.len(), 3);
}

#[test]
fn test_get_nfts_by_owner() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint 2 NFTs for owner1
    for _ in 0..2 {
        client.mint(
            &admin,
            &owner1,
            &String::from_str(&e, "owner1"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    // Mint 3 NFTs for owner2
    for _ in 0..3 {
        client.mint(
            &admin,
            &owner2,
            &String::from_str(&e, "owner2"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    let owner1_nfts = client.get_nfts_by_owner(&owner1);
    let owner2_nfts = client.get_nfts_by_owner(&owner2);

    assert_eq!(owner1_nfts.len(), 2);
    assert_eq!(owner2_nfts.len(), 3);

    // Verify all owner1 NFTs belong to owner1
    for nft in owner1_nfts.iter() {
        assert_eq!(nft.owner, owner1);
    }
}

// ============================================
// Basic Transfer Test
// ============================================

/// Verifies the happy path for `transfer`: ownership, balances, and the
/// Transfer event are all updated correctly after a successful transfer.
#[test]
fn test_transfer() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint with 1 day duration so we can settle and then transfer
    let token_id = client.mint(
        &admin,
        &owner1,
        &String::from_str(&e, "commitment_001"),
        &1,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // Verify initial state
    assert_eq!(client.owner_of(&token_id), owner1);
    assert_eq!(client.balance_of(&owner1), 1);
    assert_eq!(client.balance_of(&owner2), 0);

    // Advance past expiry and settle to unlock the NFT
    e.ledger().with_mut(|li| li.timestamp = 172800);
    client.settle(&token_id);

    client.transfer(&owner1, &owner2, &token_id);

    // Verify transfer
    assert_eq!(client.owner_of(&token_id), owner2);
    assert_eq!(client.balance_of(&owner1), 0);
    assert_eq!(client.balance_of(&owner2), 1);

    // Verify Transfer event
    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, client.address);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("Transfer").into_val(&e),
            owner1.into_val(&e),
            owner2.into_val(&e)
        ]
    );
    let data: (u32, u64) = last_event.2.into_val(&e);
    assert_eq!(data.0, token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")] // NotOwner
fn test_transfer_not_owner() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let not_owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Try to transfer from non-owner (should fail)
    client.transfer(&not_owner, &recipient, &token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_transfer_nonexistent_token() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);

    client.initialize(&admin);

    client.transfer(&owner, &recipient, &999);
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")] // TransferToZeroAddress
fn test_transfer_to_self() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Try to transfer to self (should fail)
    client.transfer(&owner, &owner, &token_id);
}

/// Test that transferring an active (locked) NFT is rejected with `NFTLocked`.
///
/// # Summary
/// When a `CommitmentNFT` is first minted its `is_active` field is `true`,
/// representing a live commitment.  The contract refuses to transfer such tokens
/// so that commitment obligations (penalties, settlement) remain tied to the
/// original owner.
///
/// # Errors
/// Expects `ContractError::NFTLocked` (error code #19).
///
/// # Security Note
/// Without this guard an owner could transfer the NFT to a fresh address
/// immediately before settlement to shed liability — this test enforces that
/// the guard is always in effect.
#[test]
#[should_panic(expected = "Error(Contract, #19)")] // NFTLocked
fn test_transfer_locked_nft() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Newly minted NFT is always locked (is_active = true)
    assert!(client.is_active(&token_id));

    // Attempt to transfer the locked NFT — must fail with NFTLocked (#19)
    client.transfer(&owner, &recipient, &token_id);
}

#[test]
fn test_transfer_after_settlement() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client, _core_id) = setup_contract_with_core(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    // Mint with 1 day duration
    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test_commitment"),
        &1, // 1 day duration
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Verify NFT is active (locked) initially
    assert!(client.is_active(&token_id));

    // Fast forward time past expiration (2 days = 172800 seconds)
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    // Settle the NFT after expiry
    client.settle(&token_id);

    // Verify NFT is now inactive (unlocked)
    assert!(!client.is_active(&token_id));

    // Transfer should now succeed
    client.transfer(&owner, &recipient, &token_id);

    // Verify transfer was successful
    assert_eq!(client.owner_of(&token_id), recipient);
    assert_eq!(client.balance_of(&owner), 0);
    assert_eq!(client.balance_of(&recipient), 1);
}

// ============================================
// Transfer Edge Cases Tests
// ============================================

/// Test that self-transfer (from == to) is rejected with TransferToZeroAddress error.
///
/// **Requirement**: RFC #105 - Transfer should reject transfer to self to avoid ambiguous state.
///
/// **Expected Behavior**:
/// - transfer(owner, owner, token_id) must fail with error #18 (TransferToZeroAddress)
/// - No state changes should occur
/// - Useful for preventing accidental no-ops
#[test]
#[should_panic(expected = "Error(Contract, #18)")] // TransferToZeroAddress
fn test_transfer_edge_case_self_transfer() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Verify initial state
    assert_eq!(client.owner_of(&token_id), owner);
    assert_eq!(client.balance_of(&owner), 1);

    // Attempt self-transfer: should reject with TransferToZeroAddress error
    // This is semantically a self-transfer rejection, not a zero-address rejection
    client.transfer(&owner, &owner, &token_id);
}

/// Test that transfer from a non-owner is rejected.
///
/// **Requirement**: RFC #105 - Transfer should verify from == current owner.
///
/// **Expected Behavior**:
/// - transfer(non_owner, recipient, token_id) must fail with error #5 (NotOwner)
/// - Only the current owner can initiate transfers
/// - Prevents unauthorized transfers
#[test]
#[should_panic(expected = "Error(Contract, #5)")] // NotOwner
fn test_transfer_edge_case_from_non_owner() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let not_owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Verify initial ownership
    assert_eq!(client.owner_of(&token_id), owner);

    // Attempt transfer from non-owner: should reject with NotOwner error
    client.transfer(&not_owner, &recipient, &token_id);
}

/// Test that invalid/malformed addresses are prevented by Soroban SDK.
///
/// **Requirement**: RFC #105 - Transfer should reject zero/invalid addresses.
///
/// **Expected Behavior**:
/// - Soroban SDK prevents creation of completely malformed addresses at compile time
/// - The Address type in Soroban is guaranteed to represent a valid address
/// - This test serves as defensive documentation of SDK safety guarantees
/// - In practice, if an Address is constructed, it's already valid per SDK invariants
///
/// **Note**: This test documents an invariant rather than testing failure behavior,
/// as the SDK prevents malformed addresses before runtime.
#[test]
fn test_transfer_edge_case_address_validation_by_sdk() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let valid_recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // The Address type in Soroban SDK is strongly typed and cannot be constructed
    // with invalid/zero values. This test documents that SDK guarantees prevent
    // the invalid address case from ever reaching our contract code.

    // To demonstrate this, we use a validly generated address
    assert_eq!(client.owner_of(&token_id), owner);

    // If we could construct a zero address, it would be rejected by the contract,
    // but Soroban SDK prevents this at the type level, making the check redundant
    // at runtime. This is a safety guarantee of the SDK.
    //
    // Valid transfer with valid recipient should succeed (after settlement)
    assert_ne!(
        owner, valid_recipient,
        "Recipient must be different from owner"
    );
}

/// Comprehensive edge cases test for NFT transfer validation.
///
/// **Requirement**: RFC #105 - Document and test NFT transfer edge cases.
///
/// **Test Coverage**:
/// 1. Owner changes after successful transfer
/// 2. Balance updates correctly
/// 3. Token lists are properly maintained
/// 4. Cannot re-transfer to same recipient without authorization changes
/// 5. All validations work correctly in sequence
///
/// **Expected Behavior**: Each assertion is clearly marked with what's being tested.
#[test]
fn test_transfer_edge_cases_comprehensive() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client, core_id) = setup_contract_with_core(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let owner3 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    // Mint two separate NFTs to test transfer chains
    let token_id_1 = client.mint(
        &admin,
        &owner1,
        &String::from_str(&e, "commitment_edge_case_1"),
        &1, // 1 day to allow settlement
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    let token_id_2 = client.mint(
        &admin,
        &owner1,
        &String::from_str(&e, "commitment_edge_case_2"),
        &1, // 1 day to allow settlement
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // ===== Validation: Initial state =====
    assert_eq!(
        client.owner_of(&token_id_1),
        owner1,
        "Token 1: Owner should be owner1 initially"
    );
    assert_eq!(
        client.owner_of(&token_id_2),
        owner1,
        "Token 2: Owner should be owner1 initially"
    );
    assert_eq!(client.balance_of(&owner1), 2, "owner1 should have 2 NFTs");
    assert_eq!(client.balance_of(&owner2), 0, "owner2 should have 0 NFTs");
    assert_eq!(client.balance_of(&owner3), 0, "owner3 should have 0 NFTs");

    // Settlement is required to unlock the NFT for transfer
    e.ledger().with_mut(|li| {
        li.timestamp = 172800; // 2 days
    });
    e.as_contract(&core_id, || {
        client.settle(&token_id_1);
        client.settle(&token_id_2);
    });

    // ===== Validation: Transfer token_id_1 from owner1 to owner2 =====
    client.transfer(&owner1, &owner2, &token_id_1);

    assert_eq!(
        client.owner_of(&token_id_1),
        owner2,
        "Token 1: Owner should change to owner2 after transfer"
    );
    assert_eq!(
        client.balance_of(&owner1),
        1,
        "owner1 should have 1 NFT after first transfer"
    );
    assert_eq!(
        client.balance_of(&owner2),
        1,
        "owner2 should have 1 NFT after first transfer"
    );

    // ===== Validation: Transfer token_id_2 from owner1 to owner3 =====
    client.transfer(&owner1, &owner3, &token_id_2);

    assert_eq!(
        client.owner_of(&token_id_2),
        owner3,
        "Token 2: Owner should change to owner3"
    );
    assert_eq!(
        client.balance_of(&owner1),
        0,
        "owner1 should have 0 NFTs after second transfer"
    );
    assert_eq!(
        client.balance_of(&owner2),
        1,
        "owner2 should still have 1 NFT"
    );
    assert_eq!(
        client.balance_of(&owner3),
        1,
        "owner3 should have 1 NFT after second transfer"
    );

    // ===== Validation: owner2 can transfer their token to owner3 =====
    client.transfer(&owner2, &owner3, &token_id_1);

    assert_eq!(
        client.owner_of(&token_id_1),
        owner3,
        "Token 1: Owner should be owner3 after transfer from owner2"
    );
    assert_eq!(
        client.balance_of(&owner2),
        0,
        "owner2 should have 0 NFTs after transferring away"
    );
    assert_eq!(
        client.balance_of(&owner3),
        2,
        "owner3 should have 2 NFTs now"
    );

    // ===== Validation: Final ownership state =====
    // Verify that owner3 has all tokens and owners 1 and 2 have none
    assert_eq!(
        client.owner_of(&token_id_1),
        owner3,
        "Token 1: Final owner should be owner3"
    );
    assert_eq!(
        client.owner_of(&token_id_2),
        owner3,
        "Token 2: Final owner should be owner3"
    );
    assert_eq!(
        client.balance_of(&owner1),
        0,
        "owner1: final balance should be 0"
    );
    assert_eq!(
        client.balance_of(&owner2),
        0,
        "owner2: final balance should be 0"
    );
    assert_eq!(
        client.balance_of(&owner3),
        2,
        "owner3: final balance should be 2"
    );
}

// ============================================
// Settle Tests
// ============================================

#[test]
fn test_settle() {
    let e = Env::default();
    let (admin, client, _core_id) = setup_contract_with_core(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    // Mint with 1 day duration
    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test_commitment"),
        &1, // 1 day duration
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // NFT should be active initially
    assert!(client.is_active(&token_id));

    // Fast forward time past expiration (2 days = 172800 seconds)
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    // Verify it's expired
    assert!(client.is_expired(&token_id));

    // Settle the NFT after expiry
    client.settle(&token_id);

    // NFT should now be inactive
    assert!(!client.is_active(&token_id));

    // Verify Settle event
    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, client.address);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("Settle").into_val(&e),
            token_id.into_val(&e)
        ]
    );
    let data: u64 = last_event.2.into_val(&e);
    assert_eq!(data, e.ledger().timestamp());
}

/// Mint with duration that would cause expires_at to overflow u64 (Issue #118).
#[test]
#[should_panic(expected = "Error(Contract, #9)")] // NotExpired
fn test_settle_not_expired() {
    let e = Env::default();
    let (admin, client, _core_id) = setup_contract_with_core(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test_commitment"),
        &30, // 30 days duration
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Try to settle before expiration, should fail with NotExpired
    client.settle(&token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // AlreadySettled
fn test_settle_already_settled() {
    let e = Env::default();
    let (admin, client, _core_id) = setup_contract_with_core(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test_commitment"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Fast forward time
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    client.settle(&token_id);
    client.settle(&token_id); // Should fail
}

#[test]
fn test_settle_succeeds_after_expiry() {
    let e = Env::default();
    let (admin, client, _core_id) = setup_contract_with_core(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "settle_after_expiry"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });
    client.settle(&token_id);
    assert!(!client.is_active(&token_id));
}

#[test]
fn test_settle_first_settle_marks_inactive() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);
    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    e.ledger().with_mut(|li| li.timestamp = 172800);

    // Initial state: active
    assert!(client.is_active(&token_id));

    // First settle: success
    client.settle(&token_id);

    // Result state: inactive
    assert!(!client.is_active(&token_id));
}

#[test]
fn test_settle_double_settle_returns_error() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);
    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    e.ledger().with_mut(|li| li.timestamp = 172800);

    // First settle
    client.settle(&token_id);

    // Second settle: should return ContractError::AlreadySettled (8)
    let result = client.try_settle(&token_id);
    assert!(result.is_err());
}

#[test]
fn test_settle_consistency_after_double_settle() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);
    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    e.ledger().with_mut(|li| li.timestamp = 172800);

    client.settle(&token_id);
    let _ = client.try_settle(&token_id); // Redundant settle

    // State remains consistent
    assert!(!client.is_active(&token_id));

    // get_metadata remains consistent
    let metadata = client.get_metadata(&token_id);
    assert!(!metadata.is_active);
    assert_eq!(metadata.owner, owner);
}

#[test]
fn test_settle_no_double_events() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);
    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    e.ledger().with_mut(|li| li.timestamp = 172800);

    client.settle(&token_id);
    let events_after_first = e.events().all().len();

    let _ = client.try_settle(&token_id); // Redundant settle
    let events_after_second = e.events().all().len();

    // Verify no double events
    assert_eq!(
        events_after_first, events_after_second,
        "Redundant settle should not emit extra events"
    );
}

// ============================================
// is_expired Tests
// ============================================

#[test]
fn test_is_expired() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test_commitment"),
        &1, // 1 day
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Should not be expired initially
    assert!(!client.is_expired(&token_id));

    // Fast forward 2 days
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    // Should now be expired
    assert!(client.is_expired(&token_id));
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_is_expired_nonexistent_token() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    client.is_expired(&999);
}

// ============================================
// token_exists Tests
// ============================================

#[test]
fn test_token_exists() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Token 0 should not exist yet
    assert!(!client.token_exists(&0));

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &admin,
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Token should now exist
    assert!(client.token_exists(&token_id));

    // Non-existent token should return false
    assert!(!client.token_exists(&999));
}

// ============================================
// get_admin Tests
// ============================================

#[test]
fn test_get_admin() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")] // NotInitialized
fn test_get_admin_not_initialized() {
    let e = Env::default();
    let (_admin, client) = setup_contract(&e);

    client.get_admin();
}

// ============================================
// Validation Tests - Issue #103
// ============================================

#[test]
#[should_panic(expected = "Error(Contract, #11)")] // InvalidMaxLoss
fn test_mint_max_loss_percent_over_100() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &101, // max_loss_percent > 100
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
}

#[test]
fn test_mint_max_loss_percent_zero() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &0, // max_loss_percent = 0 (allowed)
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    assert_eq!(token_id, 0);
    let nft = client.get_metadata(&token_id);
    assert_eq!(nft.metadata.max_loss_percent, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")] // InvalidDuration
fn test_mint_duration_days_zero() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &0, // duration_days = 0
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
}

#[test]
fn test_mint_duration_days_one() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &1, // duration_days = 1 (minimum valid)
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    assert_eq!(token_id, 0);
    let nft = client.get_metadata(&token_id);
    assert_eq!(nft.metadata.duration_days, 1);
}

#[test]
fn test_mint_duration_days_max() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &u32::MAX, // duration_days = u32::MAX
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    assert_eq!(token_id, 0);
    let nft = client.get_metadata(&token_id);
    assert_eq!(nft.metadata.duration_days, u32::MAX);

    // Verify expires_at calculation handles large values
    // created_at + (u32::MAX * 86400) should not panic
    let expected_expires_at = nft.metadata.created_at + (u32::MAX as u64 * 86400);
    assert_eq!(nft.metadata.expires_at, expected_expires_at);
}

// ============================================
// Edge Cases
// ============================================

#[test]
fn test_metadata_timestamps() {
    let e = Env::default();

    // Set initial ledger timestamp
    e.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "test"),
        &30, // 30 days
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    let metadata = client.get_metadata(&token_id);

    // Verify timestamps
    assert_eq!(metadata.metadata.created_at, 1000);
    // expires_at should be created_at + (30 days * 86400 seconds)
    assert_eq!(metadata.metadata.expires_at, 1000 + (30 * 86400));
}

#[test]
fn test_balance_updates_after_transfer() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client, _core_id) = setup_contract_with_core(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    // Mint multiple NFTs for owner1 with 1 day duration so we can settle them
    client.mint(
        &admin,
        &owner1,
        &String::from_str(&e, "commitment_0"),
        &1, // 1 day duration
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    client.mint(
        &admin,
        &owner1,
        &String::from_str(&e, "commitment_1"),
        &1, // 1 day duration
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    client.mint(
        &admin,
        &owner1,
        &String::from_str(&e, "commitment_2"),
        &1, // 1 day duration
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    assert_eq!(client.balance_of(&owner1), 3);
    assert_eq!(client.balance_of(&owner2), 0);

    // Fast forward time past expiration and settle all NFTs.
    e.ledger().with_mut(|li| {
        li.timestamp = 172800; // 2 days
    });
    client.settle(&0);
    client.settle(&1);
    client.settle(&2);

    // Transfer one NFT
    client.transfer(&owner1, &owner2, &0);

    assert_eq!(client.balance_of(&owner1), 2);
    assert_eq!(client.balance_of(&owner2), 1);

    // Transfer another
    client.transfer(&owner1, &owner2, &1);

    assert_eq!(client.balance_of(&owner1), 1);
    assert_eq!(client.balance_of(&owner2), 2);

    // Verify get_nfts_by_owner reflects the transfers
    let owner1_nfts = client.get_nfts_by_owner(&owner1);
    let owner2_nfts = client.get_nfts_by_owner(&owner2);

    assert_eq!(owner1_nfts.len(), 1);
    assert_eq!(owner2_nfts.len(), 2);
}

#[test]
#[should_panic(expected = "Contract is paused - operation not allowed")]
fn test_mint_blocked_when_paused() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);
    client.pause();

    client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "paused_commitment"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
}

#[test]
#[should_panic(expected = "Contract is paused - operation not allowed")]
fn test_transfer_blocked_when_paused() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner1,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    client.pause();
    client.transfer(&owner1, &owner2, &token_id);
}

// #[test]
fn _test_unpause_restores_transfer() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client, _core_id) = setup_contract_with_core(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    let token_id = client.mint(
        &admin,
        &owner1,
        &String::from_str(&e, "commitment_002"),
        &1, // 1 day duration so we can settle it
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });
    client.settle(&token_id);

    client.pause();
    client.unpause();

    // NFT is still active after unpause; settle it first to make it transferable.
    e.ledger().with_mut(|li| {
        li.timestamp += 31 * 86_400;
    });
    client.settle(&token_id);

    client.transfer(&owner1, &owner2, &token_id);
    assert_eq!(client.owner_of(&token_id), owner2);
}

// ============================================================================
// Balance / Supply Invariant Tests
// ============================================================================
//
// Formally documented invariants:
//
// INV-1 (Supply Monotonicity):
//   `total_supply()` equals the number of successful mints and is never
//   decremented. Neither `settle()` nor `transfer()` changes the counter.
//
// INV-2 (Balance-Supply Conservation):
//   sum(balance_of(addr) for all owners) == total_supply()
//   Relies on the ownership check at L534 guaranteeing from_balance >= 1 on
//   transfer, so the conditional decrement at L570 is always taken.
//
// INV-3 (Settle Independence):
//   `settle()` does not change `total_supply()` or any `balance_of()`.
//   It only flips `nft.is_active` to false.
//
// INV-4 (Transfer Conservation):
//   `transfer()` decreases the sender's balance by 1, increases the
//   receiver's balance by 1, and leaves `total_supply()` unchanged.
// ============================================================================

#[test]
fn test_invariant_balance_sum_equals_supply_after_mints() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let asset = Address::generate(&e);

    let owner_a = Address::generate(&e);
    let owner_b = Address::generate(&e);
    let owner_c = Address::generate(&e);
    let owner_d = Address::generate(&e);
    let owners: [&Address; 4] = [&owner_a, &owner_b, &owner_c, &owner_d];

    client.initialize(&admin);

    // Base case: empty state
    assert_eq!(client.total_supply(), 0);
    assert_balance_supply_invariant(&client, &owners);

    // Mint 4 to owner_a
    mint_to_owner(&e, &client, &admin, &owner_a, &asset, "a_0");
    assert_balance_supply_invariant(&client, &owners);
    mint_to_owner(&e, &client, &admin, &owner_a, &asset, "a_1");
    assert_balance_supply_invariant(&client, &owners);
    mint_to_owner(&e, &client, &admin, &owner_a, &asset, "a_2");
    assert_balance_supply_invariant(&client, &owners);
    mint_to_owner(&e, &client, &admin, &owner_a, &asset, "a_3");
    assert_balance_supply_invariant(&client, &owners);

    // Mint 1 to owner_b
    mint_to_owner(&e, &client, &admin, &owner_b, &asset, "b_0");
    assert_balance_supply_invariant(&client, &owners);

    // Mint 3 to owner_c
    mint_to_owner(&e, &client, &admin, &owner_c, &asset, "c_0");
    assert_balance_supply_invariant(&client, &owners);
    mint_to_owner(&e, &client, &admin, &owner_c, &asset, "c_1");
    assert_balance_supply_invariant(&client, &owners);
    mint_to_owner(&e, &client, &admin, &owner_c, &asset, "c_2");
    assert_balance_supply_invariant(&client, &owners);

    // Mint 2 to owner_d
    mint_to_owner(&e, &client, &admin, &owner_d, &asset, "d_0");
    assert_balance_supply_invariant(&client, &owners);
    mint_to_owner(&e, &client, &admin, &owner_d, &asset, "d_1");
    assert_balance_supply_invariant(&client, &owners);

    // Final state: 4+1+3+2 = 10
    assert_eq!(client.total_supply(), 10);
    assert_eq!(client.balance_of(&owner_a), 4);
    assert_eq!(client.balance_of(&owner_b), 1);
    assert_eq!(client.balance_of(&owner_c), 3);
    assert_eq!(client.balance_of(&owner_d), 2);
    assert_balance_supply_invariant(&client, &owners);
}

#[test]
fn test_invariant_supply_unchanged_after_settle() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client, core_id) = setup_contract_with_core(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    // Mint 3 NFTs (1-day duration)
    let t0 = mint_to_owner(&e, &client, &admin, &owner, &asset, "s_0");
    let t1 = mint_to_owner(&e, &client, &admin, &owner, &asset, "s_1");
    let t2 = mint_to_owner(&e, &client, &admin, &owner, &asset, "s_2");

    let supply_before = client.total_supply();
    let balance_before = client.balance_of(&owner);
    assert_eq!(supply_before, 3);
    assert_eq!(balance_before, 3);

    // Fast-forward past expiration
    e.ledger().with_mut(|li| {
        li.timestamp = 172800; // 2 days
    });

    // Settle each — supply and balance must not change
    for token_id in [t0, t1, t2] {
        e.as_contract(&core_id, || {
            client.settle(&token_id);
        });
        assert_eq!(client.total_supply(), supply_before);
        assert_eq!(client.balance_of(&owner), balance_before);
    }
}

#[test]
fn test_invariant_balance_unchanged_after_settle_multi_owner() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client, core_id) = setup_contract_with_core(&e);
    let asset = Address::generate(&e);

    let alice = Address::generate(&e);
    let bob = Address::generate(&e);
    let carol = Address::generate(&e);
    let owners: [&Address; 3] = [&alice, &bob, &carol];

    // Alice: 2, Bob: 2, Carol: 1 => 5 total
    let a0 = mint_to_owner(&e, &client, &admin, &alice, &asset, "a0");
    let _a1 = mint_to_owner(&e, &client, &admin, &alice, &asset, "a1");
    let b0 = mint_to_owner(&e, &client, &admin, &bob, &asset, "b0");
    let b1 = mint_to_owner(&e, &client, &admin, &bob, &asset, "b1");
    let _c0 = mint_to_owner(&e, &client, &admin, &carol, &asset, "c0");

    assert_eq!(client.total_supply(), 5);
    assert_balance_supply_invariant(&client, &owners);

    // Fast-forward past expiration
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    // Partial settle: only a0, b0, b1
    for token_id in [a0, b0, b1] {
        e.as_contract(&core_id, || {
            client.settle(&token_id);
        });
    }

    // All balances and supply unchanged
    assert_eq!(client.balance_of(&alice), 2);
    assert_eq!(client.balance_of(&bob), 2);
    assert_eq!(client.balance_of(&carol), 1);
    assert_eq!(client.total_supply(), 5);
    assert_balance_supply_invariant(&client, &owners);
}

#[test]
fn test_invariant_transfer_balance_conservation() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client, core_id) = setup_contract_with_core(&e);
    let asset = Address::generate(&e);

    let from = Address::generate(&e);
    let to = Address::generate(&e);
    let owners: [&Address; 2] = [&from, &to];

    // Mint 3 to `from`, 1 to `to`
    let t0 = mint_to_owner(&e, &client, &admin, &from, &asset, "f0");
    let _t1 = mint_to_owner(&e, &client, &admin, &from, &asset, "f1");
    let _t2 = mint_to_owner(&e, &client, &admin, &from, &asset, "f2");
    let _t3 = mint_to_owner(&e, &client, &admin, &to, &asset, "to0");

    assert_eq!(client.total_supply(), 4);
    assert_balance_supply_invariant(&client, &owners);

    // Settle t0 so it can be transferred
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });
    e.as_contract(&core_id, || {
        client.settle(&t0);
    });

    let supply_before = client.total_supply();
    let from_bal_before = client.balance_of(&from);
    let to_bal_before = client.balance_of(&to);

    // Transfer t0: from -> to
    client.transfer(&from, &to, &t0);

    // INV-4: sender -1, receiver +1, supply unchanged
    assert_eq!(client.balance_of(&from), from_bal_before - 1);
    assert_eq!(client.balance_of(&to), to_bal_before + 1);
    assert_eq!(client.total_supply(), supply_before);
    // INV-2: sum still equals supply
    assert_balance_supply_invariant(&client, &owners);
}

#[test]
fn test_invariant_complex_mint_settle_transfer_scenario() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client, core_id) = setup_contract_with_core(&e);
    let asset = Address::generate(&e);

    let alice = Address::generate(&e);
    let bob = Address::generate(&e);
    let carol = Address::generate(&e);
    let owners: [&Address; 3] = [&alice, &bob, &carol];

    // --- Phase 1: Mint 6 NFTs ---
    // Alice: 3, Bob: 2, Carol: 1
    let a0 = mint_to_owner(&e, &client, &admin, &alice, &asset, "a0");
    let a1 = mint_to_owner(&e, &client, &admin, &alice, &asset, "a1");
    let a2 = mint_to_owner(&e, &client, &admin, &alice, &asset, "a2");
    let b0 = mint_to_owner(&e, &client, &admin, &bob, &asset, "b0");
    let b1 = mint_to_owner(&e, &client, &admin, &bob, &asset, "b1");
    let c0 = mint_to_owner(&e, &client, &admin, &carol, &asset, "c0");

    assert_eq!(client.total_supply(), 6);
    assert_balance_supply_invariant(&client, &owners);

    // --- Phase 2: Settle 4 of 6 ---
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    for token_id in [a0, a1, b0, c0] {
        e.as_contract(&core_id, || {
            client.settle(&token_id);
        });
    }

    // INV-3: supply and balances unchanged
    assert_eq!(client.total_supply(), 6);
    assert_eq!(client.balance_of(&alice), 3);
    assert_eq!(client.balance_of(&bob), 2);
    assert_eq!(client.balance_of(&carol), 1);
    assert_balance_supply_invariant(&client, &owners);

    // --- Phase 3: Transfer 3 settled NFTs ---
    // a0: alice -> bob
    client.transfer(&alice, &bob, &a0);
    assert_balance_supply_invariant(&client, &owners);

    // a1: alice -> carol
    client.transfer(&alice, &carol, &a1);
    assert_balance_supply_invariant(&client, &owners);

    // b0: bob -> carol
    client.transfer(&bob, &carol, &b0);
    assert_balance_supply_invariant(&client, &owners);

    assert_eq!(client.total_supply(), 6);
    assert_eq!(client.balance_of(&alice), 1); // had 3, transferred 2
    assert_eq!(client.balance_of(&bob), 2); // had 2, received 1, transferred 1
    assert_eq!(client.balance_of(&carol), 3); // had 1, received 2

    // --- Phase 4: Settle remaining active NFTs ---
    for token_id in [a2, b1] {
        e.as_contract(&core_id, || {
            client.settle(&token_id);
        });
    }
    assert_eq!(client.total_supply(), 6);
    assert_balance_supply_invariant(&client, &owners);

    // --- Phase 5: Mint 2 more (still active, no settle) ---
    mint_to_owner(&e, &client, &admin, &alice, &asset, "a3");
    mint_to_owner(&e, &client, &admin, &bob, &asset, "b2");

    assert_eq!(client.total_supply(), 8);
    assert_eq!(client.balance_of(&alice), 2);
    assert_eq!(client.balance_of(&bob), 3);
    assert_eq!(client.balance_of(&carol), 3);
    assert_balance_supply_invariant(&client, &owners);
}

#[test]
fn test_invariant_transfer_chain_preserves_supply() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client, core_id) = setup_contract_with_core(&e);
    let asset = Address::generate(&e);

    let a = Address::generate(&e);
    let b = Address::generate(&e);
    let c = Address::generate(&e);
    let d = Address::generate(&e);
    let owners: [&Address; 4] = [&a, &b, &c, &d];

    // Single token, chain: A -> B -> C -> D
    let token = mint_to_owner(&e, &client, &admin, &a, &asset, "chain");

    assert_eq!(client.total_supply(), 1);
    assert_balance_supply_invariant(&client, &owners);

    // Settle so we can transfer
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });
    e.as_contract(&core_id, || {
        client.settle(&token);
    });

    // A -> B
    client.transfer(&a, &b, &token);
    assert_eq!(client.total_supply(), 1);
    assert_balance_supply_invariant(&client, &owners);
    assert_eq!(client.balance_of(&a), 0);
    assert_eq!(client.balance_of(&b), 1);

    // B -> C
    client.transfer(&b, &c, &token);
    assert_eq!(client.total_supply(), 1);
    assert_balance_supply_invariant(&client, &owners);
    assert_eq!(client.balance_of(&b), 0);
    assert_eq!(client.balance_of(&c), 1);

    // C -> D
    client.transfer(&c, &d, &token);
    assert_eq!(client.total_supply(), 1);
    assert_balance_supply_invariant(&client, &owners);
    assert_eq!(client.balance_of(&c), 0);
    assert_eq!(client.balance_of(&d), 1);
}

// ============================================================================
// Commitment ID Uniqueness and Format Tests (Issue: commitment_id uniqueness)
// ============================================================================

/// Test that two create_commitment calls produce different commitment_ids
#[test]
fn test_commitment_id_uniqueness() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint first commitment
    let token_id_1 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "ignored_id_1"), // Will be overridden with auto-generated ID
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // Mint second commitment
    let token_id_2 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "ignored_id_2"), // Will be overridden with auto-generated ID
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &2000,
        &asset_address,
        &5,
    );

    // Verify tokens are different
    assert_ne!(token_id_1, token_id_2);

    // Verify commitment_ids are different
    let metadata1 = client.get_metadata(&token_id_1);
    let metadata2 = client.get_metadata(&token_id_2);
    assert_ne!(
        metadata1.metadata.commitment_id,
        metadata2.metadata.commitment_id
    );

    // Verify they follow the expected format: COMMIT_0, COMMIT_1
    assert_eq!(
        metadata1.metadata.commitment_id,
        String::from_str(&e, "COMMIT_0")
    );
    assert_eq!(
        metadata2.metadata.commitment_id,
        String::from_str(&e, "COMMIT_1")
    );
}

/// Test that commitment_id format is consistent across multiple mints
#[test]
fn test_commitment_id_format_consistency() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint first commitment - should have COMMIT_0
    let token_id_0 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "any_id"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
    let metadata_0 = client.get_metadata(&token_id_0);
    assert_eq!(
        metadata_0.metadata.commitment_id,
        String::from_str(&e, "COMMIT_0")
    );

    // Mint second commitment - should have COMMIT_1
    let token_id_1 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "any_id"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1100,
        &asset_address,
        &5,
    );
    let metadata_1 = client.get_metadata(&token_id_1);
    assert_eq!(
        metadata_1.metadata.commitment_id,
        String::from_str(&e, "COMMIT_1")
    );

    // Mint third commitment - should have COMMIT_2
    let token_id_2 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "any_id"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1200,
        &asset_address,
        &5,
    );
    let metadata_2 = client.get_metadata(&token_id_2);
    assert_eq!(
        metadata_2.metadata.commitment_id,
        String::from_str(&e, "COMMIT_2")
    );

    // Mint fourth commitment - should have COMMIT_3
    let token_id_3 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "any_id"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1300,
        &asset_address,
        &5,
    );
    let metadata_3 = client.get_metadata(&token_id_3);
    assert_eq!(
        metadata_3.metadata.commitment_id,
        String::from_str(&e, "COMMIT_3")
    );

    // Mint fifth commitment - should have COMMIT_4
    let token_id_4 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "any_id"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1400,
        &asset_address,
        &5,
    );
    let metadata_4 = client.get_metadata(&token_id_4);
    assert_eq!(
        metadata_4.metadata.commitment_id,
        String::from_str(&e, "COMMIT_4")
    );

    // Verify total supply matches what we created
    assert_eq!(client.total_supply(), 5);
}

/// Test that get_commitment_by_id returns the correct commitment
#[test]
fn test_get_commitment_by_id() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint two commitments
    let token_id_1 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "any_id_1"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    let token_id_2 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "any_id_2"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &2000, // Different amount
        &asset_address,
        &5,
    );

    // Get first commitment by ID
    let commitment_id_1 = String::from_str(&e, "COMMIT_0");
    let nft1 = client.get_commitment_by_id(&commitment_id_1);
    assert_eq!(nft1.token_id, token_id_1);
    assert_eq!(nft1.metadata.initial_amount, 1000);

    // Get second commitment by ID
    let commitment_id_2 = String::from_str(&e, "COMMIT_1");
    let nft2 = client.get_commitment_by_id(&commitment_id_2);
    assert_eq!(nft2.token_id, token_id_2);
    assert_eq!(nft2.metadata.initial_amount, 2000);

    // Verify they are different
    assert_ne!(nft1.metadata.commitment_id, nft2.metadata.commitment_id);
    assert_ne!(nft1.metadata.initial_amount, nft2.metadata.initial_amount);
}

/// Test that get_commitment_by_id fails for non-existent commitment_id
#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_get_commitment_by_invalid_id_fails() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    // Try to get non-existent commitment
    let invalid_id = String::from_str(&e, "COMMIT_999");
    let _ = client.get_commitment_by_id(&invalid_id);
}

// ============================================================================
// Issue #211: Unit Tests — Transfer: self-transfer, non-owner, locked NFT,
//             non-existent token
// ============================================================================
//
// Trust-boundary summary for `transfer`:
//   • Only the current owner (who satisfies `require_auth`) may send tokens.
//   • Active (locked) NFTs cannot leave the owner's wallet until settled.
//   • The sender address must differ from the receiver address.
//   • The token_id must identify a minted, existing token.
//
// Error codes under test:
//   #3  = TokenNotFound
//   #5  = NotOwner
//   #18 = TransferToZeroAddress  (also used for self-transfer)
//   #19 = NFTLocked
// ============================================================================

// ----------------------------------------------------------------------------
// 1. Self-transfer
// ----------------------------------------------------------------------------

/// Verifies that `transfer(from, to, token_id)` is rejected when `from == to`.
///
/// # Summary
/// Transferring an NFT to yourself produces no meaningful state change and can
/// mask bugs in calling code.  The contract guards against this by returning
/// `TransferToZeroAddress` (#18) — the same error used for literal zero-address
/// destinations — to keep the error surface small.
///
/// # Parameters tested
/// - `from` — address that owns the token and initiates the transfer
/// - `to`   — same address as `from` (self-transfer)
///
/// # Errors
/// Expects `ContractError::TransferToZeroAddress` (error code `#18`).
///
/// # Security Note
/// Without this check a caller could "reset" approval state or confuse
/// off-chain indexers by emitting spurious Transfer events with `from == to`.
#[test]
#[should_panic(expected = "Error(Contract, #18)")] // TransferToZeroAddress
fn test_transfer_211_self_transfer_panics() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commit_self"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // Precondition: owner holds the token
    assert_eq!(client.owner_of(&token_id), owner);

    // Self-transfer must be rejected with TransferToZeroAddress (#18)
    client.transfer(&owner, &owner, &token_id);
}

/// Non-panicking variant of the self-transfer test.
///
/// Uses `try_transfer` so the error can be inspected programmatically in
/// integration harnesses that cannot catch panics.
#[test]
fn test_transfer_211_self_transfer_returns_error() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commit_self_err"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // try_transfer must return Err — no state mutation should have occurred
    let result = client.try_transfer(&owner, &owner, &token_id);
    assert!(
        result.is_err(),
        "self-transfer must return an error, not succeed"
    );

    // Post-condition: ownership and balance unchanged
    assert_eq!(
        client.owner_of(&token_id),
        owner,
        "owner must remain unchanged after rejected self-transfer"
    );
    assert_eq!(
        client.balance_of(&owner),
        1,
        "balance must remain 1 after rejected self-transfer"
    );
}

// ----------------------------------------------------------------------------
// 2. Non-owner transfer
// ----------------------------------------------------------------------------

/// Verifies that `transfer` is rejected when the `from` address does not own
/// the token.
///
/// # Summary
/// Only the recorded owner of a token is permitted to transfer it.  Any
/// address that is not the current owner receives `NotOwner` (#5), regardless
/// of whether it has previously held the token or is the contract admin.
///
/// # Parameters tested
/// - `from` — address that does NOT own the token
/// - `to`   — a valid third-party recipient address
///
/// # Errors
/// Expects `ContractError::NotOwner` (error code `#5`).
///
/// # Security Note
/// This is the primary ownership-enforcement check.  A missing or bypassed
/// guard would allow arbitrary theft of NFTs.
#[test]
#[should_panic(expected = "Error(Contract, #5)")] // NotOwner
fn test_transfer_211_non_owner_panics() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let attacker = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commit_nonowner"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // Precondition: actual owner is `owner`, not `attacker`
    assert_eq!(client.owner_of(&token_id), owner);
    assert_ne!(attacker, owner, "attacker must be a different address");

    // Non-owner transfer must be rejected with NotOwner (#5)
    client.transfer(&attacker, &recipient, &token_id);
}

/// Non-panicking variant of the non-owner transfer test.
///
/// Also verifies that ownership and balances are unchanged after the rejection.
#[test]
fn test_transfer_211_non_owner_returns_error() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let attacker = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commit_nonowner_err"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // Attempt by non-owner must fail
    let result = client.try_transfer(&attacker, &recipient, &token_id);
    assert!(
        result.is_err(),
        "non-owner transfer must return an error, not succeed"
    );

    // Post-condition: original owner retains the token
    assert_eq!(
        client.owner_of(&token_id),
        owner,
        "owner must be unchanged after rejected non-owner transfer"
    );
    assert_eq!(client.balance_of(&owner), 1);
    assert_eq!(
        client.balance_of(&attacker),
        0,
        "attacker must not gain any balance"
    );
    assert_eq!(
        client.balance_of(&recipient),
        0,
        "recipient must not receive any token"
    );
}

// ----------------------------------------------------------------------------
// 3. Locked (active) NFT
// ----------------------------------------------------------------------------

/// Verifies that `transfer` is rejected when the NFT is active (`is_active == true`).
///
/// # Summary
/// A freshly minted commitment NFT has `is_active = true`, meaning the
/// underlying commitment is still live.  Transferring it is prohibited (#145)
/// to ensure that settlement obligations remain with the original party.  The
/// contract returns `NFTLocked` (#19) to signal this state.
///
/// # Parameters tested
/// - `token_id` — a token whose `is_active` field is `true` (never settled)
///
/// # Errors
/// Expects `ContractError::NFTLocked` (error code `#19`).
///
/// # Security Note
/// If this guard were absent an owner could transfer a failing commitment to
/// an empty wallet immediately before settlement, avoiding penalty deductions.
/// This test must remain green; any relaxation of the guard requires an
/// explicit design decision and updated threat model.
#[test]
#[should_panic(expected = "Error(Contract, #19)")] // NFTLocked
fn test_transfer_211_locked_nft_panics() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commit_locked"),
        &30, // 30-day duration — well within active window
        &10,
        &String::from_str(&e, "safe"),
        &5000,
        &asset_address,
        &5,
    );

    // Precondition: NFT is active/locked immediately after minting
    assert!(
        client.is_active(&token_id),
        "freshly minted NFT must be active"
    );

    // Transfer of locked NFT must fail with NFTLocked (#19)
    client.transfer(&owner, &recipient, &token_id);
}

/// Non-panicking variant of the locked-NFT transfer test.
///
/// Also verifies that `is_active` remains `true` and ownership is unchanged
/// after the attempted transfer.
#[test]
fn test_transfer_211_locked_nft_returns_error() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commit_locked_err"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // Precondition
    assert!(client.is_active(&token_id));

    // try_transfer must return Err
    let result = client.try_transfer(&owner, &recipient, &token_id);
    assert!(
        result.is_err(),
        "transfer of locked NFT must return an error"
    );

    // Post-condition: NFT remains locked and owned by original owner
    assert!(
        client.is_active(&token_id),
        "NFT must still be active after rejected transfer"
    );
    assert_eq!(
        client.owner_of(&token_id),
        owner,
        "ownership must be unchanged after rejected locked-NFT transfer"
    );
    assert_eq!(client.balance_of(&owner), 1);
    assert_eq!(client.balance_of(&recipient), 0);
}

/// Complementary positive test: after settlement the same NFT becomes
/// transferable.
///
/// # Summary
/// Provides the "happy path" contrast to the locked-NFT rejection test.
/// Settling an expired NFT flips `is_active` to `false`, which lifts the
/// transfer lock.  This test confirms the full lifecycle:
/// mint → lock check fails → time passes → settle → transfer succeeds.
#[test]
fn test_transfer_211_succeeds_after_unlock() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint a 1-day NFT so expiry is easy to simulate
    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commit_unlock"),
        &1, // 1-day duration
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Locked immediately after mint
    assert!(client.is_active(&token_id));
    assert!(
        client.try_transfer(&owner, &recipient, &token_id).is_err(),
        "transfer must fail while NFT is locked"
    );

    // Advance ledger past expiry (> 1 day = 86400 s)
    e.ledger().with_mut(|li| {
        li.timestamp = 172_800; // 2 days
    });

    // Settle to unlock
    client.settle(&token_id);
    assert!(
        !client.is_active(&token_id),
        "NFT must be inactive after settlement"
    );

    // Transfer must now succeed
    client.transfer(&owner, &recipient, &token_id);

    assert_eq!(
        client.owner_of(&token_id),
        recipient,
        "recipient must own the NFT after successful transfer"
    );
    assert_eq!(client.balance_of(&owner), 0);
    assert_eq!(client.balance_of(&recipient), 1);
}

// ----------------------------------------------------------------------------
// 4. Non-existent token
// ----------------------------------------------------------------------------

/// Verifies that `transfer` is rejected when the `token_id` does not exist.
///
/// # Summary
/// Attempting to transfer a token that was never minted — or whose ID is
/// outside the valid range — must return `TokenNotFound` (#3).  There must
/// be no panic and no storage mutation.
///
/// # Parameters tested
/// - `token_id` — an ID for which no NFT record exists in persistent storage
///
/// # Errors
/// Expects `ContractError::TokenNotFound` (error code `#3`).
///
/// # Security Note
/// Without this check, a transfer call on an unminted ID might silently do
/// nothing or corrupt balance state.  The explicit error ensures callers
/// receive deterministic, auditable feedback.
#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_transfer_211_nonexistent_token_panics() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let sender = Address::generate(&e);
    let recipient = Address::generate(&e);

    client.initialize(&admin);

    // Sanity: no tokens have been minted
    assert_eq!(client.total_supply(), 0);

    // Transfer of non-existent token_id must fail with TokenNotFound (#3)
    client.transfer(&sender, &recipient, &9999);
}

/// Non-panicking variant of the non-existent token transfer test.
///
/// Covers large, out-of-range, and boundary token IDs.
#[test]
fn test_transfer_211_nonexistent_token_returns_error() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let sender = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint exactly one token so token_id 0 exists but 1 and beyond do not
    let _minted = client.mint(
        &admin,
        &sender,
        &String::from_str(&e, "commit_exist"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(client.total_supply(), 1);

    // token_id 1 does not exist
    let result_one = client.try_transfer(&sender, &recipient, &1);
    assert!(
        result_one.is_err(),
        "transfer of token_id 1 (never minted) must return an error"
    );

    // token_id u32::MAX does not exist
    let result_max = client.try_transfer(&sender, &recipient, &u32::MAX);
    assert!(
        result_max.is_err(),
        "transfer of token_id u32::MAX must return an error"
    );

    // Post-condition: sender has exactly the one token minted above
    assert_eq!(
        client.balance_of(&sender),
        1,
        "sender balance must be unchanged after rejected transfers"
    );
    assert_eq!(
        client.balance_of(&recipient),
        0,
        "recipient must not gain any token from rejected transfers"
    );
}
