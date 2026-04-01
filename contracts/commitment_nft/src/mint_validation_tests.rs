#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};

// Minimal setup helper to register and initialize the contract and return (admin, client)
fn setup_contract_min(e: &Env) -> (Address, CommitmentNFTContractClient<'_>) {
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(e, &contract_id);
    let admin = Address::generate(e);
    (admin, client)
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")]
fn test_mint_invalid_initial_amount_zero() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, client) = setup_contract_min(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    // Initialize and then call mint with initial_amount = 0 which should fail with InvalidAmount (#13)
    client.initialize(&admin);

    client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_000"),
        &30u32,
        &10u32,
        &String::from_str(&e, "balanced"),
        &0i128,
        &asset_address,
        &5u32,
    );
}

#[test]
fn test_mint_metadata_and_index_lookup() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, client) = setup_contract_min(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint a valid commitment
    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "ignored_id"),
        &30u32,
        &10u32,
        &String::from_str(&e, "balanced"),
        &1000i128,
        &asset_address,
        &5u32,
    );

    // token_id should be 0 for first mint
    assert_eq!(token_id, 0);

    // Check metadata stored
    let nft = client.get_metadata(&token_id);
    assert_eq!(nft.token_id, token_id);
    assert_eq!(nft.metadata.initial_amount, 1000i128);
    assert_eq!(nft.metadata.asset_address, asset_address);
    assert_eq!(nft.is_active, true);

    // Reverse lookup by auto-generated commitment_id
    let by_id = client.get_commitment_by_id(&String::from_str(&e, "COMMIT_0"));
    assert_eq!(by_id.token_id, token_id);
}

#[test]
fn test_mint_active_flag_and_owner_balance() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, client) = setup_contract_min(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_balance"),
        &30u32,
        &10u32,
        &String::from_str(&e, "balanced"),
        &500i128,
        &asset_address,
        &3u32,
    );

    // Owner balance should be 1 and NFT is active
    assert_eq!(client.balance_of(&owner), 1);
    let nft = client.get_metadata(&token_id);
    assert!(nft.is_active);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_mint_reentrancy_guard_tripped() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, client) = setup_contract_min(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Set reentrancy guard true in instance storage to simulate guard tripped
    e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

    // This mint should fail with ReentrancyDetected (#14)
    client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "reentrancy_test"),
        &30u32,
        &10u32,
        &String::from_str(&e, "balanced"),
        &1000i128,
        &asset_address,
        &5u32,
    );
}
