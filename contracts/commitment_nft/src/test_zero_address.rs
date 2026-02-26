#![cfg(test)]
extern crate std;

use crate::*;
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

    // Trying the 2-argument mint: (to, token_id)
    client.mint(&zero_address, &0i128);
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
    let token_id = 1i128;

    client.mint(&sender, &token_id);

    // Standard transfer: (from, to, token_id)
    client.transfer(&sender, &zero_address, &token_id);
}
