#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_emergency_mode_toggle() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);

    client.initialize(&admin, &nft_contract);

    assert!(!client.is_emergency_mode());

    // Toggle ON
    client.set_emergency_mode(&admin, &true);
    assert!(client.is_emergency_mode());

    // Toggle OFF
    client.set_emergency_mode(&admin, &false);
    assert!(!client.is_emergency_mode());
}

#[test]
#[should_panic(expected = "Action not allowed in emergency mode")]
fn test_create_commitment_forbidden_in_emergency() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin, &nft_contract);
    client.set_emergency_mode(&admin, &true);

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&e, "safe"),
        early_exit_penalty: 5,
        min_fee_threshold: 100,
    };

    // This should panic because of emergency mode
    client.create_commitment(&owner, &1000, &asset, &rules);
}

#[test]
#[should_panic(expected = "Action only allowed in emergency mode")]
fn test_emergency_withdraw_forbidden_in_normal_mode() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let to = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin, &nft_contract);

    // Normal mode, should panic
    client.emergency_withdraw(&admin, &asset, &to, &1000);
}

#[test]
#[should_panic(expected = "Unauthorized: caller not allowed")]
fn test_set_emergency_mode_unauthorized() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let attacker = Address::generate(&e);

    client.initialize(&admin, &nft_contract);

    // Using attacker address should fail the require_admin check
    client.set_emergency_mode(&attacker, &true);
}
