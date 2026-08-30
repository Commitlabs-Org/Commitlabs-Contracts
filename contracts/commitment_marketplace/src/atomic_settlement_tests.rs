//! Regression tests for the fixed-price settlement boundary.
//!
//! These tests intentionally use a real Soroban token contract. A mock that
//! always succeeds cannot prove that a failed payment leaves the listing,
//! royalty configuration, and buyer balance untouched.

#![cfg(test)]

extern crate std;

use crate::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    token, Address, Env,
};

fn setup(
    e: &Env,
) -> (
    Address,
    Address,
    Address,
    Address,
    Address,
    CommitmentMarketplaceClient<'_>,
) {
    e.mock_all_auths();
    let admin = Address::generate(e);
    let nft = Address::generate(e);
    let fee_recipient = Address::generate(e);
    let marketplace = e.register_contract(None, CommitmentMarketplace);
    let client = CommitmentMarketplaceClient::new(e, &marketplace);
    client.initialize(&admin, &nft, &250, &fee_recipient);
    let token_admin = Address::generate(e);
    let token = e.register_stellar_asset_contract_v2(token_admin);
    let payment_token = token.address();
    client.add_payment_token(&payment_token);
    (admin, fee_recipient, payment_token, Address::generate(e), marketplace, client)
}

#[test]
fn underfunded_buyer_returns_error_before_listing_consumption() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    client.list_nft(&seller, &41, &1_000, &payment_token);

    let result = client.try_buy_nft(&buyer, &41);
    assert!(result.is_err());
    assert_eq!(client.get_listing(&41).price, 1_000);
    assert_eq!(client.get_all_listings().len(), 1);
}

#[test]
fn underfunded_buyer_preserves_royalty_configuration() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    let royalty_recipient = Address::generate(&e);
    client.list_nft(&seller, &42, &2_000, &payment_token);
    client.set_royalty(&seller, &42, &royalty_recipient, &500);

    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &1_999);
    assert!(client.try_buy_nft(&buyer, &42).is_err());
    assert_eq!(client.get_listing(&42).seller, seller);
    assert_eq!(client.get_royalty(&42).unwrap().basis_points, 500);
}

#[test]
fn underfunded_buyer_balance_is_not_debited() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    client.list_nft(&seller, &43, &500, &payment_token);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &499);
    let before = token::Client::new(&e, &payment_token).balance(&buyer);

    assert!(client.try_buy_nft(&buyer, &43).is_err());
    assert_eq!(token::Client::new(&e, &payment_token).balance(&buyer), before);
}

#[test]
fn removed_asset_cannot_settle_an_existing_listing() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    client.list_nft(&seller, &44, &750, &payment_token);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &750);
    client.remove_payment_token(&payment_token);

    assert!(client.try_buy_nft(&buyer, &44).is_err());
    assert_eq!(client.get_listing(&44).price, 750);
    assert_eq!(client.get_all_listings().len(), 1);
}

#[test]
fn failed_asset_validation_does_not_create_a_listing() {
    let e = Env::default();
    let (_, _, _, seller, _, client) = setup(&e);
    let unauthorized_asset = Address::generate(&e);

    assert!(client.try_list_nft(&seller, &45, &1_000, &unauthorized_asset).is_err());
    assert!(client.try_get_listing(&45).is_err());
    assert!(client.get_all_listings().is_empty());
}

#[test]
fn successful_settlement_consumes_listing_once() {
    let e = Env::default();
    let (_, fee_recipient, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    let amount = 10_000i128;
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &amount);
    client.list_nft(&seller, &46, &amount, &payment_token);

    client.buy_nft(&buyer, &46);
    assert!(client.try_get_listing(&46).is_err());
    assert!(client.get_all_listings().is_empty());
    assert_eq!(token::Client::new(&e, &payment_token).balance(&fee_recipient), 250);
    assert!(client.try_buy_nft(&buyer, &46).is_err());
}

#[test]
fn duplicate_settlement_cannot_move_payment_twice() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    let second_buyer = Address::generate(&e);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &1_000);
    token::StellarAssetClient::new(&e, &payment_token).mint(&second_buyer, &1_000);
    client.list_nft(&seller, &47, &1_000, &payment_token);
    client.buy_nft(&buyer, &47);

    let seller_balance = token::Client::new(&e, &payment_token).balance(&seller);
    assert!(client.try_buy_nft(&second_buyer, &47).is_err());
    assert_eq!(token::Client::new(&e, &payment_token).balance(&seller), seller_balance);
}

#[test]
fn fee_and_royalty_are_validated_before_transfers() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    let royalty_recipient = Address::generate(&e);
    client.list_nft(&seller, &48, &10_000, &payment_token);
    client.set_royalty(&seller, &48, &royalty_recipient, &1_000);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &10_000);

    client.buy_nft(&buyer, &48);
    let payments = token::Client::new(&e, &payment_token);
    assert_eq!(payments.balance(&seller) + payments.balance(&royalty_recipient), 9_750);
    assert_eq!(payments.balance(&buyer), 0);
}

#[test]
fn settlement_event_is_emitted_after_listing_is_consumed() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &100);
    client.list_nft(&seller, &49, &100, &payment_token);
    client.buy_nft(&buyer, &49);

    let events = e.events().all();
    let last = events.last().expect("settlement must emit an event");
    assert_eq!(last.0, client.address);
    assert!(client.try_get_listing(&49).is_err());
}

#[test]
fn allowlist_rejection_does_not_change_other_listings() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let unauthorized_asset = Address::generate(&e);
    client.list_nft(&seller, &50, &100, &payment_token);

    assert!(client.try_list_nft(&seller, &51, &100, &unauthorized_asset).is_err());
    assert_eq!(client.get_all_listings().len(), 1);
    assert_eq!(client.get_listing(&50).token_id, 50);
}

#[test]
fn zero_balance_is_not_treated_as_a_valid_payment() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    client.list_nft(&seller, &52, &1, &payment_token);

    let result = client.try_buy_nft(&buyer, &52);
    assert!(result.is_err());
    assert_eq!(client.get_listing(&52).seller, seller);
}

#[test]
fn own_purchase_is_rejected_before_a_balance_debit() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    token::StellarAssetClient::new(&e, &payment_token).mint(&seller, &100);
    client.list_nft(&seller, &53, &100, &payment_token);
    let before = token::Client::new(&e, &payment_token).balance(&seller);

    assert!(client.try_buy_nft(&seller, &53).is_err());
    assert_eq!(token::Client::new(&e, &payment_token).balance(&seller), before);
    assert_eq!(client.get_listing(&53).seller, seller);
}

#[test]
fn invalid_price_does_not_leave_a_guard_or_listing() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);

    assert!(client.try_list_nft(&seller, &54, &0, &payment_token).is_err());
    assert!(client.try_get_listing(&54).is_err());
    assert!(!client.is_paused());
    client.list_nft(&seller, &55, &1, &payment_token);
    assert_eq!(client.get_listing(&55).price, 1);
}

#[test]
fn invalid_royalty_does_not_change_the_listing() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let recipient = Address::generate(&e);
    client.list_nft(&seller, &56, &1_000, &payment_token);

    assert!(client.try_set_royalty(&seller, &56, &recipient, &(MAX_ROYALTY_BASIS_POINTS + 1)).is_err());
    assert_eq!(client.get_listing(&56).price, 1_000);
    assert!(client.get_royalty(&56).is_none());
}

#[test]
fn allowlist_add_is_idempotent_and_does_not_duplicate_assets() {
    let e = Env::default();
    let (_, _, payment_token, _, _, client) = setup(&e);

    client.add_payment_token(&payment_token);
    client.add_payment_token(&payment_token);
    assert_eq!(client.get_allowed_payment_tokens().len(), 1);
    assert!(client.is_payment_token_allowed(&payment_token));
}

#[test]
fn reallowlisting_an_asset_allows_reviewed_listing_to_settle() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    client.remove_payment_token(&payment_token);
    assert!(client.try_list_nft(&seller, &57, &100, &payment_token).is_err());
    client.add_payment_token(&payment_token);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &100);
    client.list_nft(&seller, &57, &100, &payment_token);
    client.buy_nft(&buyer, &57);
    assert!(client.try_get_listing(&57).is_err());
}

#[test]
fn a_failed_second_listing_does_not_disturb_the_first() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let rejected_token = Address::generate(&e);
    client.list_nft(&seller, &58, &100, &payment_token);
    assert!(client.try_list_nft(&seller, &59, &100, &rejected_token).is_err());

    assert_eq!(client.get_all_listings().len(), 1);
    assert_eq!(client.get_listing(&58).token_id, 58);
    assert!(client.try_get_listing(&59).is_err());
}

#[test]
fn failed_purchase_preserves_active_listing_order() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    client.list_nft(&seller, &60, &100, &payment_token);
    client.list_nft(&seller, &61, &200, &payment_token);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &199);

    assert!(client.try_buy_nft(&buyer, &61).is_err());
    let active = client.get_all_listings();
    assert_eq!(active.len(), 2);
    assert_eq!(active.get(0).unwrap().token_id, 60);
    assert_eq!(active.get(1).unwrap().token_id, 61);
}

#[test]
fn successful_purchase_removes_only_the_consumed_listing() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &100);
    client.list_nft(&seller, &62, &100, &payment_token);
    client.list_nft(&seller, &63, &100, &payment_token);

    client.buy_nft(&buyer, &62);
    let active = client.get_all_listings();
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap().token_id, 63);
    assert_eq!(client.get_listing(&63).token_id, 63);
}

#[test]
fn successful_purchase_clears_only_the_consumed_royalty() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    let recipient = Address::generate(&e);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &200);
    client.list_nft(&seller, &64, &100, &payment_token);
    client.list_nft(&seller, &65, &100, &payment_token);
    client.set_royalty(&seller, &64, &recipient, &100);
    client.set_royalty(&seller, &65, &recipient, &200);

    client.buy_nft(&buyer, &64);
    assert!(client.get_royalty(&64).is_none());
    assert_eq!(client.get_royalty(&65).unwrap().basis_points, 200);
}

#[test]
fn full_fee_configuration_is_conserved_at_the_boundary() {
    let e = Env::default();
    let (admin, fee_recipient, payment_token, seller, _, client) = setup(&e);
    client.update_fee(&10_000);
    let buyer = Address::generate(&e);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &1_000);
    client.list_nft(&seller, &66, &1_000, &payment_token);

    client.buy_nft(&buyer, &66);
    let payments = token::Client::new(&e, &payment_token);
    assert_eq!(payments.balance(&buyer), 0);
    assert_eq!(payments.balance(&seller), 0);
    assert_eq!(payments.balance(&fee_recipient), 1_000);
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn failed_purchase_does_not_emit_a_sale_event() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    client.list_nft(&seller, &67, &1_000, &payment_token);
    let before = e.events().all().len();

    assert!(client.try_buy_nft(&buyer, &67).is_err());
    assert_eq!(e.events().all().len(), before);
}

#[test]
fn a_removed_asset_cannot_consume_any_of_its_existing_listings() {
    let e = Env::default();
    let (_, _, payment_token, seller, _, client) = setup(&e);
    let buyer = Address::generate(&e);
    token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &200);
    client.list_nft(&seller, &68, &100, &payment_token);
    client.list_nft(&seller, &69, &100, &payment_token);
    client.remove_payment_token(&payment_token);

    assert!(client.try_buy_nft(&buyer, &68).is_err());
    assert!(client.try_buy_nft(&buyer, &69).is_err());
    assert_eq!(client.get_all_listings().len(), 2);
}
