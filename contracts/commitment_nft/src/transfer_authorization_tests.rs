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

fn setup_contract_with_core(e: &Env) -> (Address, CommitmentNFTContractClient<'_>, Address) {
    let (admin, client) = setup_contract(e);
    let core_contract = Address::generate(e);
    client.set_core_contract(&core_contract);
    (admin, client, core_contract)
}

fn mint_test_nft(
    e: &Env,
    client: &CommitmentNFTContractClient,
    caller: &Address,
    owner: &Address,
    label: &str,
) -> u32 {
    let asset_address = Address::generate(e);
    client.mint(
        caller,
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

fn assert_owner_inventory(
    client: &CommitmentNFTContractClient,
    owner: &Address,
    expected_token_ids: &[u32],
) {
    assert_eq!(client.balance_of(owner), expected_token_ids.len() as u32);

    let owner_nfts = client.get_nfts_by_owner(owner);
    assert_eq!(owner_nfts.len(), expected_token_ids.len() as u32);

    for token_id in expected_token_ids {
        assert_eq!(client.owner_of(token_id), owner.clone());
        assert!(
            owner_nfts
                .iter()
                .any(|nft| nft.token_id == *token_id && nft.owner == owner.clone()),
            "owner inventory missing token_id {}",
            token_id,
        );
    }
}

fn assert_balance_supply_invariant(client: &CommitmentNFTContractClient, owners: &[&Address]) {
    let balance_sum: u32 = owners.iter().map(|owner| client.balance_of(owner)).sum();
    assert_eq!(balance_sum, client.total_supply());
}

#[test]
fn transfer_rejects_non_owner_and_active_owner_without_mutating_inventory() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let not_owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let token_id = mint_test_nft(&e, &client, &admin, &owner, "transfer_auth_active");

    assert!(client.is_active(&token_id));
    assert_owner_inventory(&client, &owner, &[token_id]);
    assert_owner_inventory(&client, &recipient, &[]);

    let not_owner_result = client.try_transfer(&not_owner, &recipient, &token_id);
    assert!(not_owner_result.is_err());

    let active_owner_result = client.try_transfer(&owner, &recipient, &token_id);
    assert!(active_owner_result.is_err());

    assert!(client.is_active(&token_id));
    assert_owner_inventory(&client, &owner, &[token_id]);
    assert_owner_inventory(&client, &recipient, &[]);
    assert_balance_supply_invariant(&client, &[&owner, &recipient]);
}

#[test]
fn inactive_transfer_requires_current_owner_and_updates_owner_indexes() {
    let e = Env::default();
    let (admin, client, core_contract) = setup_contract_with_core(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let authorized_minter = Address::generate(&e);
    let final_recipient = Address::generate(&e);

    client
        .add_authorized_contract(&admin, &authorized_minter)
        .unwrap();
    assert!(client.is_authorized(&authorized_minter));

    let token_id = mint_test_nft(&e, &client, &admin, &owner, "transfer_auth_inactive");
    e.ledger().with_mut(|ledger| {
        ledger.timestamp = 172_800;
    });
    client.settle(&core_contract, &token_id);
    assert!(!client.is_active(&token_id));

    let authorized_minter_result = client.try_transfer(&authorized_minter, &recipient, &token_id);
    assert!(authorized_minter_result.is_err());
    assert_owner_inventory(&client, &owner, &[token_id]);
    assert_owner_inventory(&client, &recipient, &[]);

    client.transfer(&owner, &recipient, &token_id);
    assert_owner_inventory(&client, &owner, &[]);
    assert_owner_inventory(&client, &recipient, &[token_id]);

    let old_owner_result = client.try_transfer(&owner, &final_recipient, &token_id);
    assert!(old_owner_result.is_err());
    assert_owner_inventory(&client, &recipient, &[token_id]);
    assert_owner_inventory(&client, &final_recipient, &[]);
    assert_balance_supply_invariant(&client, &[&owner, &recipient, &final_recipient]);
}
