#![cfg(test)]
extern crate std;

use crate::*;
use soroban_sdk::{Address, Env, String, testutils::Address as _};

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
    let asset_address = Address::generate(&env);
    
    client.initialize(&admin);

    // mint(caller, owner, commitment_id, duration, loss, type, amount, asset, penalty)
    client.mint(
        &admin,
        &zero_address,
        &String::from_str(&env, "commit_1"),
        &30u32,
        &10u32,
        &String::from_str(&env, "balanced"),
        &1000i128,
        &asset_address,
        &5u32
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
    let asset_address = Address::generate(&env);

    client.initialize(&admin);

    // Setup: Mint to valid sender first
    let token_id = client.mint(
        &admin,
        &sender,
        &String::from_str(&env, "commit_1"),
        &30u32,
        &10u32,
        &String::from_str(&env, "balanced"),
        &1000i128,
        &asset_address,
        &5u32
    );

    // Attempt transfer: (from, to, token_id)
    client.transfer(&sender, &zero_address, &token_id);
}
