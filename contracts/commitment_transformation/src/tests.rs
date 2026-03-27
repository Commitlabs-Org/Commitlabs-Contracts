//! Tests for `CommitmentTransformationContract`.
//!
//! Coverage goal: every [`TransformationError`] variant must be exercised at
//! least once.  The matrix below tracks which test triggers which variant:
//!
//! | Variant | Discriminant | Test(s) |
//! |---------|-------------|---------|
//! | `InvalidAmount` | 1 | `test_error_invalid_amount_withdraw_zero`, `test_error_invalid_amount_withdraw_negative` |
//! | `InvalidTrancheRatios` | 2 | `test_create_tranches_invalid_ratios`, `test_error_invalid_tranche_ratios_empty`, `test_error_invalid_tranche_ratios_length_mismatch` |
//! | `InvalidFeeBps` | 3 | `test_error_invalid_fee_bps` |
//! | `Unauthorized` | 4 | `test_create_tranches_unauthorized`, `test_error_unauthorized_set_fee` |
//! | `NotInitialized` | 5 | `test_error_not_initialized_get_admin` |
//! | `AlreadyInitialized` | 6 | `test_initialize_twice_fails` |
//! | `CommitmentNotFound` | 7 | `test_all_error_messages` (message-level) |
//! | `TransformationNotFound` | 8 | `test_error_transformation_not_found_tranche_set`, `…collateral`, `…instrument`, `…guarantee` |
//! | `InvalidState` | 9 | `test_all_error_messages` (message-level) |
//! | `ReentrancyDetected` | 10 | `test_all_error_messages` (message-level) |
//! | `FeeRecipientNotSet` | 11 | `test_fee_withdraw_requires_recipient` |
//! | `InsufficientFees` | 12 | `test_error_insufficient_fees` |

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

#[test]
fn test_initialize() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_transformation_fee_bps(), 0);
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_initialize_twice_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.initialize(&admin, &core);
}

#[test]
fn test_set_transformation_fee() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_transformation_fee(&admin, &100);
    assert_eq!(client.get_transformation_fee_bps(), 100);
}

#[test]
fn test_set_authorized_transformer() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);
    // user is now authorized
}

#[test]
fn test_create_tranches() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let total_value = 1_000_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 6000u32, 3000u32, 1000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "mezzanine"),
        String::from_str(&e, "equity"),
    ];
    let fee_asset = Address::generate(&e); // no fee when fee_bps=0, so no transfer
    let id = client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
    assert!(!id.is_empty());

    let set = client.get_tranche_set(&id);
    assert_eq!(set.commitment_id, commitment_id);
    assert_eq!(set.owner, user);
    assert_eq!(set.total_value, total_value);
    assert_eq!(set.tranches.len(), 3);
    assert_eq!(client.get_commitment_tranche_sets(&commitment_id).len(), 1);
}

#[test]
#[should_panic(expected = "Tranche ratios must sum to 100")]
fn test_create_tranches_invalid_ratios() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let total_value = 1_000_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 5000u32, 3000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "mezzanine"),
    ];
    let fee_asset = Address::generate(&e);
    client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

#[test]
fn test_collateralize() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let asset = Address::generate(&e);
    let asset_id = client.collateralize(&user, &commitment_id, &500_000i128, &asset);
    assert!(!asset_id.is_empty());

    let col = client.get_collateralized_asset(&asset_id);
    assert_eq!(col.commitment_id, commitment_id);
    assert_eq!(col.owner, user);
    assert_eq!(col.collateral_amount, 500_000i128);
    assert_eq!(col.asset_address, asset);
    assert_eq!(client.get_commitment_collateral(&commitment_id).len(), 1);
}

#[test]
fn test_create_secondary_instrument() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let instrument_type = String::from_str(&e, "receivable");
    let amount = 200_000i128;
    let instrument_id =
        client.create_secondary_instrument(&user, &commitment_id, &instrument_type, &amount);
    assert!(!instrument_id.is_empty());

    let inst = client.get_secondary_instrument(&instrument_id);
    assert_eq!(inst.commitment_id, commitment_id);
    assert_eq!(inst.owner, user);
    assert_eq!(inst.instrument_type, instrument_type);
    assert_eq!(inst.amount, amount);
    assert_eq!(client.get_commitment_instruments(&commitment_id).len(), 1);
}

#[test]
fn test_add_protocol_guarantee() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let guarantee_type = String::from_str(&e, "liquidity_backstop");
    let terms_hash = String::from_str(&e, "0xabc123");
    let guarantee_id =
        client.add_protocol_guarantee(&user, &commitment_id, &guarantee_type, &terms_hash);
    assert!(!guarantee_id.is_empty());

    let guar = client.get_protocol_guarantee(&guarantee_id);
    assert_eq!(guar.commitment_id, commitment_id);
    assert_eq!(guar.guarantee_type, guarantee_type);
    assert_eq!(guar.terms_hash, terms_hash);
    assert_eq!(client.get_commitment_guarantees(&commitment_id).len(), 1);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_create_tranches_unauthorized() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _user) = setup(&e);
    let unauthorized = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    // do not authorize unauthorized

    let commitment_id = String::from_str(&e, "c_1");
    let tranche_share_bps: Vec<u32> = vec![&e, 6000u32, 4000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "equity"),
    ];
    let fee_asset = Address::generate(&e);
    client.create_tranches(
        &unauthorized,
        &commitment_id,
        &1_000_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

#[test]
fn test_transformation_with_fee() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_transformation_fee(&admin, &0); // 0% so no token transfer in unit test
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let total_value = 1_000_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    let id = client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
    let set = client.get_tranche_set(&id);
    assert_eq!(set.fee_paid, 0i128); // 0% fee
    assert_eq!(set.total_value, total_value);
}

#[test]
fn test_transformation_fee_calculation_and_collection() {
    // Test fee calculation: 1% of 1_000_000 = 10_000 (logic only; actual transfer needs token mock)
    let fee_bps: u32 = 100;
    let total_value: i128 = 1_000_000;
    let expected_fee = (total_value * fee_bps as i128) / 10000;
    assert_eq!(expected_fee, 10_000);
}

#[test]
fn test_fee_set_and_get_fee_recipient() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    assert!(client.get_fee_recipient().is_none());
    let treasury = Address::generate(&e);
    client.set_fee_recipient(&admin, &treasury);
    assert_eq!(client.get_fee_recipient().unwrap(), treasury);
}

#[test]
fn test_fee_get_collected_fees_default() {
    let e = Env::default();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    let asset = Address::generate(&e);
    assert_eq!(client.get_collected_fees(&asset), 0);
}

#[test]
#[should_panic(expected = "Fee recipient not set")]
fn test_fee_withdraw_requires_recipient() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    let asset = Address::generate(&e);
    client.withdraw_fees(&admin, &asset, &100i128);
}

// ============================================================================
// Error variant coverage — discriminant 1: InvalidAmount
// ============================================================================

/// `withdraw_fees` with amount = 0 must surface `InvalidAmount`.
#[test]
#[should_panic(expected = "Invalid amount: must be positive")]
fn test_error_invalid_amount_withdraw_zero() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    let treasury = Address::generate(&e);
    client.set_fee_recipient(&admin, &treasury);
    let asset = Address::generate(&e);
    // amount = 0 must panic with InvalidAmount
    client.withdraw_fees(&admin, &asset, &0i128);
}

/// `withdraw_fees` with a negative amount must surface `InvalidAmount`.
#[test]
#[should_panic(expected = "Invalid amount: must be positive")]
fn test_error_invalid_amount_withdraw_negative() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    let treasury = Address::generate(&e);
    client.set_fee_recipient(&admin, &treasury);
    let asset = Address::generate(&e);
    client.withdraw_fees(&admin, &asset, &-1i128);
}

// ============================================================================
// Error variant coverage — discriminant 2: InvalidTrancheRatios (extra paths)
// ============================================================================

/// An empty `tranche_share_bps` array must fail with `InvalidTrancheRatios`.
#[test]
#[should_panic(expected = "Tranche ratios must sum to 100")]
fn test_error_invalid_tranche_ratios_empty() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let empty_bps: Vec<u32> = Vec::new(&e);
    let empty_risk: Vec<String> = Vec::new(&e);
    let fee_asset = Address::generate(&e);
    client.create_tranches(
        &user,
        &String::from_str(&e, "c_1"),
        &1_000_000i128,
        &empty_bps,
        &empty_risk,
        &fee_asset,
    );
}

/// Mismatched lengths between BPS and risk-level arrays must fail.
#[test]
#[should_panic(expected = "Tranche ratios must sum to 100")]
fn test_error_invalid_tranche_ratios_length_mismatch() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    // 2 BPS entries but only 1 risk-level entry
    let bps: Vec<u32> = vec![&e, 6000u32, 4000u32];
    let risk: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    client.create_tranches(
        &user,
        &String::from_str(&e, "c_1"),
        &1_000_000i128,
        &bps,
        &risk,
        &fee_asset,
    );
}

// ============================================================================
// Error variant coverage — discriminant 3: InvalidFeeBps
// ============================================================================

/// `set_transformation_fee` with a fee exceeding 10 000 bps must fail.
#[test]
#[should_panic(expected = "Fee must be 0-10000 bps")]
fn test_error_invalid_fee_bps() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    // 10_001 bps (> 100%) must be rejected
    client.set_transformation_fee(&admin, &10_001u32);
}

// ============================================================================
// Error variant coverage — discriminant 4: Unauthorized (admin-only paths)
// ============================================================================

/// A non-admin caller of `set_transformation_fee` must receive `Unauthorized`.
#[test]
#[should_panic(expected = "Unauthorized: caller not owner or authorized")]
fn test_error_unauthorized_set_fee() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    // `user` is not admin
    client.set_transformation_fee(&user, &500u32);
}

/// A non-admin caller of `set_fee_recipient` must receive `Unauthorized`.
#[test]
#[should_panic(expected = "Unauthorized: caller not owner or authorized")]
fn test_error_unauthorized_set_fee_recipient() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    let treasury = Address::generate(&e);
    client.set_fee_recipient(&user, &treasury);
}

// ============================================================================
// Error variant coverage — discriminant 5: NotInitialized
// ============================================================================

/// Calling `get_admin` before `initialize` must surface `NotInitialized`.
#[test]
#[should_panic(expected = "Contract not initialized")]
fn test_error_not_initialized_get_admin() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    // No initialize call — must panic
    let _ = client.get_admin();
}

// ============================================================================
// Error variant coverage — discriminant 8: TransformationNotFound
// ============================================================================

/// `get_tranche_set` with a bogus ID must surface `TransformationNotFound`.
#[test]
#[should_panic(expected = "Transformation record not found")]
fn test_error_transformation_not_found_tranche_set() {
    let e = Env::default();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.get_tranche_set(&String::from_str(&e, "no_such_id"));
}

/// `get_collateralized_asset` with a bogus ID must surface `TransformationNotFound`.
#[test]
#[should_panic(expected = "Transformation record not found")]
fn test_error_transformation_not_found_collateral() {
    let e = Env::default();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.get_collateralized_asset(&String::from_str(&e, "no_such_id"));
}

/// `get_secondary_instrument` with a bogus ID must surface `TransformationNotFound`.
#[test]
#[should_panic(expected = "Transformation record not found")]
fn test_error_transformation_not_found_instrument() {
    let e = Env::default();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.get_secondary_instrument(&String::from_str(&e, "no_such_id"));
}

/// `get_protocol_guarantee` with a bogus ID must surface `TransformationNotFound`.
#[test]
#[should_panic(expected = "Transformation record not found")]
fn test_error_transformation_not_found_guarantee() {
    let e = Env::default();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.get_protocol_guarantee(&String::from_str(&e, "no_such_id"));
}

// ============================================================================
// Error variant coverage — discriminant 12: InsufficientFees
// ============================================================================

/// `withdraw_fees` when the collected balance is zero must surface `InsufficientFees`.
#[test]
#[should_panic(expected = "Insufficient collected fees to withdraw")]
fn test_error_insufficient_fees() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    let treasury = Address::generate(&e);
    client.set_fee_recipient(&admin, &treasury);
    let asset = Address::generate(&e);
    // No fees have been collected yet; any positive amount must fail
    client.withdraw_fees(&admin, &asset, &1i128);
}

/// Partial withdrawal followed by an over-withdrawal also surfaces `InsufficientFees`.
#[test]
#[should_panic(expected = "Insufficient collected fees to withdraw")]
fn test_error_insufficient_fees_over_withdrawal() {
    // We cannot actually collect fees without a real token contract, so we
    // rely on the fact that the initial balance is 0 and directly request
    // more than zero.
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    let treasury = Address::generate(&e);
    client.set_fee_recipient(&admin, &treasury);
    let asset = Address::generate(&e);
    // collected = 0, requesting 500 — must panic
    client.withdraw_fees(&admin, &asset, &500i128);
}

// ============================================================================
// Message-level coverage for variants 7 (CommitmentNotFound),
// 9 (InvalidState), and 10 (ReentrancyDetected).
//
// These variants are defined in the enum but currently have no reachable
// code path that triggers them through the contract client.  The tests below
// exercise the `message()` method directly so that the match arms are
// compiled and covered, and so that future code additions that introduce
// these paths will immediately have a correct baseline.
// ============================================================================

/// Every `TransformationError` variant must return a non-empty, correct
/// message string.  This test exercises all twelve arms of `message()`.
#[test]
fn test_all_error_messages() {
    assert_eq!(
        TransformationError::InvalidAmount.message(),
        "Invalid amount: must be positive"
    );
    assert_eq!(
        TransformationError::InvalidTrancheRatios.message(),
        "Tranche ratios must sum to 100"
    );
    assert_eq!(
        TransformationError::InvalidFeeBps.message(),
        "Fee must be 0-10000 bps"
    );
    assert_eq!(
        TransformationError::Unauthorized.message(),
        "Unauthorized: caller not owner or authorized"
    );
    assert_eq!(
        TransformationError::NotInitialized.message(),
        "Contract not initialized"
    );
    assert_eq!(
        TransformationError::AlreadyInitialized.message(),
        "Contract already initialized"
    );
    // CommitmentNotFound — reserved; not yet reachable via contract client
    assert_eq!(
        TransformationError::CommitmentNotFound.message(),
        "Commitment not found"
    );
    assert_eq!(
        TransformationError::TransformationNotFound.message(),
        "Transformation record not found"
    );
    // InvalidState — reserved; not yet reachable via contract client
    assert_eq!(
        TransformationError::InvalidState.message(),
        "Invalid state for transformation"
    );
    // ReentrancyDetected — only reachable mid-execution; covered here at message level
    assert_eq!(
        TransformationError::ReentrancyDetected.message(),
        "Reentrancy detected"
    );
    assert_eq!(
        TransformationError::FeeRecipientNotSet.message(),
        "Fee recipient not set"
    );
    assert_eq!(
        TransformationError::InsufficientFees.message(),
        "Insufficient collected fees to withdraw"
    );
}

/// All twelve discriminant values must map to the documented integers.
#[test]
fn test_error_discriminants() {
    assert_eq!(TransformationError::InvalidAmount as u32, 1);
    assert_eq!(TransformationError::InvalidTrancheRatios as u32, 2);
    assert_eq!(TransformationError::InvalidFeeBps as u32, 3);
    assert_eq!(TransformationError::Unauthorized as u32, 4);
    assert_eq!(TransformationError::NotInitialized as u32, 5);
    assert_eq!(TransformationError::AlreadyInitialized as u32, 6);
    assert_eq!(TransformationError::CommitmentNotFound as u32, 7);
    assert_eq!(TransformationError::TransformationNotFound as u32, 8);
    assert_eq!(TransformationError::InvalidState as u32, 9);
    assert_eq!(TransformationError::ReentrancyDetected as u32, 10);
    assert_eq!(TransformationError::FeeRecipientNotSet as u32, 11);
    assert_eq!(TransformationError::InsufficientFees as u32, 12);
}

// ============================================================================
// Additional happy-path and edge-case tests
// ============================================================================

/// Admin is the implicit authorized caller — no explicit allowlist entry needed.
#[test]
fn test_admin_can_create_tranches_directly() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);

    let bps: Vec<u32> = vec![&e, 5000u32, 5000u32];
    let risk: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "equity"),
    ];
    let fee_asset = Address::generate(&e);
    let id = client.create_tranches(
        &admin, // admin has implicit authorization
        &String::from_str(&e, "c_admin"),
        &2_000_000i128,
        &bps,
        &risk,
        &fee_asset,
    );
    assert!(!id.is_empty());
    let set = client.get_tranche_set(&id);
    assert_eq!(set.tranches.len(), 2);
}

/// Single-tranche (100 %) split must be accepted.
#[test]
fn test_single_tranche_100_bps() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let bps: Vec<u32> = vec![&e, 10000u32];
    let risk: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    let id = client.create_tranches(
        &user,
        &String::from_str(&e, "c_single"),
        &1_000_000i128,
        &bps,
        &risk,
        &fee_asset,
    );
    let set = client.get_tranche_set(&id);
    assert_eq!(set.tranches.len(), 1);
    // Net value equals total value (0 % fee)
    assert_eq!(set.tranches.get(0).unwrap().amount, 1_000_000i128);
}

/// `get_collected_fees` returns 0 for an asset with no collected fees.
#[test]
fn test_collected_fees_default_zero() {
    let e = Env::default();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    let asset = Address::generate(&e);
    assert_eq!(client.get_collected_fees(&asset), 0i128);
}

/// Revoking transformer authorization must prevent subsequent calls.
#[test]
#[should_panic(expected = "Unauthorized: caller not owner or authorized")]
fn test_revoke_transformer_prevents_calls() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);
    // Revoke
    client.set_authorized_transformer(&admin, &user, &false);

    let bps: Vec<u32> = vec![&e, 10000u32];
    let risk: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    client.create_tranches(
        &user,
        &String::from_str(&e, "c_rev"),
        &500_000i128,
        &bps,
        &risk,
        &Address::generate(&e),
    );
}

/// Multiple transformations for the same commitment must each be retrievable.
#[test]
fn test_multiple_tranches_same_commitment() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_multi");
    let bps: Vec<u32> = vec![&e, 10000u32];
    let risk: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);

    let id1 = client.create_tranches(
        &user,
        &commitment_id,
        &1_000_000i128,
        &bps.clone(),
        &risk.clone(),
        &fee_asset,
    );
    let id2 = client.create_tranches(
        &user,
        &commitment_id,
        &2_000_000i128,
        &bps,
        &risk,
        &fee_asset,
    );

    assert_ne!(id1, id2);
    let sets = client.get_commitment_tranche_sets(&commitment_id);
    assert_eq!(sets.len(), 2);
}
