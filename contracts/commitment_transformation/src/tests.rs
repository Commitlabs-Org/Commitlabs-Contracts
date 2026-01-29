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
    let id = client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
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
    client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
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
    let (admin, core, user) = setup(&e);
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
    client.create_tranches(
        &unauthorized,
        &commitment_id,
        &1_000_000i128,
        &tranche_share_bps,
        &risk_levels,
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
    client.set_transformation_fee(&admin, &100); // 1%
    client.set_authorized_transformer(&admin, &user, &true);

    let commitment_id = String::from_str(&e, "c_1");
    let total_value = 1_000_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let id = client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
    );
    let set = client.get_tranche_set(&id);
    assert_eq!(set.fee_paid, 10_000i128); // 1% of 1_000_000
}
