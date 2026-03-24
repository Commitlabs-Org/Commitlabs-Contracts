//! Fee model tests for commitment_core contract
//!
//! Tests cover:
//! - Creation fee collection
//! - Early exit penalty retention
//! - Fee administration (set rates, set recipient, withdraw)
//! - Fee getters
//! - Edge cases and security

#![cfg(test)]

use crate::{CommitmentCoreContract, CommitmentCoreContractClient, CommitmentRules};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    token, Address, Env, String,
};

#[contract]
struct FeeMockNftContract;

#[contractimpl]
impl FeeMockNftContract {
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

fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let token_contract = e.register_stellar_asset_contract_v2(admin.clone());
    let addr = token_contract.address();
    (
        addr.clone(),
        token::Client::new(e, &addr),
        token::StellarAssetClient::new(e, &addr),
    )
}

fn setup_test() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    token::Client<'static>,
    CommitmentCoreContractClient<'static>,
) {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let nft_contract = e.register_contract(None, FeeMockNftContract);
    let user = Address::generate(&e);
    let (token_address, token_client, token_admin_client) = create_token_contract(&e, &admin);

    // Mint tokens to user
    token_admin_client.mint(&user, &10_000_000);

    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    client.initialize(&admin, &nft_contract);

    (e, admin, nft_contract, user, token_address, token_client, client)
}

fn default_rules(e: &Env) -> CommitmentRules {
    CommitmentRules {
        duration_days: 30,
        max_loss_percent: 20,
        commitment_type: String::from_str(e, "balanced"),
        early_exit_penalty: 10,
        min_fee_threshold: 0,
        grace_period_days: 0,
    }
}

// ============================================================================
// Creation Fee Tests
// ============================================================================

#[test]
fn test_set_creation_fee_bps() {
    let (e, admin, _, _, _, _, client) = setup_test();

    // Set creation fee to 1% (100 bps)
    client.set_creation_fee_bps(&admin, &100);

    // Verify fee was set
    assert_eq!(client.get_creation_fee_bps(), 100);
}

#[test]
#[should_panic(expected = "Invalid fee basis points")]
fn test_set_creation_fee_bps_invalid() {
    let (e, admin, _, _, _, _, client) = setup_test();

    // Try to set fee > 10000 bps (100%)
    client.set_creation_fee_bps(&admin, &10001);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_set_creation_fee_bps_unauthorized() {
    let (e, _, _, user, _, _, client) = setup_test();

    // Non-admin tries to set fee
    client.set_creation_fee_bps(&user, &100);
}

#[test]
fn test_create_commitment_with_zero_fee() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    // No fee set (defaults to 0)
    let amount = 1_000_000i128;
    let rules = default_rules(&e);

    let commitment_id = client.create_commitment(&user, &amount, &token_address, &rules);

    // Verify commitment amount is full amount (no fee deducted)
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.amount, amount);
    assert_eq!(commitment.current_value, amount);

    // Verify no fees collected
    assert_eq!(client.get_collected_fees(&token_address), 0);
}

#[test]
fn test_create_commitment_with_creation_fee() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    // Set 1% creation fee (100 bps)
    client.set_creation_fee_bps(&admin, &100);

    let amount = 1_000_000i128;
    let expected_fee = 10_000i128; // 1% of 1,000,000
    let expected_net = amount - expected_fee;
    let rules = default_rules(&e);

    let commitment_id = client.create_commitment(&user, &amount, &token_address, &rules);

    // Verify commitment amount is net amount (after fee)
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.amount, expected_net);
    assert_eq!(commitment.current_value, expected_net);

    // Verify fee was collected
    assert_eq!(client.get_collected_fees(&token_address), expected_fee);

    // Verify TVL reflects net amount
    assert_eq!(client.get_total_value_locked(), expected_net);
}

#[test]
fn test_create_commitment_with_max_fee() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    // Set 100% creation fee (10000 bps) - extreme case
    client.set_creation_fee_bps(&admin, &10000);

    let amount = 1_000_000i128;
    let expected_fee = amount; // 100%
    let expected_net = 0i128;
    let rules = default_rules(&e);

    let commitment_id = client.create_commitment(&user, &amount, &token_address, &rules);

    // Verify commitment amount is 0 (all went to fees)
    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.amount, expected_net);

    // Verify all amount was collected as fee
    assert_eq!(client.get_collected_fees(&token_address), expected_fee);
}

#[test]
fn test_create_commitment_fee_rounds_down() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    // Set 0.15% creation fee (15 bps)
    client.set_creation_fee_bps(&admin, &15);

    let amount = 100i128;
    // 100 * 15 / 10000 = 0.15 -> rounds down to 0
    let expected_fee = 0i128;
    let expected_net = amount;
    let rules = default_rules(&e);

    let commitment_id = client.create_commitment(&user, &amount, &token_address, &rules);

    let commitment = client.get_commitment(&commitment_id);
    assert_eq!(commitment.amount, expected_net);
    assert_eq!(client.get_collected_fees(&token_address), expected_fee);
}

#[test]
fn test_multiple_commitments_accumulate_fees() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    // Set 1% creation fee
    client.set_creation_fee_bps(&admin, &100);

    let amount = 1_000_000i128;
    let fee_per_commitment = 10_000i128;
    let rules = default_rules(&e);

    // Create 3 commitments
    client.create_commitment(&user, &amount, &token_address, &rules);
    client.create_commitment(&user, &amount, &token_address, &rules);
    client.create_commitment(&user, &amount, &token_address, &rules);

    // Verify fees accumulated
    assert_eq!(
        client.get_collected_fees(&token_address),
        fee_per_commitment * 3
    );
}

// ============================================================================
// Early Exit Fee Tests
// ============================================================================

#[test]
fn test_early_exit_penalty_retained_as_fee() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    let amount = 1_000_000i128;
    let mut rules = default_rules(&e);
    rules.early_exit_penalty = 10; // 10% penalty

    let commitment_id = client.create_commitment(&user, &amount, &token_address, &rules);

    // Early exit
    client.early_exit(&commitment_id, &user);

    let expected_penalty = 100_000i128; // 10% of 1,000,000
    let expected_returned = amount - expected_penalty;
    let expected_user_balance = 10_000_000i128 - amount + expected_returned;

    // Verify penalty was added to collected fees
    assert_eq!(client.get_collected_fees(&token_address), expected_penalty);

    // Verify user received net amount
    assert_eq!(token_client.balance(&user), expected_user_balance);
}

#[test]
fn test_early_exit_with_creation_fee_and_penalty() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    // Set 1% creation fee
    client.set_creation_fee_bps(&admin, &100);

    let amount = 1_000_000i128;
    let creation_fee = 10_000i128; // 1%
    let net_amount = amount - creation_fee;

    let mut rules = default_rules(&e);
    rules.early_exit_penalty = 10; // 10% penalty

    let commitment_id = client.create_commitment(&user, &amount, &token_address, &rules);

    // Early exit
    client.early_exit(&commitment_id, &user);

    let exit_penalty = 99_000i128; // 10% of 990,000
    let expected_returned = net_amount - exit_penalty;
    let total_fees = creation_fee + exit_penalty;
    let expected_user_balance = 10_000_000i128 - amount + expected_returned;

    // Verify both fees were collected
    assert_eq!(client.get_collected_fees(&token_address), total_fees);

    // Verify user received correct amount
    assert_eq!(token_client.balance(&user), expected_user_balance);
}

// ============================================================================
// Fee Recipient Tests
// ============================================================================

#[test]
fn test_set_fee_recipient() {
    let (e, admin, _, _, _, _, client) = setup_test();

    let recipient = Address::generate(&e);
    client.set_fee_recipient(&admin, &recipient);

    assert_eq!(client.get_fee_recipient(), Some(recipient));
}

#[test]
#[should_panic(expected = "Zero address")]
fn test_set_fee_recipient_zero_address() {
    let (e, admin, _, _, _, _, client) = setup_test();

    let zero_str = String::from_str(&e, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF");
    let zero_addr = Address::from_string(&zero_str);

    client.set_fee_recipient(&admin, &zero_addr);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_set_fee_recipient_unauthorized() {
    let (e, _, _, user, _, _, client) = setup_test();

    let recipient = Address::generate(&e);
    client.set_fee_recipient(&user, &recipient);
}

// ============================================================================
// Fee Withdrawal Tests
// ============================================================================

#[test]
fn test_withdraw_fees() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    let recipient = Address::generate(&e);
    client.set_fee_recipient(&admin, &recipient);

    // Set 1% creation fee and create commitment
    client.set_creation_fee_bps(&admin, &100);
    let amount = 1_000_000i128;
    let expected_fee = 10_000i128;
    let rules = default_rules(&e);

    client.create_commitment(&user, &amount, &token_address, &rules);

    // Withdraw fees
    client.withdraw_fees(&admin, &token_address, &expected_fee);

    // Verify fees were transferred to recipient
    assert_eq!(token_client.balance(&recipient), expected_fee);

    // Verify collected fees were decremented
    assert_eq!(client.get_collected_fees(&token_address), 0);
}

#[test]
fn test_withdraw_partial_fees() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    let recipient = Address::generate(&e);
    client.set_fee_recipient(&admin, &recipient);

    // Collect fees
    client.set_creation_fee_bps(&admin, &100);
    let amount = 1_000_000i128;
    let total_fee = 10_000i128;
    let rules = default_rules(&e);

    client.create_commitment(&user, &amount, &token_address, &rules);

    // Withdraw partial amount
    let withdraw_amount = 5_000i128;
    client.withdraw_fees(&admin, &token_address, &withdraw_amount);

    // Verify partial withdrawal
    assert_eq!(token_client.balance(&recipient), withdraw_amount);
    assert_eq!(
        client.get_collected_fees(&token_address),
        total_fee - withdraw_amount
    );
}

#[test]
#[should_panic(expected = "Fee recipient not set")]
fn test_withdraw_fees_no_recipient() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    // Collect fees but don't set recipient
    client.set_creation_fee_bps(&admin, &100);
    let rules = default_rules(&e);
    client.create_commitment(&user, &1_000_000, &token_address, &rules);

    // Try to withdraw without setting recipient
    client.withdraw_fees(&admin, &token_address, &10_000);
}

#[test]
#[should_panic(expected = "Insufficient collected fees")]
fn test_withdraw_fees_insufficient() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    let recipient = Address::generate(&e);
    client.set_fee_recipient(&admin, &recipient);

    // Collect small fee
    client.set_creation_fee_bps(&admin, &100);
    let rules = default_rules(&e);
    client.create_commitment(&user, &1_000_000, &token_address, &rules);

    // Try to withdraw more than collected
    client.withdraw_fees(&admin, &token_address, &20_000);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_withdraw_fees_unauthorized() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    let recipient = Address::generate(&e);
    client.set_fee_recipient(&admin, &recipient);

    // Collect fees
    client.set_creation_fee_bps(&admin, &100);
    let rules = default_rules(&e);
    client.create_commitment(&user, &1_000_000, &token_address, &rules);

    // Non-admin tries to withdraw
    client.withdraw_fees(&user, &token_address, &10_000);
}

#[test]
#[should_panic(expected = "Invalid amount")]
fn test_withdraw_fees_zero_amount() {
    let (e, admin, _, _, token_address, _, client) = setup_test();

    let recipient = Address::generate(&e);
    client.set_fee_recipient(&admin, &recipient);

    // Try to withdraw zero
    client.withdraw_fees(&admin, &token_address, &0);
}

// ============================================================================
// Getter Tests
// ============================================================================

#[test]
fn test_get_creation_fee_bps_default() {
    let (e, _, _, _, _, _, client) = setup_test();

    // Default should be 0
    assert_eq!(client.get_creation_fee_bps(), 0);
}

#[test]
fn test_get_fee_recipient_default() {
    let (e, _, _, _, _, _, client) = setup_test();

    // Default should be None
    assert_eq!(client.get_fee_recipient(), None);
}

#[test]
fn test_get_collected_fees_default() {
    let (e, _, _, _, token_address, _, client) = setup_test();

    // Default should be 0
    assert_eq!(client.get_collected_fees(&token_address), 0);
}

#[test]
fn test_get_collected_fees_multiple_assets() {
    let (e, admin, _, user, _, _, client) = setup_test();

    // Create two different tokens
    let (token1, _token1_client, token1_admin_client) = create_token_contract(&e, &admin);
    let (token2, _token2_client, token2_admin_client) = create_token_contract(&e, &admin);

    token1_admin_client.mint(&user, &10_000_000);
    token2_admin_client.mint(&user, &10_000_000);

    // Set creation fee
    client.set_creation_fee_bps(&admin, &100);

    let rules = default_rules(&e);

    // Create commitments with different assets
    client.create_commitment(&user, &1_000_000, &token1, &rules);
    client.create_commitment(&user, &2_000_000, &token2, &rules);

    // Verify fees tracked separately per asset
    assert_eq!(client.get_collected_fees(&token1), 10_000); // 1% of 1M
    assert_eq!(client.get_collected_fees(&token2), 20_000); // 1% of 2M
}

// ============================================================================
// Edge Cases and Integration Tests
// ============================================================================

#[test]
fn test_fee_collection_with_settle() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    // Set creation fee
    client.set_creation_fee_bps(&admin, &100);

    let amount = 1_000_000i128;
    let creation_fee = 10_000i128;
    let net_amount = amount - creation_fee;
    let expected_user_balance = 10_000_000i128 - amount + net_amount;

    let rules = default_rules(&e);

    let commitment_id = client.create_commitment(&user, &amount, &token_address, &rules);

    // Settle commitment
    e.ledger()
        .with_mut(|li| li.timestamp = li.timestamp + (rules.duration_days as u64 * 86_400) + 1);
    client.settle(&commitment_id);

    // Verify creation fee still collected
    assert_eq!(client.get_collected_fees(&token_address), creation_fee);

    // Verify user got back net amount
    assert_eq!(token_client.balance(&user), expected_user_balance);
}

#[test]
fn test_complete_fee_lifecycle() {
    let (e, admin, _, user, token_address, token_client, client) = setup_test();

    let recipient = Address::generate(&e);

    // 1. Configure fees
    client.set_creation_fee_bps(&admin, &100); // 1%
    client.set_fee_recipient(&admin, &recipient);

    // 2. Create commitment with creation fee
    let amount = 1_000_000i128;
    let creation_fee = 10_000i128;
    let mut rules = default_rules(&e);
    rules.early_exit_penalty = 10; // 10%

    let commitment_id = client.create_commitment(&user, &amount, &token_address, &rules);

    // 3. Early exit with penalty
    client.early_exit(&commitment_id, &user);

    let net_amount = amount - creation_fee;
    let exit_penalty = 99_000i128; // 10% of 990,000
    let total_fees = creation_fee + exit_penalty;

    // 4. Verify total fees collected
    assert_eq!(client.get_collected_fees(&token_address), total_fees);

    // 5. Withdraw fees
    client.withdraw_fees(&admin, &token_address, &total_fees);

    // 6. Verify recipient received fees
    assert_eq!(token_client.balance(&recipient), total_fees);

    // 7. Verify no fees remaining
    assert_eq!(client.get_collected_fees(&token_address), 0);
}
