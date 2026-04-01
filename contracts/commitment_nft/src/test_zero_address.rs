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

    let zero_address = generate_zero_address(&env);
    let dummy_token_id = 0u32;

    // Fixed: Passing both owner and token_id
    client.mint(&zero_address, &dummy_token_id);
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
    client.mint(&sender, &token_id);

    let zero_address = generate_zero_address(&env);
    client.transfer(&sender, &zero_address, &token_id);
}
