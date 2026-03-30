#![cfg(test)]
extern crate std;

use crate::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn generate_zero_address(env: &Env) -> Address {
    Address::from_string(&String::from_str(
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
    client.initialize(&admin);

    let zero_address = generate_zero_address(&env);
    
    client.mint(
        &admin, // caller (must be admin)
        &zero_address, // owner
        &String::from_str(&env, "commitment"),
        &30u32,        // duration_days
        &10u32,        // max_loss_percent
        &String::from_str(&env, "safe"), // commitment_type
        &1000i128,     // initial_amount 
        &zero_address, // asset_address
        &5u32          // early_exit_penalty
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")]
fn test_nft_transfer_to_zero_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&env, &contract_id);

    let sender = Address::generate(&env);
    let zero_address = generate_zero_address(&env);
    let token_id = 1u32;
    let asset = Address::generate(&env);

    client.initialize(&sender);

    // Setup: Mint to valid sender first
    client.mint(
        &sender, // caller
        &sender, // owner
        &String::from_str(&env, "commitment"),
        &30u32,        // duration_days
        &10u32,        // max_loss_percent
        &String::from_str(&env, "safe"), // commitment_type
        &1000i128,     // initial_amount 
        &asset,        // asset_address
        &5u32          // early_exit_penalty
    );

    // Attempt transfer: (from, to, token_id)
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
