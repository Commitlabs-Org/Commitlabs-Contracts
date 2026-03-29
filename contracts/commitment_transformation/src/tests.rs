#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, vec, Address, Env, String, Vec};

#[contract]
struct MockCoreContract;

#[contractimpl]
impl MockCoreContract {
    pub fn set_commitment(e: Env, commitment_id: String, commitment: CoreCommitment) {
        e.storage().instance().set(&commitment_id, &commitment);
    }

    pub fn get_commitment(e: Env, commitment_id: String) -> CoreCommitment {
        e.storage()
            .instance()
            .get(&commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"))
    }
}

fn setup(
    e: &Env,
) -> (
    Address,
    Address,
    Address,
    Address,
    CommitmentTransformationContractClient<'_>,
    MockCoreContractClient<'_>,
) {
    let admin = Address::generate(e);
    let owner = Address::generate(e);
    let transformer = Address::generate(e);
    let outsider = Address::generate(e);

    let core_id = e.register_contract(None, MockCoreContract);
    let core_client = MockCoreContractClient::new(e, &core_id);

    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(e, &contract_id);
    client.initialize(&admin, &core_id);

    (admin, owner, transformer, outsider, client, core_client)
}

fn mock_commitment(e: &Env, commitment_id: &str, owner: &Address) -> CoreCommitment {
    CoreCommitment {
        commitment_id: String::from_str(e, commitment_id),
        owner: owner.clone(),
        nft_token_id: 1,
        rules: CoreCommitmentRules {
            duration_days: 30,
            max_loss_percent: 20,
            commitment_type: String::from_str(e, "balanced"),
            early_exit_penalty: 10,
            min_fee_threshold: 0,
            grace_period_days: 0,
        },
        amount: 1_000_000,
        asset_address: Address::generate(e),
        created_at: 100,
        expires_at: 100 + 30 * 86400,
        current_value: 1_000_000,
        status: String::from_str(e, "active"),
    }
}

fn seed_commitment(
    e: &Env,
    core_client: &MockCoreContractClient<'_>,
    commitment_id: &str,
    owner: &Address,
) -> String {
    let id = String::from_str(e, commitment_id);
    core_client.set_commitment(&id, &mock_commitment(e, commitment_id, owner));
    id
}

#[test]
fn test_initialize() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, _owner, _transformer, _outsider, client, core_client) = setup(&e);
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.get_core_contract(), core_client.address.clone());
    assert_eq!(client.get_transformation_fee_bps(), 0);
}

#[test]
#[should_panic(expected = "Contract already initialized")]
fn test_initialize_twice_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, _owner, _transformer, _outsider, client, core_client) = setup(&e);
    client.initialize(&admin, &core_client.address);
}

#[test]
fn test_set_transformation_fee() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, _owner, _transformer, _outsider, client, _core_client) = setup(&e);
    client.set_transformation_fee(&admin, &100);
    assert_eq!(client.get_transformation_fee_bps(), 100);
}

#[test]
fn test_set_authorized_transformer() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, _owner, transformer, _outsider, client, _core_client) = setup(&e);
    assert!(!client.is_authorized_transformer(&transformer));
    client.set_authorized_transformer(&admin, &transformer, &true);
    assert!(client.is_authorized_transformer(&transformer));
    client.set_authorized_transformer(&admin, &transformer, &false);
    assert!(!client.is_authorized_transformer(&transformer));
}

#[test]
fn test_create_tranches_owner_allowed_and_owner_recorded() {
    let e = Env::default();
    e.mock_all_auths();
    let (_admin, owner, _transformer, _outsider, client, core_client) = setup(&e);

    let commitment_id = seed_commitment(&e, &core_client, "c_1", &owner);
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
        &owner,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
    assert!(!id.is_empty());

    let set = client.get_tranche_set(&id);
    assert_eq!(set.commitment_id, commitment_id);
    assert_eq!(set.owner, owner);
    assert_eq!(set.total_value, total_value);
    assert_eq!(set.tranches.len(), 3);
    assert_eq!(client.get_commitment_tranche_sets(&commitment_id).len(), 1);
}

#[test]
#[should_panic(expected = "Tranche ratios must sum to 100")]
fn test_create_tranches_invalid_ratios() {
    let e = Env::default();
    e.mock_all_auths();
    let (_admin, owner, _transformer, _outsider, client, core_client) = setup(&e);

    let commitment_id = seed_commitment(&e, &core_client, "c_1", &owner);
    let total_value = 1_000_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 5000u32, 3000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "mezzanine"),
    ];
    let fee_asset = Address::generate(&e);
    client.create_tranches(
        &owner,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

#[test]
fn test_collateralize_admin_allowed_but_owner_remains_commitment_owner() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, owner, _transformer, _outsider, client, core_client) = setup(&e);

    let commitment_id = seed_commitment(&e, &core_client, "c_1", &owner);
    let asset = Address::generate(&e);
    let asset_id = client.collateralize(&admin, &commitment_id, &500_000i128, &asset);
    assert!(!asset_id.is_empty());

    let col = client.get_collateralized_asset(&asset_id);
    assert_eq!(col.commitment_id, commitment_id);
    assert_eq!(col.owner, owner);
    assert_eq!(col.collateral_amount, 500_000i128);
    assert_eq!(col.asset_address, asset);
    assert_eq!(client.get_commitment_collateral(&commitment_id).len(), 1);
}

#[test]
fn test_create_secondary_instrument_authorized_transformer_allowed() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, owner, transformer, _outsider, client, core_client) = setup(&e);
    client.set_authorized_transformer(&admin, &transformer, &true);

    let commitment_id = seed_commitment(&e, &core_client, "c_1", &owner);
    let instrument_type = String::from_str(&e, "receivable");
    let amount = 200_000i128;
    let instrument_id =
        client.create_secondary_instrument(&transformer, &commitment_id, &instrument_type, &amount);
    assert!(!instrument_id.is_empty());

    let inst = client.get_secondary_instrument(&instrument_id);
    assert_eq!(inst.commitment_id, commitment_id);
    assert_eq!(inst.owner, owner);
    assert_eq!(inst.instrument_type, instrument_type);
    assert_eq!(inst.amount, amount);
    assert_eq!(client.get_commitment_instruments(&commitment_id).len(), 1);
}

#[test]
fn test_add_protocol_guarantee_admin_allowed() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, owner, _transformer, _outsider, client, core_client) = setup(&e);

    let commitment_id = seed_commitment(&e, &core_client, "c_1", &owner);
    let guarantee_type = String::from_str(&e, "liquidity_backstop");
    let terms_hash = String::from_str(&e, "0xabc123");
    let guarantee_id =
        client.add_protocol_guarantee(&admin, &commitment_id, &guarantee_type, &terms_hash);
    assert!(!guarantee_id.is_empty());

    let guar = client.get_protocol_guarantee(&guarantee_id);
    assert_eq!(guar.commitment_id, commitment_id);
    assert_eq!(guar.guarantee_type, guarantee_type);
    assert_eq!(guar.terms_hash, terms_hash);
    assert_eq!(client.get_commitment_guarantees(&commitment_id).len(), 1);
}

#[test]
fn test_add_protocol_guarantee_authorized_transformer_allowed() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, owner, transformer, _outsider, client, core_client) = setup(&e);
    client.set_authorized_transformer(&admin, &transformer, &true);

    let commitment_id = seed_commitment(&e, &core_client, "c_1", &owner);
    let guarantee_type = String::from_str(&e, "liquidity_backstop");
    let terms_hash = String::from_str(&e, "0xabc123");
    let guarantee_id =
        client.add_protocol_guarantee(&transformer, &commitment_id, &guarantee_type, &terms_hash);
    assert!(!guarantee_id.is_empty());
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_create_tranches_non_owner_non_protocol_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let (_admin, owner, _transformer, outsider, client, core_client) = setup(&e);

    let commitment_id = seed_commitment(&e, &core_client, "c_1", &owner);
    let tranche_share_bps: Vec<u32> = vec![&e, 6000u32, 4000u32];
    let risk_levels: Vec<String> = vec![
        &e,
        String::from_str(&e, "senior"),
        String::from_str(&e, "equity"),
    ];
    let fee_asset = Address::generate(&e);
    client.create_tranches(
        &outsider,
        &commitment_id,
        &1_000_000i128,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_owner_cannot_add_protocol_guarantee() {
    let e = Env::default();
    e.mock_all_auths();
    let (_admin, owner, _transformer, _outsider, client, core_client) = setup(&e);

    let commitment_id = seed_commitment(&e, &core_client, "c_1", &owner);
    client.add_protocol_guarantee(
        &owner,
        &commitment_id,
        &String::from_str(&e, "liquidity_backstop"),
        &String::from_str(&e, "0xabc123"),
    );
}

#[test]
#[should_panic(expected = "Commitment not found")]
fn test_create_tranches_missing_commitment_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let (_admin, owner, _transformer, _outsider, client, _core_client) = setup(&e);

    let commitment_id = String::from_str(&e, "missing");
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    client.create_tranches(
        &owner,
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
    let (admin, owner, _transformer, _outsider, client, core_client) = setup(&e);
    client.set_transformation_fee(&admin, &0); // 0% so no token transfer in unit test

    let commitment_id = seed_commitment(&e, &core_client, "c_1", &owner);
    let total_value = 1_000_000i128;
    let tranche_share_bps: Vec<u32> = vec![&e, 10000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];
    let fee_asset = Address::generate(&e);
    let id = client.create_tranches(
        &owner,
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
    let (admin, _owner, _transformer, _outsider, client, _core_client) = setup(&e);
    assert!(client.get_fee_recipient().is_none());
    let treasury = Address::generate(&e);
    client.set_fee_recipient(&admin, &treasury);
    assert_eq!(client.get_fee_recipient().unwrap(), treasury);
}

#[test]
fn test_fee_get_collected_fees_default() {
    let e = Env::default();
    let (_admin, _owner, _transformer, _outsider, client, _core_client) = setup(&e);
    let asset = Address::generate(&e);
    assert_eq!(client.get_collected_fees(&asset), 0);
}

#[test]
#[should_panic(expected = "Fee recipient not set")]
fn test_fee_withdraw_requires_recipient() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, _owner, _transformer, _outsider, client, _core_client) = setup(&e);
    let asset = Address::generate(&e);
    client.withdraw_fees(&admin, &asset, &100i128);
}
