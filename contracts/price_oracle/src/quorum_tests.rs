//! Security regression tests for multi-source price aggregation.
//!
//! The production contract keeps a legacy latest-price key, so these tests use
//! the reader APIs as a consumer would: quorum and freshness are evaluated at
//! read time and a median is returned only from fresh, authorized observations.

#![cfg(test)]

extern crate std;

use crate::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{Address, Env, Vec};
use std::boxed::Box;

struct Fixture {
    e: Env,
    admin: Address,
    asset: Address,
    oracle_one: Address,
    oracle_two: Address,
    oracle_three: Address,
    client: PriceOracleContractClient<'static>,
}

fn fixture() -> Fixture {
    // The client lifetime is tied to the leaked test environment solely to keep
    // this helper readable; each test owns an isolated environment.
    let e = Box::leak(Box::new(Env::default()));
    e.mock_all_auths();
    let admin = Address::generate(e);
    let asset = Address::generate(e);
    let oracle_one = Address::generate(e);
    let oracle_two = Address::generate(e);
    let oracle_three = Address::generate(e);
    let contract_id = e.register_contract(None, PriceOracleContract);
    let client = PriceOracleContractClient::new(e, &contract_id);
    e.as_contract(&contract_id, || {
        PriceOracleContract::initialize(e.clone(), admin.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle_one.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle_two.clone()).unwrap();
        PriceOracleContract::add_oracle(e.clone(), admin.clone(), oracle_three.clone()).unwrap();
    });
    Fixture {
        e: e.clone(),
        admin,
        asset,
        oracle_one,
        oracle_two,
        oracle_three,
        client,
    }
}

#[test]
fn odd_quorum_returns_the_sorted_middle_value() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &3);
    f.client.set_price(&f.oracle_one, &f.asset, &900, &2);
    f.client.set_price(&f.oracle_two, &f.asset, &100, &2);
    f.client.set_price(&f.oracle_three, &f.asset, &500, &2);

    let data = f.client.get_median_price(&f.asset, &None);
    assert_eq!(data.price, 500);
    assert_eq!(data.decimals, 2);
    assert_eq!(f.client.get_price_valid(&f.asset, &None).price, 500);
}

#[test]
fn an_outlier_does_not_move_the_three_source_median() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &3);
    f.client.set_price(&f.oracle_one, &f.asset, &100, &0);
    f.client.set_price(&f.oracle_two, &f.asset, &101, &0);
    f.client
        .set_price(&f.oracle_three, &f.asset, &9_000_000, &0);

    assert_eq!(f.client.get_median_price(&f.asset, &None).price, 101);
}

#[test]
fn equal_observations_are_stable_regardless_of_submission_order() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &3);
    f.client.set_price(&f.oracle_three, &f.asset, &42, &0);
    f.client.set_price(&f.oracle_one, &f.asset, &42, &0);
    f.client.set_price(&f.oracle_two, &f.asset, &42, &0);

    assert_eq!(f.client.get_median_price(&f.asset, &None).price, 42);
}

#[test]
fn even_quorum_uses_overflow_safe_midpoint() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &2);
    f.client.set_price(&f.oracle_one, &f.asset, &i128::MAX, &0);
    f.client
        .set_price(&f.oracle_two, &f.asset, &(i128::MAX - 2), &0);

    assert_eq!(
        f.client.get_median_price(&f.asset, &None).price,
        i128::MAX - 1
    );
}

#[test]
fn different_decimals_are_normalized_to_highest_precision() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &2);
    f.client.set_price(&f.oracle_one, &f.asset, &12, &0);
    f.client.set_price(&f.oracle_two, &f.asset, &1_300, &2);

    let data = f.client.get_median_price(&f.asset, &None);
    assert_eq!(data.price, 1_250);
    assert_eq!(data.decimals, 2);
}

#[test]
fn underflowing_precision_conversion_is_deterministic() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &2);
    f.client.set_price(&f.oracle_one, &f.asset, &1_234, &3);
    f.client.set_price(&f.oracle_two, &f.asset, &123_500, &5);

    let data = f.client.get_median_price(&f.asset, &None);
    assert_eq!(data.price, 123_450);
    assert_eq!(data.decimals, 5);
}

#[test]
fn one_source_is_the_backward_compatible_default_quorum() {
    let f = fixture();
    f.client.set_price(&f.oracle_one, &f.asset, &777, &8);

    assert_eq!(f.client.get_quorum(&f.asset), 1);
    assert_eq!(f.client.get_price_valid(&f.asset, &None).price, 777);
}

#[test]
fn insufficient_fresh_sources_fail_closed() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &3);
    f.client.set_price(&f.oracle_one, &f.asset, &100, &8);
    f.client.set_price(&f.oracle_two, &f.asset, &101, &8);

    assert_eq!(
        f.client.try_get_median_price(&f.asset, &None),
        Err(Ok(OracleError::InsufficientObservations))
    );
}

#[test]
fn stale_observations_do_not_satisfy_quorum() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &2);
    f.client.set_price(&f.oracle_one, &f.asset, &100, &8);
    f.client.set_price(&f.oracle_two, &f.asset, &101, &8);
    f.e.ledger().with_mut(|ledger| ledger.timestamp += 61);

    assert_eq!(
        f.client.try_get_median_price(&f.asset, &Some(60)),
        Err(Ok(OracleError::InsufficientObservations))
    );
}

#[test]
fn one_stale_legacy_source_keeps_the_existing_stale_error() {
    let f = fixture();
    f.client.set_price(&f.oracle_one, &f.asset, &100, &8);
    f.e.ledger().with_mut(|ledger| ledger.timestamp += 3_601);

    assert_eq!(
        f.client.try_get_price_valid(&f.asset, &None),
        Err(Ok(OracleError::StalePrice))
    );
}

#[test]
fn future_observations_are_rejected_as_stale() {
    let f = fixture();
    f.client.set_price(&f.oracle_one, &f.asset, &100, &8);
    let future = PriceData {
        price: 100,
        updated_at: f.e.ledger().timestamp() + 1,
        decimals: 8,
    };
    f.e.as_contract(&f.client.address, || {
        f.e.storage().instance().set(
            &DataKey::Observation(f.asset.clone(), f.oracle_one.clone()),
            &future,
        );
    });

    assert_eq!(
        f.client.try_get_median_price(&f.asset, &None),
        Err(Ok(OracleError::StalePrice))
    );
}

#[test]
fn zero_quorum_is_rejected_by_admin_configuration() {
    let f = fixture();
    assert_eq!(
        f.client.try_set_quorum(&f.admin, &f.asset, &0),
        Err(Ok(OracleError::InvalidQuorum))
    );
}

#[test]
fn non_admin_cannot_change_quorum() {
    let f = fixture();
    let not_admin = Address::generate(&f.e);
    assert_eq!(
        f.client.try_set_quorum(&not_admin, &f.asset, &2),
        Err(Ok(OracleError::Unauthorized))
    );
}

#[test]
fn decimals_above_safe_precision_are_rejected_on_write() {
    let f = fixture();
    assert_eq!(
        f.client
            .try_set_price(&f.oracle_one, &f.asset, &100, &(MAX_PRICE_DECIMALS + 1)),
        Err(Ok(OracleError::InvalidDecimals))
    );
}

#[test]
fn decimal_scaling_overflow_fails_closed() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &2);
    f.client.set_price(&f.oracle_one, &f.asset, &i128::MAX, &0);
    f.client.set_price(&f.oracle_two, &f.asset, &1, &18);

    assert_eq!(
        f.client.try_get_median_price(&f.asset, &None),
        Err(Ok(OracleError::ArithmeticOverflow))
    );
}

#[test]
fn repeated_source_updates_replace_without_counting_twice() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &2);
    f.client.set_price(&f.oracle_one, &f.asset, &1, &0);
    f.client.set_price(&f.oracle_one, &f.asset, &100, &0);
    f.client.set_price(&f.oracle_two, &f.asset, &200, &0);

    assert_eq!(f.client.get_median_price(&f.asset, &None).price, 150);
}

#[test]
fn removed_source_no_longer_counts_toward_quorum() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &2);
    f.client.set_price(&f.oracle_one, &f.asset, &100, &0);
    f.client.set_price(&f.oracle_two, &f.asset, &100, &0);
    f.client.remove_oracle(&f.admin, &f.oracle_two);

    assert_eq!(
        f.client.try_get_median_price(&f.asset, &None),
        Err(Ok(OracleError::InsufficientObservations))
    );
}

#[test]
fn a_fresh_source_can_restore_a_failed_quorum() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &2);
    f.client.set_price(&f.oracle_one, &f.asset, &100, &0);
    f.client.set_price(&f.oracle_two, &f.asset, &101, &0);
    f.e.ledger().with_mut(|ledger| ledger.timestamp += 100);
    f.client.set_price(&f.oracle_three, &f.asset, &102, &0);

    assert_eq!(f.client.get_median_price(&f.asset, &Some(60)).price, 102);
}

#[test]
fn aggregate_timestamp_is_the_oldest_accepted_observation() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &2);
    f.client.set_price(&f.oracle_one, &f.asset, &100, &0);
    f.e.ledger().with_mut(|ledger| ledger.timestamp += 5);
    f.client.set_price(&f.oracle_two, &f.asset, &110, &0);

    let data = f.client.get_median_price(&f.asset, &None);
    assert_eq!(data.updated_at, 0);
}

#[test]
fn legacy_storage_without_source_metadata_remains_readable() {
    let f = fixture();
    f.e.as_contract(&f.client.address, || {
        f.e.storage().instance().set(
            &DataKey::Price(f.asset.clone()),
            &PriceData {
                price: 55,
                updated_at: f.e.ledger().timestamp(),
                decimals: 8,
            },
        );
    });

    assert_eq!(f.client.get_price_valid(&f.asset, &None).price, 55);
}

#[test]
fn unauthorized_writes_do_not_create_source_metadata() {
    let f = fixture();
    let unauthorized = Address::generate(&f.e);
    assert!(f
        .client
        .try_set_price(&unauthorized, &f.asset, &1, &8)
        .is_err());
    assert_eq!(f.client.get_price(&f.asset).price, 0);
    assert_eq!(f.client.get_quorum(&f.asset), 1);
}

#[test]
fn explicit_median_api_matches_batch_consumer_expectation() {
    let f = fixture();
    f.client.set_quorum(&f.admin, &f.asset, &3);
    f.client.set_price(&f.oracle_one, &f.asset, &1_000_000, &8);
    f.client.set_price(&f.oracle_two, &f.asset, &1_020_000, &8);
    f.client
        .set_price(&f.oracle_three, &f.asset, &1_010_000, &8);
    let mut assets = Vec::new(&f.e);
    assets.push_back(f.asset.clone());

    let batch = f.client.get_batch_prices(&assets, &3600);
    assert_eq!(batch.get(0).unwrap().1.price, 1_010_000);
}

#[test]
fn marketplace_price_conversion_propagates_scaling_overflow() {
    let f = fixture();
    f.client.set_price(&f.oracle_one, &f.asset, &i128::MAX, &0);

    assert_eq!(
        f.client
            .try_get_price_for_marketplace(&f.asset, &Some(1)),
        Err(Ok(OracleError::ArithmeticOverflow))
    );
}
