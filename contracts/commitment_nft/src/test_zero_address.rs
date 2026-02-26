#![cfg(test)]
extern crate std;

use crate::{CommitmentNftContract, CommitmentNftContractClient};
use soroban_sdk::{Address, Env, String};

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

    let contract_id = env.register_contract(None, CommitmentNftContract);
    let client = CommitmentNftContractClient::new(&env, &contract_id);

    let zero_address = generate_zero_address(&env);

    // Provide common dummy arguments for mint
    // If your mint function takes a token_id as well, change this to: client.mint(&zero_address, &1i128);
    client.mint(&zero_address);
}

#[test]
#[should_panic]
fn test_nft_transfer_to_zero_address_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentNftContract);
    let client = CommitmentNftContractClient::new(&env, &contract_id);

    let sender = Address::generate(&env);
    let zero_address = generate_zero_address(&env);

    client.mint(&sender);
    let token_id = 1i128; // Standard Soroban token_id type

    // Attempt to transfer the NFT to the zero address
    client.transfer(&sender, &zero_address, &token_id);
}
