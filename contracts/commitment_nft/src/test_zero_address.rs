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
#[should_panic(expected = "Error(Contract, #18)")]
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
#[should_panic(expected = "Error(Contract, #18)")]
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

#[test]
#[should_panic(expected = "Error(Contract, #22)")]
fn test_initialize_with_zero_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);

    let zero_address = generate_zero_address(&env);
    client.initialize(&zero_address);
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")]
fn test_set_admin_with_zero_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let zero_address = generate_zero_address(&env);
    client.set_admin(&admin, &zero_address);
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")]
fn test_set_core_contract_with_zero_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);
    
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let zero_address = generate_zero_address(&env);
    client.set_core_contract(&zero_address);
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")]
fn test_add_authorized_contract_zero_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let zero_address = generate_zero_address(&env);
    client.add_authorized_contract(&admin, &zero_address);
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")]
fn test_nft_mint_with_zero_asset_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let owner = Address::generate(&env);
    let zero_address = generate_zero_address(&env);

    client.mint(
        &admin,
        &owner,
        &String::from_str(&env, "COMMIT_TEST"),
        &30u32,
        &10u32,
        &String::from_str(&env, "safe"),
        &1000i128,
        &zero_address,
        &0u32,
    );
}
