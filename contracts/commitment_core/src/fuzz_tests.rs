#![cfg(test)]

use crate::{
    fuzzing::{
        checked_fee_and_net_from_bps, checked_fee_from_bps, classify_generated_commitment_id_bytes,
        observe_amount, observe_commitment_input, AmountShape, CommitmentIdShape,
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
}

#[test]
fn test_fuzz_checked_fee_conservation_seed_grid() {
    let amounts = [
        0i128,
        1,
        9_999,
        10_000,
        10_001,
        i128::MAX / 10_000 - 1,
        i128::MAX / 10_000,
        i128::MAX / 10_000 + 1,
        i128::MAX - 10_000,
        i128::MAX - 1,
        i128::MAX,
    ];
    let bps_values = [0u32, 1, 2, 99, 100, 5_000, 9_999, 10_000];

    for amount in amounts {
        for fee_bps in bps_values {
            let fee = checked_fee_from_bps(amount, fee_bps)
                .expect("valid non-negative amount and bps should not overflow");
            let (fee_from_pair, net) = checked_fee_and_net_from_bps(amount, fee_bps)
                .expect("valid non-negative amount and bps should produce fee and net");

            assert_eq!(fee, fee_from_pair);
            assert!(fee >= 0, "fee must never be negative");
            assert!(fee <= amount, "fee must never exceed amount");
            assert!(net >= 0, "net amount must never be negative");
            assert_eq!(
                net.checked_add(fee),
                Some(amount),
                "net_amount + fee must conserve the original amount"
            );

            if fee_bps == 0 {
                assert_eq!(fee, 0);
                assert_eq!(net, amount);
            }
            if fee_bps == 10_000 {
                assert_eq!(fee, amount);
                assert_eq!(net, 0);
            }
        }
    }
}

#[test]
fn test_fuzz_checked_fee_rejects_invalid_domain_seed_grid() {
    assert_eq!(checked_fee_from_bps(-1, 0), None);
    assert_eq!(checked_fee_from_bps(1, 10_001), None);
    assert_eq!(checked_fee_and_net_from_bps(-1, 10_000), None);
    assert_eq!(checked_fee_and_net_from_bps(1, 10_001), None);
}

#[test]
fn test_fuzz_amount_observation_handles_max_adjacent_boundaries() {
    for amount in [i128::MAX - 1, i128::MAX] {
        let full_fee = observe_amount(amount, 10_000);
        assert_eq!(full_fee.shape, AmountShape::Valid);
        assert_eq!(full_fee.fee, Some(amount));
        assert_eq!(full_fee.net, Some(0));

        let tiny_fee = observe_amount(amount, 1);
        assert_eq!(tiny_fee.shape, AmountShape::Valid);
        assert!(tiny_fee.fee.unwrap() <= amount);
        assert_eq!(
            tiny_fee.net.unwrap().checked_add(tiny_fee.fee.unwrap()),
            Some(amount)
        );
    }
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
fn test_create_commitment_rejects_fee_math_overflow() {
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

    let result = client.try_create_commitment(&owner, &amount, &asset_address, &default_rules(&e));

    assert!(result.is_err());
    assert_eq!(client.get_total_commitments(), 0);
    assert_eq!(client.get_total_value_locked(), 0);
    assert_eq!(client.get_collected_fees(&asset_address), 0);
}
