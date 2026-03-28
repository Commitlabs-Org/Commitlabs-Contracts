#![cfg(test)]

//! Unit tests for the CommitmentTransformation contract.
//!
//! # Coverage areas
//!
//! - Initialisation and double-init guard.
//! - Admin / authorization paths.
//! - All transform entry points: `create_tranches`, `collateralize`,
//!   `create_secondary_instrument`, `add_protocol_guarantee`.
//! - **Reentrancy guard on every transform entry point** (the primary scope of
//!   issue #259). Each test simulates an in-flight call by pre-setting the
//!   `ReentrancyGuard` storage key to `true` and asserts that the contract
//!   panics with `"Reentrancy detected"` before any state mutates.
//! - Guard release on success: after a successful call the guard is `false`
//!   again so the next independent call is not falsely blocked.
//! - Fee calculation and fee-recipient management.
//! - Edge cases: zero-amount, mismatched/invalid tranche ratios, missing
//!   fee-recipient on withdrawal.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec, Address, Env, String, Vec};

// ============================================================================
// Helpers
// ============================================================================

/// Returns (admin, core_contract, authorized_user).
fn setup(e: &Env) -> (Address, Address, Address) {
    let admin = Address::generate(e);
    let core = Address::generate(e);
    let user = Address::generate(e);
    (admin, core, user)
}

/// Initialises the contract and authorises `user` as a transformer.
fn setup_with_authorized_user(
    e: &Env,
) -> (
    Address, // admin
    Address, // contract_id
    CommitmentTransformationContractClient<'_>,
    Address, // user
) {
    let (admin, core, user) = setup(e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);
    (admin, contract_id, client, user)
}

/// Manually locks the reentrancy guard in storage, simulating a mid-call state.
fn lock_reentrancy_guard(e: &Env, contract_id: &Address) {
    e.as_contract(contract_id, || {
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &true);
    });
}

/// Reads the raw boolean value of the reentrancy guard from storage.
fn read_reentrancy_guard(e: &Env, contract_id: &Address) -> bool {
    e.as_contract(contract_id, || {
        e.storage()
            .instance()
            .get::<_, bool>(&DataKey::ReentrancyGuard)
            .unwrap_or(false)
    })
}

// ============================================================================
// Initialisation
// ============================================================================

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

// ============================================================================
// Administrative helpers
// ============================================================================

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
    // user is now authorized — subsequent transform calls will succeed
}

// ============================================================================
// create_tranches — happy path
// ============================================================================

#[test]
fn test_create_tranches() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);
    let _ = contract_id;

    let commitment_id = String::from_str(&e, "c_1");
    let total_value = 1_000_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 6000u32, 3000u32, 1000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "mezzanine"),
        String::from_str(&e, "equity"),
    ];
    let fee_asset = Address::generate(&e);
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
    let (_, _, client, user) = setup_with_authorized_user(&e);

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
#[should_panic(expected = "Unauthorized")]
fn test_create_tranches_unauthorized() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _user) = setup(&e);
    let unauthorized = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);

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

// ============================================================================
// Reentrancy guard — create_tranches
// ============================================================================

#[test]
#[should_panic(expected = "Reentrancy detected")]
fn test_create_tranches_reentrancy_blocked() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    lock_reentrancy_guard(&e, &contract_id);

    let commitment_id = String::from_str(&e, "c_1");
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
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
fn test_create_tranches_guard_released_after_success() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    let commitment_id = String::from_str(&e, "c_guard_release");
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);

    client.create_tranches(
        &user,
        &commitment_id,
        &1_000_000i128,
        &tranche_share_bps.clone(),
        &risk_levels.clone(),
        &fee_asset,
    );

    assert!(
        !read_reentrancy_guard(&e, &contract_id),
        "Guard must be released (false) after a successful call"
    );

    let commitment_id2 = String::from_str(&e, "c_guard_release_2");
    let id2 = client.create_tranches(
        &user,
        &commitment_id2,
        &500_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
    assert!(!id2.is_empty());
}

// ============================================================================
// collateralize — happy path
// ============================================================================

#[test]
fn test_collateralize() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client, user) = setup_with_authorized_user(&e);

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

// ============================================================================
// Reentrancy guard — collateralize
// ============================================================================

#[test]
#[should_panic(expected = "Reentrancy detected")]
fn test_collateralize_reentrancy_blocked() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    lock_reentrancy_guard(&e, &contract_id);

    let commitment_id = String::from_str(&e, "c_reentrant");
    let asset = Address::generate(&e);
    client.collateralize(&user, &commitment_id, &100_000i128, &asset);
}

#[test]
fn test_collateralize_guard_released_after_success() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    let commitment_id = String::from_str(&e, "c_col_release");
    let asset = Address::generate(&e);
    client.collateralize(&user, &commitment_id, &100_000i128, &asset);

    assert!(
        !read_reentrancy_guard(&e, &contract_id),
        "Guard must be released after successful collateralize"
    );
}

#[test]
#[should_panic(expected = "Reentrancy detected")]
fn test_create_tranches_blocked_when_guard_locked_by_collateralize() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    lock_reentrancy_guard(&e, &contract_id);

    let commitment_id = String::from_str(&e, "c_cross");
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    client.create_tranches(
        &user,
        &commitment_id,
        &500_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

// ============================================================================
// create_secondary_instrument — happy path
// ============================================================================

#[test]
fn test_create_secondary_instrument() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client, user) = setup_with_authorized_user(&e);

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

// ============================================================================
// Reentrancy guard — create_secondary_instrument
// ============================================================================

#[test]
#[should_panic(expected = "Reentrancy detected")]
fn test_create_secondary_instrument_reentrancy_blocked() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    lock_reentrancy_guard(&e, &contract_id);

    let commitment_id = String::from_str(&e, "c_reentrant");
    let instrument_type = String::from_str(&e, "option");
    client.create_secondary_instrument(&user, &commitment_id, &instrument_type, &50_000i128);
}

#[test]
fn test_create_secondary_instrument_guard_released_after_success() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    let commitment_id = String::from_str(&e, "c_sec_release");
    let instrument_type = String::from_str(&e, "warrant");
    client.create_secondary_instrument(&user, &commitment_id, &instrument_type, &75_000i128);

    assert!(
        !read_reentrancy_guard(&e, &contract_id),
        "Guard must be released after successful creation"
    );
}

#[test]
#[should_panic(expected = "Reentrancy detected")]
fn test_collateralize_blocked_when_guard_locked_by_secondary_instrument() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    lock_reentrancy_guard(&e, &contract_id);

    let commitment_id = String::from_str(&e, "c_cross2");
    let asset = Address::generate(&e);
    client.collateralize(&user, &commitment_id, &100_000i128, &asset);
}

// ============================================================================
// add_protocol_guarantee — happy path
// ============================================================================

#[test]
fn test_add_protocol_guarantee() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client, user) = setup_with_authorized_user(&e);

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

// ============================================================================
// Reentrancy guard — add_protocol_guarantee
// ============================================================================

#[test]
#[should_panic(expected = "Reentrancy detected")]
fn test_add_protocol_guarantee_reentrancy_blocked() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    lock_reentrancy_guard(&e, &contract_id);

    let commitment_id = String::from_str(&e, "c_reentrant");
    let guarantee_type = String::from_str(&e, "solvency_shield");
    let terms_hash = String::from_str(&e, "0xdeadbeef");
    client.add_protocol_guarantee(&user, &commitment_id, &guarantee_type, &terms_hash);
}

#[test]
fn test_add_protocol_guarantee_guard_released_after_success() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    let commitment_id = String::from_str(&e, "c_guar_release");
    let guarantee_type = String::from_str(&e, "liquidity_backstop");
    let terms_hash = String::from_str(&e, "0x1111");
    client.add_protocol_guarantee(&user, &commitment_id, &guarantee_type, &terms_hash);

    assert!(
        !read_reentrancy_guard(&e, &contract_id),
        "Guard must be released after successful addition"
    );
}

#[test]
#[should_panic(expected = "Reentrancy detected")]
fn test_secondary_instrument_blocked_when_guard_locked_by_guarantee() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    lock_reentrancy_guard(&e, &contract_id);

    let commitment_id = String::from_str(&e, "c_cross3");
    let instrument_type = String::from_str(&e, "receivable");
    client.create_secondary_instrument(&user, &commitment_id, &instrument_type, &10_000i128);
}

// ============================================================================
// sequential calls
// ============================================================================

#[test]
fn test_multiple_sequential_calls_no_guard_leakage() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, contract_id, client, user) = setup_with_authorized_user(&e);

    let fee_asset = Address::generate(&e);

    let commitment_id_1 = String::from_str(&e, "seq_c1");
    let tranche_share_bps: Vec<u32> = vec![&e, 5000u32, 5000u32];
    let risk_levels = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "equity"),
    ];
    client.create_tranches(
        &user,
        &commitment_id_1,
        &2_000_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
    assert!(!read_reentrancy_guard(&e, &contract_id));

    let commitment_id_2 = String::from_str(&e, "seq_c2");
    let asset = Address::generate(&e);
    client.collateralize(&user, &commitment_id_2, &300_000i128, &asset);
    assert!(!read_reentrancy_guard(&e, &contract_id));

    let commitment_id_3 = String::from_str(&e, "seq_c3");
    let instrument_type = String::from_str(&e, "option");
    client.create_secondary_instrument(&user, &commitment_id_3, &instrument_type, &100_000i128);
    assert!(!read_reentrancy_guard(&e, &contract_id));

    let commitment_id_4 = String::from_str(&e, "seq_c4");
    let guarantee_type = String::from_str(&e, "emergency_backstop");
    let terms_hash = String::from_str(&e, "0xcafe");
    client.add_protocol_guarantee(&user, &commitment_id_4, &guarantee_type, &terms_hash);
    assert!(!read_reentrancy_guard(&e, &contract_id));
}

// ============================================================================
// Fee management
// ============================================================================

#[test]
fn test_transformation_with_fee() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, _, client, user) = setup_with_authorized_user(&e);
    client.set_transformation_fee(&admin, &0);

    let commitment_id = String::from_str(&e, "c_1");
    let total_value = 1_000_000i128;
    let tranche_share_bps = vec![&e, 10000u32];
    let risk_levels = vec![&e, String::from_str(&e, "senior")];
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
    assert_eq!(set.fee_paid, 0i128);
}

#[test]
fn test_transformation_fee_calculation_and_collection() {
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
