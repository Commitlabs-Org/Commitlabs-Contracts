#![cfg(test)]

use crate::*;
use soroban_sdk::{Address as SdkAddress, Env, String};

fn generate_zero_address(env: &Env) -> SdkAddress {
    SdkAddress::from_string(&String::from_str(
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
    let dummy_token_id = 0u32;

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

    let sender = SdkAddress::generate(&env);
    let zero_address = generate_zero_address(&env);
    let token_id = 1u32;

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

    let zero_address = generate_zero_address(&env);
    client.transfer(&sender, &zero_address, &token_id);
}
