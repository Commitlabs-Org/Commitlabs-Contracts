#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

fn setup_contract(e: &Env) -> (Address, CommitmentNFTContractClient<'_>) {
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(e, &contract_id);
    let admin = Address::generate(e);
    client.initialize(&admin);
    (admin, client)
}

fn mint_test_nft(
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
        &1,
        &10,
        &String::from_str(e, "safe"),
        &1_000,
        asset_address,
        &5,
    )
}

#[test]
fn test_initialize_sets_admin_and_zero_supply() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.total_supply(), 0);
}

#[test]
fn test_mint_and_settle_as_core_updates_supply_and_activity() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let core_contract = Address::generate(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.set_core_contract(&core_contract);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_smoke"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1_000,
        &asset_address,
        &5,
    );

    assert_eq!(client.total_supply(), 1);
    assert_eq!(client.owner_of(&token_id), owner);
    assert!(client.is_active(&token_id));

    e.ledger().with_mut(|ledger| {
        ledger.timestamp = 86_400;
    });

    client.settle(&core_contract, &token_id);
    assert!(!client.is_active(&token_id));
    assert_eq!(client.total_supply(), 1);
}

#[test]
fn test_royalty_info_default_zero_rate() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);
    let token_id = mint_test_nft(
        &e,
        &client,
        &admin,
        &owner,
        &asset_address,
        "royalty_default",
    );

    let (recipient, amount) = client.royalty_info(&token_id, &10_000);

    assert_eq!(recipient, admin);
    assert_eq!(amount, 0);
}

#[test]
fn test_set_royalty_at_cap_and_royalty_info_rounds_down() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.set_royalty(&admin, &recipient, &1_000);
    let token_id = mint_test_nft(&e, &client, &admin, &owner, &asset_address, "royalty_cap");

    let (royalty_recipient, amount) = client.royalty_info(&token_id, &12_345);

    assert_eq!(royalty_recipient, recipient);
    assert_eq!(amount, 1_234);
}

#[test]
fn test_set_royalty_above_cap_rejected() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let recipient = Address::generate(&e);

    let result = client.try_set_royalty(&admin, &recipient, &1_001);

    assert!(result.is_err());
}

#[test]
fn test_royalty_info_nonexistent_token_rejected() {
    let e = Env::default();
    let (_admin, client) = setup_contract(&e);

    let result = client.try_royalty_info(&999, &10_000);

    assert!(result.is_err());
}

#[test]
fn test_royalty_info_negative_sale_price_rejected() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);
    let token_id = mint_test_nft(
        &e,
        &client,
        &admin,
        &owner,
        &asset_address,
        "royalty_negative",
    );

    let result = client.try_royalty_info(&token_id, &-1);

    assert!(result.is_err());
}
