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
fn test_create_commitment_zero_owner_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&env, &contract_id);

    // Initialize with a valid admin + nft_contract so the contract is ready
    let admin = Address::generate(&env);
    let nft_contract = Address::generate(&env);
    client.initialize(&admin, &nft_contract);

    let zero_owner = generate_zero_address(&env);
    let amount: i128 = 100_000_000;
    let asset_address = Address::from_string(&String::from_str(&env, "GBCRBCRBCRBCRBCRBCRBCRBCRBCRBCRBCRBCRBCRBCRBCRBCRBCRBCR"));

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&env, "safe"),
        early_exit_penalty: 15,
        min_fee_threshold: 0,
        grace_period_days: 0,
    };

    client.create_commitment(&zero_owner, &amount, &asset_address, &rules);
}
