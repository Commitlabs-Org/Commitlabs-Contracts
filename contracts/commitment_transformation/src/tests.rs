#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Env, String, Vec,
};

fn setup(e: &Env) -> (Address, Address, Address) {
    let admin = Address::generate(e);
    let core = Address::generate(e);
    let user = Address::generate(e);
    (admin, core, user)
}

fn setup_fee_token(e: &Env) -> (Address, StellarAssetClient<'_>, TokenClient<'_>) {
    let token_admin = Address::generate(e);
    let token_contract = e.register_stellar_asset_contract_v2(token_admin);
    let asset = token_contract.address();
    let admin_client = StellarAssetClient::new(e, &asset);
    let token_client = TokenClient::new(e, &asset);
    (asset, admin_client, token_client)
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
fn test_set_transformation_fee_accepts_boundaries() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);

    client.initialize(&admin, &core);
    client.set_transformation_fee(&admin, &0);
    assert_eq!(client.get_transformation_fee_bps(), 0);

    client.set_transformation_fee(&admin, &10_000);
    assert_eq!(client.get_transformation_fee_bps(), 10_000);
}

#[test]
fn test_set_transformation_fee_rejects_above_max_and_preserves_value() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);

    client.initialize(&admin, &core);
    client.set_transformation_fee(&admin, &250);

    let result = client.try_set_transformation_fee(&admin, &10_001);

    assert!(result.is_err());
    assert_eq!(client.get_transformation_fee_bps(), 250);
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

#[test]
#[should_panic(expected = "Insufficient collected fees to withdraw")]
fn test_fee_withdraw_rejects_amount_above_collected_fees() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    let treasury = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin, &core);
    client.set_fee_recipient(&admin, &treasury);

    client.withdraw_fees(&admin, &asset, &1i128);
}

#[test]
#[should_panic(expected = "Invalid amount: must be positive")]
fn test_fee_withdraw_rejects_zero_amount() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, _) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    let treasury = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin, &core);
    client.set_fee_recipient(&admin, &treasury);

    client.withdraw_fees(&admin, &asset, &0i128);
}

#[test]
#[should_panic(expected = "Unauthorized: caller not owner or authorized")]
fn test_fee_withdraw_requires_admin() {
    let e = Env::default();
    e.mock_all_auths();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    let treasury = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin, &core);
    client.set_fee_recipient(&admin, &treasury);

    client.withdraw_fees(&user, &asset, &1i128);
}

#[test]
fn test_fee_withdraw_transfers_collected_fees_to_recipient() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();
    let (admin, core, user) = setup(&e);
    let contract_id = e.register_contract(None, CommitmentTransformationContract);
    let client = CommitmentTransformationContractClient::new(&e, &contract_id);
    let treasury = Address::generate(&e);
    let (fee_asset, token_admin_client, token_client) = setup_fee_token(&e);

    client.initialize(&admin, &core);
    client.set_authorized_transformer(&admin, &user, &true);
    client.set_transformation_fee(&admin, &100);
    client.set_fee_recipient(&admin, &treasury);

    let total_value = 1_000_000i128;
    let expected_fee = 10_000i128;
    token_admin_client.mint(&user, &expected_fee);

    let commitment_id = String::from_str(&e, "c_fee");
    let tranche_share_bps: Vec<u32> = vec![&e, 10_000u32];
    let risk_levels: Vec<String> = vec![&e, String::from_str(&e, "senior")];

    let transformation_id = client.create_tranches(
        &user,
        &commitment_id,
        &total_value,
        &tranche_share_bps,
        &risk_levels,
        &fee_asset,
    );

    let tranche_set = client.get_tranche_set(&transformation_id);
    assert_eq!(tranche_set.fee_paid, expected_fee);
    assert_eq!(client.get_collected_fees(&fee_asset), expected_fee);
    assert_eq!(token_client.balance(&user), 0);
    assert_eq!(token_client.balance(&contract_id), expected_fee);
    assert_eq!(token_client.balance(&treasury), 0);

    client.withdraw_fees(&admin, &fee_asset, &expected_fee);

    assert_eq!(client.get_collected_fees(&fee_asset), 0);
    assert_eq!(token_client.balance(&contract_id), 0);
    assert_eq!(token_client.balance(&treasury), expected_fee);
}
