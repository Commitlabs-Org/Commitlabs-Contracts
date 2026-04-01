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
