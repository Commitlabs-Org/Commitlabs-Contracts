#![cfg(test)]
extern crate std;

use crate::*;
use soroban_sdk::{
    testutils::Address as _,
    Address, Env, String,
};

fn generate_zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(
        env,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    ))
}

#[test]
#[should_panic]
fn test_nft_mint_to_zero_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let zero_address = generate_zero_address(&env);
    let asset = Address::generate(&env);

    client.initialize(&admin);

    client.mint(
        &admin,
        &zero_address,
        &String::from_str(&env, "commitment_001"),
        &30,
        &10,
        &String::from_str(&env, "balanced"),
        &1000,
        &asset,
        &5,
    );
}

#[test]
#[should_panic]
fn test_nft_transfer_to_zero_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let zero_address = generate_zero_address(&env);
    let asset = Address::generate(&env);

    client.initialize(&admin);

    // Setup: Mint to valid sender first
    let token_id = client.mint(
        &admin,
        &sender,
        &String::from_str(&env, "commitment_001"),
        &30,
        &10,
        &String::from_str(&env, "balanced"),
        &1000,
        &asset,
        &5,
    );

    // Attempt transfer: (from, to, token_id)
    client.transfer(&sender, &zero_address, &token_id);
}
