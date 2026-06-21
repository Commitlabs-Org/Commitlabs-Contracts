#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, String,
};

fn mint_one_day_nft(
    e: &Env,
    client: &CommitmentNFTContractClient,
    admin: &Address,
    owner: &Address,
    label: &str,
) -> u32 {
    let asset_address = Address::generate(e);

    client.mint(
        admin,
        owner,
        &String::from_str(e, label),
        &1,
        &10,
        &String::from_str(e, "safe"),
        &1_000,
        &asset_address,
        &5,
    )
}

fn setup_contract(e: &Env) -> (Address, CommitmentNFTContractClient<'_>) {
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(e, &contract_id);
    let admin = Address::generate(e);
    client.initialize(&admin);
    (admin, client)
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
fn test_transfer_rejects_active_commitment_nft() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token_id = mint_one_day_nft(&e, &client, &admin, &owner, "active_transfer_blocked");

    let result = client.try_transfer(&owner, &recipient, &token_id);

    assert!(result.is_err(), "active commitment NFT transfer must fail");
    assert_eq!(client.owner_of(&token_id), owner);
    assert_eq!(client.balance_of(&owner), 1);
    assert_eq!(client.balance_of(&recipient), 0);
    assert_eq!(client.get_nfts_by_owner(&owner).len(), 1);
    assert_eq!(client.get_nfts_by_owner(&recipient).len(), 0);
}

#[test]
fn test_transfer_rejects_non_owner_after_settlement() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let core_contract = Address::generate(&e);
    let owner = Address::generate(&e);
    let non_owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token_id = mint_one_day_nft(&e, &client, &admin, &owner, "non_owner_transfer");

    client.set_core_contract(&core_contract);
    e.ledger().with_mut(|ledger| {
        ledger.timestamp = 86_400;
    });
    client.settle(&core_contract, &token_id);

    let result = client.try_transfer(&non_owner, &recipient, &token_id);

    assert!(result.is_err(), "non-owner transfer must fail");
    assert_eq!(client.owner_of(&token_id), owner);
    assert_eq!(client.balance_of(&owner), 1);
    assert_eq!(client.balance_of(&non_owner), 0);
    assert_eq!(client.balance_of(&recipient), 0);
}

#[test]
fn test_settled_transfer_updates_owner_indexes_and_preserves_supply() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let core_contract = Address::generate(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token_id = mint_one_day_nft(&e, &client, &admin, &owner, "settled_transfer");

    client.set_core_contract(&core_contract);
    e.ledger().with_mut(|ledger| {
        ledger.timestamp = 86_400;
    });
    client.settle(&core_contract, &token_id);

    assert!(!client.is_active(&token_id));
    assert_eq!(client.total_supply(), 1);
    assert_eq!(client.owner_of(&token_id), owner);
    assert_eq!(client.balance_of(&owner), 1);
    assert_eq!(client.balance_of(&recipient), 0);

    client.transfer(&owner, &recipient, &token_id);

    assert_eq!(client.total_supply(), 1);
    assert_eq!(client.owner_of(&token_id), recipient);
    assert_eq!(client.balance_of(&owner), 0);
    assert_eq!(client.balance_of(&recipient), 1);

    let owner_nfts = client.get_nfts_by_owner(&owner);
    let recipient_nfts = client.get_nfts_by_owner(&recipient);
    assert_eq!(owner_nfts.len(), 0);
    assert_eq!(recipient_nfts.len(), 1);
    assert_eq!(recipient_nfts.get(0).unwrap().token_id, token_id);
    assert_eq!(recipient_nfts.get(0).unwrap().owner, recipient);
    assert!(!recipient_nfts.get(0).unwrap().is_active);
}

#[test]
fn test_authorized_minter_cannot_transfer_without_ownership() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let core_contract = Address::generate(&e);
    let owner = Address::generate(&e);
    let authorized_minter = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token_id = mint_one_day_nft(&e, &client, &admin, &owner, "authorized_minter_not_owner");

    client.add_authorized_contract(&admin, &authorized_minter);
    assert!(client.is_authorized(&authorized_minter));

    client.set_core_contract(&core_contract);
    e.ledger().with_mut(|ledger| {
        ledger.timestamp = 86_400;
    });
    client.settle(&core_contract, &token_id);

    let result = client.try_transfer(&authorized_minter, &recipient, &token_id);

    assert!(
        result.is_err(),
        "authorized minters are not transfer operators"
    );
    assert_eq!(client.owner_of(&token_id), owner);
    assert_eq!(client.balance_of(&owner), 1);
    assert_eq!(client.balance_of(&authorized_minter), 0);
    assert_eq!(client.balance_of(&recipient), 0);
}
