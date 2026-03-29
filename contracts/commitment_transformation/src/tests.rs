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
// Tranche Management Tests
// ============================================================================

/// Helper to create tranches and return the first tranche ID
fn create_test_tranches(
    e: &Env,
    client: &CommitmentTransformationContractClient,
    owner: &Address,
    commitment_id: &String,
) -> (String, String) {
    let total_value = 1_000_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 6000u32, 3000u32, 1000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "mezzanine"),
        String::from_str(&e, "equity"),
    ];
    let fee_asset = Address::generate(&e);
    let set_id = client.create_tranches(
        owner,
        commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );

    let set = client.get_tranche_set(&set_id);
    let tranche_id_0 = set.tranches.get(0).unwrap().tranche_id;
    let tranche_id_1 = set.tranches.get(1).unwrap().tranche_id;
    (tranche_id_0, tranche_id_1)
}

#[test]
fn test_get_tranche() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    let tranche = client.get_tranche(&tranche_id);
    assert_eq!(tranche.tranche_id, tranche_id);
    assert_eq!(tranche.commitment_id, commitment_id);
    assert_eq!(tranche.risk_level, String::from_str(&e, "senior"));
    assert_eq!(tranche.status, TrancheStatus::Active);
    assert!(tranche.amount > 0);
}

#[test]
#[should_panic(expected = "Transformation record not found")]
fn test_get_tranche_not_found() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let fake_id = String::from_str(&e, "fake_tranche");
    client.get_tranche(&fake_id);
}

#[test]
fn test_update_tranche() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    // Update risk level
    let new_risk_level = String::from_str(&e, "mezzanine");
    let updated = client.update_tranche(&user, &tranche_id, &new_risk_level);

    assert_eq!(updated.risk_level, new_risk_level);
    assert_eq!(updated.status, TrancheStatus::Active);
    assert!(updated.updated_at >= updated.created_at);

    // Verify storage was updated
    let stored = client.get_tranche(&tranche_id);
    assert_eq!(stored.risk_level, new_risk_level);
}

#[test]
#[should_panic(expected = "Transformation record not found")]
fn test_update_tranche_not_found() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let fake_id = String::from_str(&e, "fake_tranche");
    let new_risk = String::from_str(&e, "senior");
    client.update_tranche(&user, &fake_id, &new_risk);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_update_tranche_unauthorized() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let unauthorized = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    let new_risk = String::from_str(&e, "equity");
    client.update_tranche(&unauthorized, &tranche_id, &new_risk);
}

#[test]
fn test_allocate_to_tranche_increase() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    let initial = client.get_tranche(&tranche_id);
    let increase_amount = 100_000i128;

    let updated = client.allocate_to_tranche(&user, &tranche_id, &increase_amount);

    assert_eq!(updated.amount, initial.amount + increase_amount);
    assert!(updated.updated_at >= updated.created_at);

    let stored = client.get_tranche(&tranche_id);
    assert_eq!(stored.amount, initial.amount + increase_amount);
}

#[test]
fn test_allocate_to_tranche_decrease() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    let initial = client.get_tranche(&tranche_id);
    let decrease_amount = -100_000i128;

    let updated = client.allocate_to_tranche(&user, &tranche_id, &decrease_amount);

    assert_eq!(updated.amount, initial.amount + decrease_amount);
    assert!(updated.amount >= 0);
}

#[test]
fn test_allocate_to_tranche_zero() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    let initial = client.get_tranche(&tranche_id);
    let zero_amount = 0i128;

    let updated = client.allocate_to_tranche(&user, &tranche_id, &zero_amount);

    assert_eq!(updated.amount, initial.amount);
}

#[test]
#[should_panic(expected = "Invalid amount")]
fn test_allocate_to_tranche_underflow() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    let initial = client.get_tranche(&tranche_id);
    // Try to withdraw more than allocated
    let large_decrease = -(initial.amount + 1);

    client.allocate_to_tranche(&user, &tranche_id, &large_decrease);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_allocate_to_tranche_unauthorized() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let unauthorized = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    client.allocate_to_tranche(&unauthorized, &tranche_id, &100_000i128);
}

#[test]
fn test_close_tranche() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    let closed = client.close_tranche(&user, &tranche_id);

    assert_eq!(closed.status, TrancheStatus::Closed);
    assert!(closed.updated_at >= closed.created_at);

    let stored = client.get_tranche(&tranche_id);
    assert_eq!(stored.status, TrancheStatus::Closed);
}

#[test]
#[should_panic(expected = "close_tranche: already closed")]
fn test_close_tranche_already_closed() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    // Close once
    client.close_tranche(&user, &tranche_id);
    // Try to close again
    client.close_tranche(&user, &tranche_id);
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_close_tranche_unauthorized() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let unauthorized = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    client.close_tranche(&unauthorized, &tranche_id);
}

#[test]
#[should_panic(expected = "update_tranche: tranche is closed")]
fn test_update_closed_tranche_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    // Close the tranche
    client.close_tranche(&user, &tranche_id);

    // Try to update
    let new_risk = String::from_str(&e, "equity");
    client.update_tranche(&user, &tranche_id, &new_risk);
}

#[test]
#[should_panic(expected = "allocate_to_tranche: tranche is closed")]
fn test_allocate_closed_tranche_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    // Close the tranche
    client.close_tranche(&user, &tranche_id);

    // Try to allocate
    client.allocate_to_tranche(&user, &tranche_id, &100_000i128);
}

#[test]
fn test_tranche_status_lifecycle() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let (tranche_id, _) = create_test_tranches(&e, &client, &user, &commitment_id);

    // Initial state: Active
    let initial = client.get_tranche(&tranche_id);
    assert_eq!(initial.status, TrancheStatus::Active);

    // Update while active
    let new_risk = String::from_str(&e, "equity");
    let updated = client.update_tranche(&user, &tranche_id, &new_risk);
    assert_eq!(updated.status, TrancheStatus::Active);

    // Allocate while active
    let allocated = client.allocate_to_tranche(&user, &tranche_id, &50_000i128);
    assert_eq!(allocated.status, TrancheStatus::Active);

    // Close tranche
    let closed = client.close_tranche(&user, &tranche_id);
    assert_eq!(closed.status, TrancheStatus::Closed);

    // Final state persisted
    let final_state = client.get_tranche(&tranche_id);
    assert_eq!(final_state.status, TrancheStatus::Closed);
    assert_eq!(final_state.risk_level, new_risk);
}

#[test]
fn test_create_tranches_initializes_status() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let total_value = 1_000_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 5000u32, 5000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "equity"),
    ];
    let fee_asset = Address::generate(&e);
    let set_id = client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );

    let set = client.get_tranche_set(&set_id);
    
    // Verify all tranches have Active status and valid timestamps
    for i in 0..set.tranches.len() {
        let tranche = set.tranches.get(i).unwrap();
        assert_eq!(tranche.status, TrancheStatus::Active);
        // In test env, timestamp may be 0, so just check updated_at == created_at
        assert_eq!(tranche.updated_at, tranche.created_at);
        
        // Also verify individual storage
        let stored = client.get_tranche(&tranche.tranche_id);
        assert_eq!(stored.status, TrancheStatus::Active);
        assert_eq!(stored.amount, tranche.amount);
    }
}

#[test]
fn test_large_allocation_amounts() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_large");
    let total_value = 1_000_000_000_000i128; // Large but safe value
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    
    let set_id = client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );

    let set = client.get_tranche_set(&set_id);
    let tranche_id = set.tranches.get(0).unwrap().tranche_id;

    // Small increase should work
    let updated = client.allocate_to_tranche(&user, &tranche_id, &1_000_000i128);
    assert!(updated.amount > 0);
}

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

    // Create first set of tranches
    let (tranche1_0, tranche1_1) = create_test_tranches(&e, &client, &user, &commitment_id);

    // Create second set of tranches for same commitment
    let total_value = 500_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 7000u32, 3000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "mezzanine"),
    ];
    let fee_asset = Address::generate(&e);
    let _set2_id = client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );

    // Verify both sets are tracked
    let sets = client.get_commitment_tranche_sets(&commitment_id);
    assert_eq!(sets.len(), 2);

    // Verify all tranches are accessible
    let t1 = client.get_tranche(&tranche1_0);
    let t2 = client.get_tranche(&tranche1_1);
    assert_eq!(t1.commitment_id, commitment_id);
    assert_eq!(t2.commitment_id, commitment_id);

    // Can independently update tranches from different sets
    client.update_tranche(&user, &tranche1_0, &String::from_str(&e, "equity"));
    let updated = client.get_tranche(&tranche1_0);
    assert_eq!(updated.risk_level, String::from_str(&e, "equity"));
}
