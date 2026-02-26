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
fn test_create_commitment_zero_owner_fails() {
    let env = Env::default();
    env.mock_all_auths();

    // Mapping to the specific Contract names used in this crate
    let contract_id = env.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&env, &contract_id);

    let zero_owner = generate_zero_address(&env);
    let amount: i128 = 100_000_000;
    let asset_address = Address::generate(&env);

    // Corrected field names for the Commitlabs CommitmentRules struct
    let rules = CommitmentRules {
        min_commitment_amount: 0,
        max_commitment_amount: i128::MAX,
        min_duration: 0,
        max_duration: u64::MAX,
    };

    client.create_commitment(&zero_owner, &amount, &asset_address, &rules);
}
