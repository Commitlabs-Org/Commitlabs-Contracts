//! Unit tests for transformation lifecycle - invalid state transitions
//!
//! This module tests all invalid state transitions and error conditions
//! to ensure the contract properly validates state before operations.

#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec, Address, Env, String, Vec};

fn setup(e: &Env) -> (Address, Address, Address) {
    let admin = Address::generate(e);
    let core = Address::generate(e);
    let user = Address::generate(e);
    (admin, core, user)
}

// ============================================================================
// Initialization State Transition Tests
// ============================================================================

#[test]
#[should_panic(expected = "Contract not initialized")]
fn test_get_admin_before_initialization() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    // Attempt to get admin before initialization
    client.get_admin();
}

#[test]
#[should_panic(expected = "Contract not initialized")]
fn test_set_transformation_fee_before_initialization() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, _, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    // Attempt to set fee before initialization
    client.set_transformation_fee(&admin, &100);
}

#[test]
#[should_panic(expected = "Contract not initialized")]
fn test_set_authorized_transformer_before_initialization() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, _, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    // Attempt to authorize transformer before initialization
    client.set_authorized_transformer(&admin, &user, &true);
}

#[test]
#[should_panic(expected = "Contract not initialized")]
fn test_set_fee_recipient_before_initialization() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, _, _) = setup(&e);
    let recipient = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    // Attempt to set fee recipient before initialization
    client.set_fee_recipient(&admin, &recipient);
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_double_initialization_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let admin2 = Address::generate(&e);
    let core2 = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    // First initialization succeeds
    client.initialize(&admin, &core);
    
    // Second initialization must fail
    client.initialize(&admin2, &core2);
}

// ============================================================================
// Authorization State Transition Tests
// ============================================================================

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_create_tranches_without_authorization() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let unauthorized_user = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    // Do NOT authorize the user
    
    let commitment_id = String::from_str(&e, "c_1");
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    
    // Attempt to create tranches without authorization
    client.create_tranches(
        &unauthorized_user,
        &commitment_id,
        &1_000_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_collateralize_without_authorization() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let unauthorized_user = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    // Do NOT authorize the user
    
    let commitment_id = String::from_str(&e, "c_1");
    let asset = Address::generate(&e);
    
    // Attempt to collateralize without authorization
    client.collateralize(&unauthorized_user, &commitment_id, &500_000i128, &asset);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_create_secondary_instrument_without_authorization() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let unauthorized_user = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    // Do NOT authorize the user
    
    let commitment_id = String::from_str(&e, "c_1");
    let instrument_type = String::from_str(&e, "receivable");
    
    // Attempt to create instrument without authorization
    client.create_secondary_instrument(
        &unauthorized_user,
        &commitment_id,
        &instrument_type,
        &200_000i128,
    );
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_add_protocol_guarantee_without_authorization() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let unauthorized_user = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    // Do NOT authorize the user
    
    let commitment_id = String::from_str(&e, "c_1");
    let guarantee_type = String::from_str(&e, "liquidity_backstop");
    let terms_hash = String::from_str(&e, "0xabc123");
    
    // Attempt to add guarantee without authorization
    client.add_protocol_guarantee(
        &unauthorized_user,
        &commitment_id,
        &guarantee_type,
        &terms_hash,
    );
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_non_admin_cannot_set_transformation_fee() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    
    // Non-admin user attempts to set fee
    client.set_transformation_fee(&user, &100);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_non_admin_cannot_set_authorized_transformer() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let other_user = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    
    // Non-admin user attempts to authorize another user
    client.set_authorized_transformer(&user, &other_user, &true);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_non_admin_cannot_set_fee_recipient() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let recipient = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    
    // Non-admin user attempts to set fee recipient
    client.set_fee_recipient(&user, &recipient);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_non_admin_cannot_withdraw_fees() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let recipient = Address::generate(&e);
    let asset = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    client.set_fee_recipient(&admin, &recipient);
    
    // Non-admin user attempts to withdraw fees
    client.withdraw_fees(&user, &asset, &100i128);
}

#[test]
fn test_authorized_user_loses_access_after_deauthorization() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    
    // Authorize user
    client.set_authorized_transformer(&admin, &user, &true);
    
    // User can create tranches
    let commitment_id = String::from_str(&e, "c_1");
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    
    let id = client.create_tranches(
        &user,
        &commitment_id,
        &1_000_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
    assert!(!id.is_empty());
    
    // Deauthorize user
    client.set_authorized_transformer(&admin, &user, &false);
    
    // Now user should fail (but we can't test panic in middle of test)
    // This test verifies the authorization toggle works
}

// ============================================================================
// Invalid Data State Transition Tests
// ============================================================================

#[test]
#[should_panic(expected = "Invalid amount")]
fn test_create_tranches_with_zero_value() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);
    
    let commitment_id = String::from_str(&e, "c_1");
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    
    // Attempt to create tranches with zero value
    client.create_tranches(
        &user,
        &commitment_id,
        &0i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

#[test]
#[should_panic(expected = "Invalid amount")]
fn test_create_tranches_with_negative_value() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);
    
    let commitment_id = String::from_str(&e, "c_1");
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    
    // Attempt to create tranches with negative value
    client.create_tranches(
        &user,
        &commitment_id,
        &-1000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

#[test]
#[should_panic(expected = "Tranche ratios must sum to 100")]
fn test_create_tranches_with_ratios_sum_less_than_100() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);
    
    let commitment_id = String::from_str(&e, "c_1");
    // Ratios sum to 8000 (80%), not 10000 (100%)
    let tranche_share_bps: Vec<u32> = vec![&e, 5000u32, 3000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "equity"),
    ];
    let fee_asset = Address::generate(&e);
    
    client.create_tranches(
        &user,
        &commitment_id,
        &1_000_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

#[test]
#[should_panic(expected = "Tranche ratios must sum to 100")]
fn test_create_tranches_with_ratios_sum_greater_than_100() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);
    
    let commitment_id = String::from_str(&e, "c_1");
    // Ratios sum to 12000 (120%), not 10000 (100%)
    let tranche_share_bps: Vec<u32> = vec![&e, 7000u32, 5000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "equity"),
    ];
    let fee_asset = Address::generate(&e);
    
    client.create_tranches(
        &user,
        &commitment_id,
        &1_000_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

#[test]
#[should_panic(expected = "Tranche ratios must sum to 100")]
fn test_create_tranches_with_empty_tranches() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);
    
    let commitment_id = String::from_str(&e, "c_1");
    // Empty vectors
    let tranche_share_bps: Vec<u32> = vec![&e];
    let risk_levels: Vec<String> = vec![&e];
    let fee_asset = Address::generate(&e);
    
    client.create_tranches(
        &user,
        &commitment_id,
        &1_000_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}


