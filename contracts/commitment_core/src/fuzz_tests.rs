#![cfg(test)]

use crate::{
    fuzzing::{
        checked_fee_value_from_bps, classify_generated_commitment_id_bytes, observe_amount,
        observe_commitment_input, AmountShape, CommitmentIdShape, FeeValueObservation,
    },
    CommitmentCoreContract, CommitmentCoreContractClient, CommitmentRules,
};
use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, token::StellarAssetClient, Address, Env,
    String,
};

#[contract]
struct FuzzMockNftContract;

#[contractimpl]
impl FuzzMockNftContract {
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

fn default_rules(e: &Env) -> CommitmentRules {
    CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(e, "safe"),
        early_exit_penalty: 15,
        min_fee_threshold: 0,
        grace_period_days: 0,
    }
}

#[test]
fn test_fuzz_commitment_id_seed_shapes() {
    assert_eq!(
        classify_generated_commitment_id_bytes(b""),
        CommitmentIdShape::Empty
    );
    assert_eq!(
        classify_generated_commitment_id_bytes(b"user_supplied"),
        CommitmentIdShape::InvalidPrefix
    );
    assert_eq!(
        classify_generated_commitment_id_bytes(b"c_"),
        CommitmentIdShape::MissingDigits
    );
    assert_eq!(
        classify_generated_commitment_id_bytes(b"c_12x"),
        CommitmentIdShape::NonDigitSuffix
    );
    assert_eq!(
        classify_generated_commitment_id_bytes(b"c_18446744073709551615"),
        CommitmentIdShape::ValidGenerated
    );
}

#[test]
fn test_fuzz_commitment_id_rejects_oversized_seed() {
    let oversized = *b"c_1234567890123456789012345678901";
    assert_eq!(
        classify_generated_commitment_id_bytes(&oversized),
        CommitmentIdShape::TooLong
    );
}

#[test]
fn test_fuzz_amount_seed_shapes() {
    assert_eq!(observe_amount(0, 0).shape, AmountShape::NonPositive);
    assert_eq!(observe_amount(-1, 0).shape, AmountShape::NonPositive);
    assert_eq!(observe_amount(1, 10_001).shape, AmountShape::InvalidFeeBps);

    let max_fee = observe_amount(1, 10_000);
    assert_eq!(max_fee.shape, AmountShape::Valid);
    assert_eq!(max_fee.fee, Some(1));
    assert_eq!(max_fee.net, Some(0));

    let normal = observe_amount(1_000, 100);
    assert_eq!(normal.shape, AmountShape::Valid);
    assert_eq!(normal.fee, Some(10));
    assert_eq!(normal.net, Some(990));

    let max_adjacent = observe_amount(i128::MAX, 2);
    assert_eq!(max_adjacent.shape, AmountShape::Valid);
    assert_eq!(
        max_adjacent
            .net
            .unwrap()
            .checked_add(max_adjacent.fee.unwrap()),
        Some(i128::MAX)
    );
}

fn assert_fee_value_invariants(amount: i128, bps: u32) {
    let FeeValueObservation { net, fee } =
        checked_fee_value_from_bps(amount, bps).expect("valid fee input must compute");

    assert!(
        fee >= 0,
        "fee must be non-negative for amount={amount}, bps={bps}"
    );
    assert!(
        fee <= amount,
        "fee must not exceed amount={amount}, bps={bps}"
    );
    assert!(
        net >= 0,
        "net must be non-negative for amount={amount}, bps={bps}"
    );
    assert_eq!(
        net.checked_add(fee),
        Some(amount),
        "net + fee must conserve amount for amount={amount}, bps={bps}"
    );
}

#[test]
fn test_fuzz_fee_value_seed_boundaries_conserve_amount() {
    let amounts = [
        0,
        1,
        9_999,
        10_000,
        10_001,
        i128::MAX / 10_000 - 1,
        i128::MAX / 10_000,
        i128::MAX / 10_000 + 1,
        i128::MAX / 2,
        i128::MAX - 10_000,
        i128::MAX - 1,
        i128::MAX,
    ];
    let bps_values = [0, 1, 2, 15, 100, 9_999, 10_000];

    for amount in amounts {
        for bps in bps_values {
            assert_fee_value_invariants(amount, bps);
        }
    }
}

#[test]
fn test_fuzz_fee_value_deterministic_input_sweep() {
    let mut state = 0x9e37_79b9_7f4a_7c15_d1b5_4a32_d192_ed03u128;

    for _ in 0..4096 {
        state = state
            .wrapping_mul(0xda94_2042_e4dd_58b5_0000_0000_0000_0001u128)
            .wrapping_add(0x9e37_79b9_7f4a_7c15u128);
        let amount = (state >> 1) as i128;
        let bps = ((state >> 96) % 10_001) as u32;

        assert_fee_value_invariants(amount, bps);
    }
}

#[test]
fn test_fuzz_fee_value_rejects_invalid_domain() {
    assert_eq!(checked_fee_value_from_bps(-1, 0), None);
    assert_eq!(checked_fee_value_from_bps(1, 10_001), None);
}

#[test]
fn test_fuzz_observation_combines_id_and_amount_seed() {
    let observation = observe_commitment_input(b"c_42", 1_000, 100);
    assert_eq!(observation.id_shape, CommitmentIdShape::ValidGenerated);
    assert_eq!(observation.amount.shape, AmountShape::Valid);
    assert_eq!(observation.amount.fee, Some(10));
    assert_eq!(observation.amount.net, Some(990));
}

#[test]
fn test_create_commitment_accepts_max_amount_fee_without_overflow() {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let nft_contract = e.register_contract(None, FuzzMockNftContract);
    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let owner = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let amount = i128::MAX;

    let token_contract = e.register_stellar_asset_contract_v2(token_admin);
    let asset_address = token_contract.address();
    let token_admin_client = StellarAssetClient::new(&e, &asset_address);
    token_admin_client.mint(&owner, &amount);

    client.initialize(&admin, &nft_contract);
    client.set_creation_fee_bps(&admin, &2);

    let commitment_id =
        client.create_commitment(&owner, &amount, &asset_address, &default_rules(&e));
    let commitment = client.get_commitment(&commitment_id);
    let expected_fee = checked_fee_value_from_bps(amount, 2).unwrap().fee;
    let expected_net = amount.checked_sub(expected_fee).unwrap();

    assert_eq!(commitment.amount, expected_net);
    assert_eq!(commitment.current_value, expected_net);
    assert_eq!(client.get_total_commitments(), 1);
    assert_eq!(client.get_total_value_locked(), expected_net);
    assert_eq!(client.get_collected_fees(&asset_address), expected_fee);
    assert_eq!(expected_net.checked_add(expected_fee), Some(amount));
}
