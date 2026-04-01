#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Events},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, IntoVal, String,
};

#[contract]
struct MockNftContract;

#[contractimpl]
impl MockNftContract {
    pub fn mint(
        _e: Env,
        _caller: Address,
        _owner: Address,
        _commitment_id: String,
        _duration_days: u32,
        _max_loss_percent: u32,
        _commitment_type: String,
        _initial_amount: i128,
        _asset_address: Address,
        _early_exit_penalty: u32,
    ) -> u32 {
        1
    }

    pub fn settle(_e: Env, _caller: Address, _token_id: u32) {}

    pub fn mark_inactive(_e: Env, _caller: Address, _token_id: u32) {}
}

fn test_rules(e: &Env) -> CommitmentRules {
    CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(e, "safe"),
        early_exit_penalty: 15,
        min_fee_threshold: 100,
        grace_period_days: 0,
    }
}

fn setup_contract(e: &Env) -> (Address, CommitmentCoreContractClient<'_>, Address, Address) {
    e.mock_all_auths_allowing_non_root_auth();
    e.ledger().with_mut(|ledger| {
        ledger.timestamp = 1_700_000_000;
    });

    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(e, &contract_id);
    let admin = Address::generate(e);
    let nft_contract = e.register_contract(None, MockNftContract);

    client.initialize(&admin, &nft_contract);

    (contract_id, client, admin, nft_contract)
}

fn setup_token(e: &Env) -> (Address, StellarAssetClient<'_>, TokenClient<'_>) {
    let token_admin = Address::generate(e);
    let token_contract = e.register_stellar_asset_contract_v2(token_admin);
    let asset = token_contract.address();
    let admin_client = StellarAssetClient::new(e, &asset);
    let token_client = TokenClient::new(e, &asset);
    (asset, admin_client, token_client)
}

#[test]
fn test_emergency_mode_toggle_emits_events() {
    let e = Env::default();
    let (contract_id, client, admin, _nft_contract) = setup_contract(&e);

    assert!(!client.is_emergency_mode());

    client.set_emergency_mode(&admin, &true);
    assert!(client.is_emergency_mode());

    client.set_emergency_mode(&admin, &false);
    assert!(!client.is_emergency_mode());

    let events = e.events().all();
    let emg_mode_symbol = symbol_short!("EmgMode").into_val(&e);
    let emg_on = symbol_short!("EMG_ON").into_val(&e);
    let emg_off = symbol_short!("EMG_OFF").into_val(&e);

    let mode_events: std::vec::Vec<_> = events
        .iter()
        .filter(|event| {
            event.0 == contract_id
                && event
                    .1
                    .first()
                    .map_or(false, |topic| topic.shallow_eq(&emg_mode_symbol))
        })
        .collect();

    assert_eq!(mode_events.len(), 2);
    assert!(mode_events[0]
        .2
        .shallow_eq(&(emg_on, e.ledger().timestamp()).into_val(&e)));
    assert!(mode_events[1]
        .2
        .shallow_eq(&(emg_off, e.ledger().timestamp()).into_val(&e)));
}

#[test]
#[should_panic(expected = "Unauthorized: caller not allowed")]
fn test_set_emergency_mode_unauthorized() {
    let e = Env::default();
    let (_contract_id, client, _admin, _nft_contract) = setup_contract(&e);
    let attacker = Address::generate(&e);

    client.set_emergency_mode(&attacker, &true);
}

#[test]
fn test_create_commitment_forbidden_in_emergency_preserves_state() {
    let e = Env::default();
    let (contract_id, client, admin, _nft_contract) = setup_contract(&e);
    let owner = Address::generate(&e);
    let (asset, token_admin_client, token_client) = setup_token(&e);
    let amount = 1_000i128;

    token_admin_client.mint(&owner, &(amount * 2));
    client.set_emergency_mode(&admin, &true);

    let result = client.try_create_commitment(&owner, &amount, &asset, &test_rules(&e));

    assert!(result.is_err());
    assert!(client.is_emergency_mode());
    assert_eq!(client.get_total_commitments(), 0);
    assert_eq!(client.get_total_value_locked(), 0);
    assert_eq!(client.get_owner_commitments(&owner).len(), 0);
    assert_eq!(
        client.get_commitments_created_between(&0, &u64::MAX).len(),
        0
    );
    assert_eq!(token_client.balance(&owner), amount * 2);
    assert_eq!(token_client.balance(&contract_id), 0);
}

#[test]
#[should_panic(expected = "Action only allowed in emergency mode")]
fn test_emergency_withdraw_forbidden_in_normal_mode() {
    let e = Env::default();
    let (_contract_id, client, admin, _nft_contract) = setup_contract(&e);
    let recipient = Address::generate(&e);
    let (asset, _token_admin_client, _token_client) = setup_token(&e);

    client.emergency_withdraw(&admin, &asset, &recipient, &1_000);
}

#[test]
#[should_panic(expected = "Unauthorized: caller not allowed")]
fn test_emergency_withdraw_rejects_non_admin_in_emergency() {
    let e = Env::default();
    let (contract_id, client, admin, _nft_contract) = setup_contract(&e);
    let attacker = Address::generate(&e);
    let recipient = Address::generate(&e);
    let (asset, token_admin_client, _token_client) = setup_token(&e);

    token_admin_client.mint(&contract_id, &1_000i128);
    client.set_emergency_mode(&admin, &true);

    client.emergency_withdraw(&attacker, &asset, &recipient, &500);
}

#[test]
#[should_panic(expected = "Invalid amount")]
fn test_emergency_withdraw_rejects_non_positive_amount() {
    let e = Env::default();
    let (contract_id, client, admin, _nft_contract) = setup_contract(&e);
    let recipient = Address::generate(&e);
    let (asset, token_admin_client, _token_client) = setup_token(&e);

    token_admin_client.mint(&contract_id, &1_000i128);
    client.set_emergency_mode(&admin, &true);

    client.emergency_withdraw(&admin, &asset, &recipient, &0);
}

#[test]
fn test_emergency_withdraw_transfers_assets_in_emergency() {
    let e = Env::default();
    let (contract_id, client, admin, _nft_contract) = setup_contract(&e);
    let recipient = Address::generate(&e);
    let (asset, token_admin_client, token_client) = setup_token(&e);
    let withdraw_amount = 750i128;

    token_admin_client.mint(&contract_id, &1_500i128);
    client.set_emergency_mode(&admin, &true);

    client.emergency_withdraw(&admin, &asset, &recipient, &withdraw_amount);

    assert!(client.is_emergency_mode());
    assert_eq!(token_client.balance(&contract_id), 750);
    assert_eq!(token_client.balance(&recipient), withdraw_amount);
}
