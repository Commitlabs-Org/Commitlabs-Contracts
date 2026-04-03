#![cfg(test)]

use commitment_core::CommitmentCoreContractClient;
use commitment_core::CommitmentCoreContract;
use commitment_core::CommitmentRules;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, IntoVal, String, symbol_short};
use soroban_sdk::token::StellarAssetClient;

// Minimal mock NFT contract used by these integration tests.
#[contract]
pub struct MockNftContract;

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
        1u32
    }
}

// Integration tests placed under `tests/` compile the library without its
// #[cfg(test)] modules, avoiding failures coming from other unit-test files.

fn test_rules(e: &Env) -> CommitmentRules {
    CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(e, "balanced"),
        early_exit_penalty: 10,
        min_fee_threshold: 100,
        grace_period_days: 0,
    }
}

#[test]
fn create_rate_limit_blocks_non_exempt() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let contract_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let nft_contract = e.register_contract(None, MockNftContract);
    let admin = Address::generate(&e);
    let owner = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let amount = 1_000i128;

    let token_contract = e.register_stellar_asset_contract_v2(token_admin);
    let asset_address = token_contract.address();
    let token_admin_client = StellarAssetClient::new(&e, &asset_address);
    token_admin_client.mint(&owner, &(amount * 2));

    // Initialize and configure rate limit via client (ensures proper auth frames)
    e.as_contract(&contract_id, || {
        commitment_core::CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.set_rate_limit(&admin, &symbol_short!("create"), &60u64, &1u32);
    let rules = test_rules(&e);

    // First create allowed
    let r1 = client.try_create_commitment(&owner, &amount, &asset_address, &rules);
    assert!(r1.is_ok());

    // Second create within same window should be blocked
    let r2 = client.try_create_commitment(&owner, &amount, &asset_address, &rules);
    assert!(r2.is_err(), "Expected rate limit to block second create");
}

#[test]
fn create_rate_limit_exempt_allows_multiple() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let contract_id = e.register_contract(None, commitment_core::CommitmentCoreContract);
    let nft_contract = e.register_contract(None, MockNftContract);
    let admin = Address::generate(&e);
    let owner = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let amount = 1_000i128;

    let token_contract = e.register_stellar_asset_contract_v2(token_admin);
    let asset_address = token_contract.address();
    let token_admin_client = StellarAssetClient::new(&e, &asset_address);
    token_admin_client.mint(&owner, &(amount * 2));

    e.as_contract(&contract_id, || {
        commitment_core::CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.set_rate_limit(&admin, &symbol_short!("create"), &60u64, &1u32);
    // Set owner exempt via admin
    client.set_rate_limit_exempt(&admin, &owner, &true);
    let rules = test_rules(&e);

    let r1 = client.try_create_commitment(&owner, &amount, &asset_address, &rules);
    assert!(r1.is_ok());
    let r2 = client.try_create_commitment(&owner, &amount, &asset_address, &rules);
    assert!(r2.is_ok());

    // Ensure total commitments increased to 2
    assert_eq!(client.get_total_commitments(), 2);
}
