#![cfg(test)]
extern crate std;

use crate::{CommitmentCoreContract, CommitmentCoreContractClient, CommitmentRules};
use soroban_sdk::{Address, Env, String};

fn generate_zero_address(env: &Env) -> Address {
    // Generates a standard all-zero Stellar address
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

    let zero_owner = generate_zero_address(&env);
    let amount: i128 = 1000;
    let asset_address = Address::generate(&env);

    // NOTE: We assume `CommitmentRules` implements Default.
    // If you get a compilation error here, look at `contracts/commitment_core/src/tests.rs`
    // to see how your team normally initializes `CommitmentRules` and copy that dummy setup here.
    let rules = CommitmentRules::default();

    // Passing all 4 required arguments
    client.create_commitment(&zero_owner, &amount, &asset_address, &rules);
}
