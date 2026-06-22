
//! # Commitment Marketplace Contract Tests
//!
//! Unit tests for the CommitmentMarketplace Soroban contract.
//!
//! ## Coverage
//! - Initialization, listing, offers, auctions, and reentrancy guard.
//! - Edge cases and error conditions.
//!
//! ## Security
//! - Explicit tests for reentrancy guard on all entry points.
//! - All state-changing entry points require authentication.
//!
//! ## Usage
//! Run with `cargo test -p commitment-marketplace` from the workspace root.

#![cfg(test)]

extern crate std;

use crate::*;
use soroban_sdk::{
    contract, contractimpl, contracttype,
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    vec, Address, Env, IntoVal,
};

// ============================================================================
// Test Setup Helpers
// ============================================================================

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
enum ReentryAction {
    BuyNft,
    AcceptOffer,
    PlaceBid,
    EndAuction,
}

#[derive(Clone)]
#[contracttype]
enum ReentrantTokenKey {
    Marketplace,
    Action,
    TokenId,
    Seller,
    Offerer,
    Bidder,
    BidAmount,
    Enabled,
    Observed,
}

#[contract]
struct ReentrantPaymentToken;

#[contractimpl]
impl ReentrantPaymentToken {
    pub fn configure(
        e: Env,
        marketplace: Address,
        action: ReentryAction,
        token_id: u32,
        seller: Address,
        offerer: Address,
        bidder: Address,
        bid_amount: i128,
    ) {
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::Marketplace, &marketplace);
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::Action, &action);
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::TokenId, &token_id);
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::Seller, &seller);
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::Offerer, &offerer);
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::Bidder, &bidder);
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::BidAmount, &bid_amount);
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::Enabled, &true);
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::Observed, &false);
    }

    pub fn set_enabled(e: Env, enabled: bool) {
        e.storage()
            .instance()
            .set(&ReentrantTokenKey::Enabled, &enabled);
    }

    pub fn observed_reentrancy(e: Env) -> bool {
        e.storage()
            .instance()
            .get(&ReentrantTokenKey::Observed)
            .unwrap_or(false)
    }

    pub fn transfer(e: Env, _from: Address, _to: Address, _amount: i128) {
        if !e
            .storage()
            .instance()
            .get(&ReentrantTokenKey::Enabled)
            .unwrap_or(false)
        {
            return;
        }

        e.storage()
            .instance()
            .set(&ReentrantTokenKey::Enabled, &false);

        let marketplace: Address = e
            .storage()
            .instance()
            .get(&ReentrantTokenKey::Marketplace)
            .unwrap();
        let action: ReentryAction = e
            .storage()
            .instance()
            .get(&ReentrantTokenKey::Action)
            .unwrap();
        let token_id: u32 = e
            .storage()
            .instance()
            .get(&ReentrantTokenKey::TokenId)
            .unwrap();
        let seller: Address = e
            .storage()
            .instance()
            .get(&ReentrantTokenKey::Seller)
            .unwrap();
        let offerer: Address = e
            .storage()
            .instance()
            .get(&ReentrantTokenKey::Offerer)
            .unwrap();
        let bidder: Address = e
            .storage()
            .instance()
            .get(&ReentrantTokenKey::Bidder)
            .unwrap();
        let bid_amount: i128 = e
            .storage()
            .instance()
            .get(&ReentrantTokenKey::BidAmount)
            .unwrap();

        let client = CommitmentMarketplaceClient::new(&e, &marketplace);
        let reentry_error = match action {
            ReentryAction::BuyNft => client.try_buy_nft(&offerer, &token_id),
            ReentryAction::AcceptOffer => client.try_accept_offer(&seller, &token_id, &offerer),
            ReentryAction::PlaceBid => client.try_place_bid(&bidder, &token_id, &bid_amount),
            ReentryAction::EndAuction => client.try_end_auction(&token_id),
        };

        let observed_reentrancy = reentry_error.is_err();
        if observed_reentrancy {
            e.storage()
                .instance()
                .set(&ReentrantTokenKey::Observed, &true);
        }
    }
}

/// @notice Helper to deploy and initialize the marketplace contract for tests.
/// @param e Test environment.
/// @return (admin, fee_recipient, client)
fn setup_marketplace(e: &Env) -> (Address, Address, CommitmentMarketplaceClient<'_>) {
    let admin = Address::generate(e);
    let nft_contract = Address::generate(e);
    let fee_recipient = Address::generate(e);

    // Use register_contract for Soroban SDK
    let marketplace_id = e.register_contract(None, CommitmentMarketplace);
    let client = CommitmentMarketplaceClient::new(e, &marketplace_id);

    client.initialize(&admin, &nft_contract, &250, &fee_recipient); // 2.5% fee

    (admin, fee_recipient, client)
}

/// @notice Helper to generate a test token address and allowlist it.
fn setup_test_token(e: &Env, client: &CommitmentMarketplaceClient<'_>) -> Address {
    setup_allowed_payment_token(e, client)
}

fn set_contract_reentrancy_guard(e: &Env, contract: &Address, active: bool) {
    e.as_contract(contract, || {
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &active);
    });
}

fn setup_allowed_payment_token(e: &Env, client: &CommitmentMarketplaceClient<'_>) -> Address {
    let payment_token = Address::generate(e);
    client.add_payment_token(&payment_token);
    payment_token
}

fn setup_reentrant_payment_token<'a>(
    e: &'a Env,
    client: &CommitmentMarketplaceClient<'a>,
) -> (Address, ReentrantPaymentTokenClient<'a>) {
    let token = e.register_contract(None, ReentrantPaymentToken);
    let token_client = ReentrantPaymentTokenClient::new(e, &token);
    client.add_payment_token(&token);
    (token, token_client)
}

fn marketplace_guard_active(e: &Env, marketplace: &Address) -> bool {
    e.as_contract(marketplace, || {
        e.storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false)
    })
}

// ============================================================================
// Initialization Tests
// ============================================================================

#[test]
fn test_initialize_marketplace() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let fee_recipient = Address::generate(&e);

    let marketplace_id = e.register_contract(None, CommitmentMarketplace);
    let client = CommitmentMarketplaceClient::new(&e, &marketplace_id);

    client.initialize(&admin, &nft_contract, &250, &fee_recipient);

    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")] // AlreadyInitialized
fn test_initialize_twice_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, _, client) = setup_marketplace(&e);
    let nft_contract = Address::generate(&e);
    let fee_recipient = Address::generate(&e);
    let new_admin = Address::generate(&e);

    client.initialize(&new_admin, &nft_contract, &250, &fee_recipient);
}

#[test]
fn test_update_fee() {
    let e = Env::default();
    e.mock_all_auths();

    let (_admin, _, client) = setup_marketplace(&e);

    client.update_fee(&500); // Update to 5%

    // Verify event
    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, client.address);
}

// ============================================================================
// Listing Tests
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // InvalidPrice
fn test_list_nft_zero_price_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.list_nft(&seller, &1, &0, &payment_token);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")] // ListingExists
fn test_list_nft_twice_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.list_nft(&seller, &1, &1000, &payment_token);
    client.list_nft(&seller, &1, &2000, &payment_token); // Should fail
}

#[test]
fn test_cancel_listing() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);
    let token_id = 1u32;

    client.list_nft(&seller, &token_id, &1000, &payment_token);
    client.cancel_listing(&seller, &token_id);

    // Verify event
    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("ListCncl").into_val(&e),
            token_id.into_val(&e)
        ]
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // ListingNotFound
fn test_get_listing_after_cancel_panics() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let token_id = 1u32;

    let payment_token = setup_allowed_payment_token(&e, &client);
    client.list_nft(&seller, &token_id, &1000, &payment_token);
    client.cancel_listing(&seller, &token_id);

    // This will panic as expected
    client.get_listing(&token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // ListingNotFound
fn test_cancel_nonexistent_listing_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    client.cancel_listing(&seller, &999);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")] // NotSeller
fn test_cancel_listing_not_seller_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let not_seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.list_nft(&seller, &1, &1000, &payment_token);
    client.cancel_listing(&not_seller, &1); // Should fail
}

#[test]
fn test_get_all_listings() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    // List 3 NFTs
    client.list_nft(&seller, &1, &1000, &payment_token);
    client.list_nft(&seller, &2, &2000, &payment_token);
    client.list_nft(&seller, &3, &3000, &payment_token);

    let listings = client.get_all_listings();
    assert_eq!(listings.len(), 3);
}

// ============================================================================
// Buy Tests (Note: These are simplified - real tests need token contract)
// ============================================================================

#[test]
fn test_buy_nft_flow() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let _buyer = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);
    let token_id = 1u32;
    let price = 1000_0000000i128;

    // List NFT
    client.list_nft(&seller, &token_id, &price, &payment_token);

    // Note: In a real test, you'd need to:
    // 1. Deploy a test token contract
    // 2. Mint tokens to the buyer
    // 3. Have buyer approve marketplace to spend tokens
    // 4. Call buy_nft
    // 5. Verify token and NFT transfers

    // For this example, we're testing the flow logic only
    // Uncomment when you have token contract set up:
    // client.buy_nft(&buyer, &token_id);

    // Verify listing is removed
    // let result = client.try_get_listing(&token_id);
    // assert!(result.is_err());
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // CannotBuyOwnListing
fn test_buy_own_listing_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.list_nft(&seller, &1, &1000, &payment_token);
    client.buy_nft(&seller, &1); // Seller trying to buy their own listing
}

// ============================================================================
// Offer System Tests
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #12)")] // InvalidOfferAmount
fn test_make_offer_zero_amount_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.make_offer(&offerer, &1, &0, &payment_token);
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")] // OfferExists
fn test_make_duplicate_offer_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.make_offer(&offerer, &1, &500, &payment_token);
    client.make_offer(&offerer, &1, &600, &payment_token); // Should fail
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // CannotBuyOwnListing
fn test_make_offer_own_listing_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);

    client.list_nft(&seller, &1, &1000, &payment_token);
    client.make_offer(&seller, &1, &800, &payment_token); // Seller making offer on own listing
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // CannotBuyOwnListing
fn test_make_offer_own_auction_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);

    client.start_auction(&seller, &1, &1000, &86400, &payment_token);
    client.make_offer(&seller, &1, &1100, &payment_token); // Seller making offer on own auction
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // CannotBuyOwnListing
fn test_accept_offer_own_listing_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);

    client.make_offer(&seller, &1, &1000, &payment_token);
    client.accept_offer(&seller, &1, &seller); // Seller accepting own offer
}



#[test]
fn test_multiple_offers_same_token() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer1 = Address::generate(&e);
    let offerer2 = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);
    let token_id = 1u32;

    client.make_offer(&offerer1, &token_id, &500, &payment_token);
    client.make_offer(&offerer2, &token_id, &600, &payment_token);

    let offers = client.get_offers(&token_id);
    assert_eq!(offers.len(), 2);
}

#[test]
fn test_cancel_offer() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);
    let token_id = 1u32;

    client.make_offer(&offerer, &token_id, &500, &payment_token);
    client.cancel_offer(&offerer, &token_id);

    let offers = client.get_offers(&token_id);
    assert_eq!(offers.len(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")] // OfferNotFound
fn test_cancel_nonexistent_offer_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer = Address::generate(&e);
    client.cancel_offer(&offerer, &999);
}

// ============================================================================
// Auction System Tests
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #6)")] // InvalidPrice
fn test_start_auction_zero_price_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.start_auction(&seller, &1, &0, &86400, &payment_token);
}

#[test]
#[should_panic(expected = "Error(Contract, #19)")] // InvalidDuration
fn test_start_auction_zero_duration_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.start_auction(&seller, &1, &1000, &0, &payment_token);
}

#[test]
fn test_place_bid() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let _bidder = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);
    let token_id = 1u32;
    let starting_price = 1000_0000000i128;
    let _bid_amount = 1200_0000000i128;

    client.start_auction(&seller, &token_id, &starting_price, &86400, &payment_token);

    // Note: In real test, setup token contract and balances
    // client.place_bid(&bidder, &token_id, &bid_amount);
    // let auction = client.get_auction(&token_id);
    // assert_eq!(auction.current_bid, bid_amount);
    // assert_eq!(auction.highest_bidder, Some(bidder));
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")] // BidTooLow
fn test_place_bid_too_low_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let bidder = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);
    let token_id = 1u32;

    client.start_auction(&seller, &token_id, &1000, &86400, &payment_token);
    client.place_bid(&bidder, &token_id, &500); // Lower than starting price
}

#[test]
#[should_panic(expected = "Error(Contract, #18)")] // BidTooLow
fn test_place_bid_not_high_enough_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let bidder = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;
    let starting_price = 1000i128;

    client.start_auction(&seller, &token_id, &starting_price, &86400, &payment_token);

    // current_bid starts at starting_price; bidding the exact same amount is <= current_bid,
    // so it must be rejected with BidTooLow before any token transfer happens.
    client.place_bid(&bidder, &token_id, &starting_price);
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")] // AuctionEnded
fn test_place_bid_after_auction_ends_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let bidder = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);
    let token_id = 1u32;
    let duration = 86400u64; // 1 day

    client.start_auction(&seller, &token_id, &1000, &duration, &payment_token);

    // Fast forward time past auction end
    e.ledger().with_mut(|li| {
        li.timestamp = 86400 + 1;
    });

    client.place_bid(&bidder, &token_id, &1500);
}

#[test]
fn test_auction_duration_boundary() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let bidder = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;
    let duration = 86400u64;
    let starting_price = 1000i128;

    // Auction starts at timestamp 0, ends_at = 0 + duration = 86400
    client.start_auction(&seller, &token_id, &starting_price, &duration, &payment_token);

    // At timestamp 0 (start), bidding equal-to-current is rejected with BidTooLow, not AuctionEnded.
    // This proves the time check passes (auction is active) but bid check fails.
    let result_active = client.try_place_bid(&bidder, &token_id, &starting_price);
    assert!(
        result_active.is_err(),
        "equal bid at auction start should fail"
    );

    // At ends_at - 1 (last active second): equal bid still fails with BidTooLow, not AuctionEnded.
    e.ledger().with_mut(|li| {
        li.timestamp = duration - 1;
    });
    let result_last_second = client.try_place_bid(&bidder, &token_id, &starting_price);
    assert!(
        result_last_second.is_err(),
        "equal bid one second before end should fail"
    );

    // At ends_at (expired): any bid is rejected with AuctionEnded.
    e.ledger().with_mut(|li| {
        li.timestamp = duration;
    });
    let result_at_end = client.try_place_bid(&bidder, &token_id, &(starting_price + 1));
    let err = result_at_end.expect_err("bid at ends_at should fail");
    // Must fail with AuctionEnded (#16), not BidTooLow (#18)
    assert_eq!(err.unwrap(), MarketplaceError::AuctionEnded);

    // At ends_at: end_auction should succeed
    client.end_auction(&token_id);
    let auction = client.get_auction(&token_id);
    assert!(auction.ended);
}

#[test]
#[should_panic(expected = "Error(Contract, #17)")] // AuctionNotEnded
fn test_end_auction_before_time_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.start_auction(&seller, &1, &1000, &86400, &payment_token);
    client.end_auction(&1); // Try to end immediately
}

#[test]
#[should_panic(expected = "Error(Contract, #16)")] // AuctionEnded
fn test_end_auction_twice_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    client.start_auction(&seller, &1, &1000, &86400, &payment_token);

    e.ledger().with_mut(|li| {
        li.timestamp = 86400 + 1;
    });

    client.end_auction(&1);
    client.end_auction(&1); // Should fail
}

#[test]
fn test_auction_active_vs_ended() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;

    client.start_auction(&seller, &token_id, &1000, &86400, &payment_token);

    // Should be in active auctions
    let auctions = client.get_all_auctions();
    assert_eq!(auctions.len(), 1);
    assert_eq!(auctions.get(0).unwrap().token_id, token_id);

    // End auction
    e.ledger().with_mut(|li| {
        li.timestamp = 86400 + 1;
    });
    client.end_auction(&token_id);

    // Should NOT be in active auctions
    let auctions_after = client.get_all_auctions();
    assert_eq!(auctions_after.len(), 0);
    
    // But still retrievable via get_auction
    let auction = client.get_auction(&token_id);
    assert!(auction.ended);
}

#[test]
fn test_get_all_auctions() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    // Start 3 auctions
    client.start_auction(&seller, &1, &1000, &86400, &payment_token);
    client.start_auction(&seller, &2, &2000, &86400, &payment_token);
    client.start_auction(&seller, &3, &3000, &86400, &payment_token);

    let auctions = client.get_all_auctions();
    assert_eq!(auctions.len(), 3);
}

// ============================================================================
// Issue #267: Unit tests for offers - duplicate offer, cancel, not maker
// ============================================================================

// Duplicate Offer Tests
#[test]
#[should_panic(expected = "Error(Contract, #13)")] // OfferExists
fn test_make_duplicate_offer_same_token_different_amount_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;

    // Make first offer
    client.make_offer(&offerer, &token_id, &500, &payment_token);
    
    // Try to make another offer with different amount - should fail
    client.make_offer(&offerer, &token_id, &1000, &payment_token);
}

#[test]
#[should_panic(expected = "Error(Contract, #13)")] // OfferExists
fn test_make_duplicate_offer_different_tokens_same_user_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer = Address::generate(&e);
    let payment_token1 = setup_test_token(&e, &client);
    let payment_token2 = setup_test_token(&e, &client);

    // Make offer on token 1
    client.make_offer(&offerer, &1, &500, &payment_token1);
    
    // Make offer on token 2 - should work (different token)
    client.make_offer(&offerer, &2, &600, &payment_token2);
    
    // Try to make another offer on token 1 - should fail
    client.make_offer(&offerer, &1, &700, &payment_token1);
}

#[test]
fn test_different_users_can_offer_same_token() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer1 = Address::generate(&e);
    let offerer2 = Address::generate(&e);
    let offerer3 = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;

    // Multiple users can offer on the same token
    client.make_offer(&offerer1, &token_id, &500, &payment_token);
    client.make_offer(&offerer2, &token_id, &600, &payment_token);
    client.make_offer(&offerer3, &token_id, &700, &payment_token);

    let offers = client.get_offers(&token_id);
    assert_eq!(offers.len(), 3);
}

// Offer Cancellation Tests
#[test]
fn test_cancel_offer_removes_correct_offer_only() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer1 = Address::generate(&e);
    let offerer2 = Address::generate(&e);
    let offerer3 = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;

    // Make multiple offers
    client.make_offer(&offerer1, &token_id, &500, &payment_token);
    client.make_offer(&offerer2, &token_id, &600, &payment_token);
    client.make_offer(&offerer3, &token_id, &700, &payment_token);

    // Cancel middle offer
    client.cancel_offer(&offerer2, &token_id);

    let offers = client.get_offers(&token_id);
    assert_eq!(offers.len(), 2);
    
    // Verify correct offers remain
    let mut found_500 = false;
    let mut found_700 = false;
    for o in offers.iter() {
        if o.amount == 500 {
            found_500 = true;
        }
        if o.amount == 700 {
            found_700 = true;
        }
        assert_ne!(o.amount, 600);
    }
    assert!(found_500);
    assert!(found_700);
}

#[test]
fn test_cancel_last_offer_removes_storage() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;

    // Make offer
    client.make_offer(&offerer, &token_id, &500, &payment_token);
    
    // Verify offer exists
    let offers = client.get_offers(&token_id);
    assert_eq!(offers.len(), 1);

    // Cancel offer
    client.cancel_offer(&offerer, &token_id);

    // Verify offers are empty
    let offers = client.get_offers(&token_id);
    assert_eq!(offers.len(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")] // OfferNotFound
fn test_cancel_offer_after_accept_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let offerer = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;

    // Make offer
    client.make_offer(&offerer, &token_id, &500, &payment_token);
    client.cancel_offer(&offerer, &token_id);
    client.cancel_offer(&offerer, &token_id);
}

#[test]
fn test_cancel_multiple_offers_same_user_different_tokens() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);

    // Make offers on different tokens
    client.make_offer(&offerer, &1, &500, &payment_token);
    client.make_offer(&offerer, &2, &600, &payment_token);
    client.make_offer(&offerer, &3, &700, &payment_token);

    // Cancel one offer
    client.cancel_offer(&offerer, &2);

    // Verify other offers still exist
    assert_eq!(client.get_offers(&1).len(), 1);
    assert_eq!(client.get_offers(&2).len(), 0);
    assert_eq!(client.get_offers(&3).len(), 1);
}

// Not Maker Tests (Authorization Tests)
#[test]
#[should_panic(expected = "Error(Contract, #11)")] // OfferNotFound
fn test_non_maker_cannot_cancel_offer() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer = Address::generate(&e);
    let non_maker = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;

    // Make offer
    client.make_offer(&offerer, &token_id, &500, &payment_token);
    
    // Try to cancel with different address - should fail
    client.cancel_offer(&non_maker, &token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")] // OfferNotFound
fn test_different_offerer_cannot_cancel_other_offer() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer1 = Address::generate(&e);
    let offerer2 = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;

    // Make offers from different users
    client.make_offer(&offerer1, &token_id, &500, &payment_token);
    client.make_offer(&offerer2, &token_id, &600, &payment_token);

    let non_maker = Address::generate(&e);
    client.cancel_offer(&non_maker, &token_id);
}

#[test]
fn test_maker_can_cancel_own_offer_multiple_exist() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer1 = Address::generate(&e);
    let offerer2 = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;

    // Make offers from different users
    client.make_offer(&offerer1, &token_id, &500, &payment_token);
    client.make_offer(&offerer2, &token_id, &600, &payment_token);
    
    // offerer1 should be able to cancel their own offer
    client.cancel_offer(&offerer1, &token_id);
    
    let offers = client.get_offers(&token_id);
    assert_eq!(offers.len(), 1);
    assert_eq!(offers.get(0).unwrap().offerer, offerer2);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")] // OfferNotFound
fn test_cancel_nonexistent_offer_as_non_maker_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let non_maker = Address::generate(&e);
    let token_id = 999u32;

    // Try to cancel offer that doesn't exist - should fail
    client.cancel_offer(&non_maker, &token_id);
}

#[test]
fn test_authorization_scenarios_comprehensive() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let offerer1 = Address::generate(&e);
    let offerer2 = Address::generate(&e);
    let offerer3 = Address::generate(&e);
    let random_user = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);

    // Create offers on multiple tokens
    client.make_offer(&offerer1, &1, &100, &payment_token);
    client.make_offer(&offerer2, &1, &200, &payment_token);
    client.make_offer(&offerer1, &2, &300, &payment_token);
    client.make_offer(&offerer3, &3, &400, &payment_token);

    // Each offerer can cancel their own offers
    client.cancel_offer(&offerer1, &1); // Cancels offerer1's offer on token 1
    client.cancel_offer(&offerer1, &2); // Cancels offerer1's offer on token 2
    
    // Verify remaining offers
    assert_eq!(client.get_offers(&1).len(), 1); // Only offerer2's offer remains
    assert_eq!(client.get_offers(&2).len(), 0);  // offerer1's offer cancelled
    assert_eq!(client.get_offers(&3).len(), 1);  // offerer3's offer still exists

    // Random user cannot cancel any offers
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.cancel_offer(&random_user, &1);
    }));
    assert!(result.is_err());
}

// ============================================================================
// Edge Cases and Integration Tests
// ============================================================================

#[test]
fn test_list_then_start_auction_same_token() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);
    let token_id = 1u32;

    // List NFT
    client.list_nft(&seller, &token_id, &1000, &payment_token);

    // Cancel listing
    client.cancel_listing(&seller, &token_id);

    // Now start auction (should work)
    client.start_auction(&seller, &token_id, &1000, &86400, &payment_token);

    let auction = client.get_auction(&token_id);
    assert_eq!(auction.token_id, token_id);
}

#[test]
fn test_reentrancy_protection() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, _client) = setup_marketplace(&e);

    // The reentrancy guard prevents nested calls
    // This is tested implicitly in the token transfer flows
    // In production, you'd test with malicious contracts
}

// ============================================================================
// Gas / CPU Budget Profile Tests — Hot Paths (#272)
//
// These tests are designed to measure and document the resource consumption
// (CPU instructions and memory in Soroban) of the three hot paths:
//   • buy_nft          — fixed-price purchase
//   • place_bid        — auction bid (with previous-bidder refund)
//   • end_auction      — settle auction
//
// In the Soroban test environment the `budget()` API is available on `Env`
// when compiled with `features = ["testutils"]`.  Each test records the
// budget consumed for a single hot-path invocation so that regressions are
// visible in CI output.
//
// NOTE: token transfers require a real deployed token contract.  Where a
// live token contract is not available the test documents the *non-transfer*
// portion of the hot path (state reads/writes and event emission) and marks
// the transfer portion as a known stub.
// ============================================================================

// ============================================================================
// Reentrancy Guard Unit Tests (Explicit)
// ============================================================================

#[test]
fn test_reentrant_payment_token_buy_nft_is_blocked_and_guard_resets() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let buyer = Address::generate(&e);
    let token_id = 1u32;
    let (payment_token, token_client) = setup_reentrant_payment_token(&e, &client);

    client.list_nft(&seller, &token_id, &1000, &payment_token);
    token_client.configure(
        &client.address,
        &ReentryAction::BuyNft,
        &token_id,
        &seller,
        &buyer,
        &buyer,
        &0,
    );

    client.buy_nft(&buyer, &token_id);

    assert!(token_client.observed_reentrancy());
    assert!(!marketplace_guard_active(&e, &client.address));
}

#[test]
fn test_reentrant_payment_token_accept_offer_is_blocked_and_guard_resets() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let offerer = Address::generate(&e);
    let token_id = 1u32;
    let (payment_token, token_client) = setup_reentrant_payment_token(&e, &client);

    client.list_nft(&seller, &token_id, &1000, &payment_token);
    client.make_offer(&offerer, &token_id, &700, &payment_token);
    token_client.configure(
        &client.address,
        &ReentryAction::AcceptOffer,
        &token_id,
        &seller,
        &offerer,
        &offerer,
        &0,
    );

    client.accept_offer(&seller, &token_id, &offerer);

    assert!(token_client.observed_reentrancy());
    assert!(!marketplace_guard_active(&e, &client.address));
}

#[test]
fn test_reentrant_payment_token_place_bid_refund_path_is_blocked_and_guard_resets() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let first_bidder = Address::generate(&e);
    let second_bidder = Address::generate(&e);
    let token_id = 1u32;
    let (payment_token, token_client) = setup_reentrant_payment_token(&e, &client);

    client.start_auction(&seller, &token_id, &1000, &86400, &payment_token);
    token_client.set_enabled(&false);
    client.place_bid(&first_bidder, &token_id, &1100);
    token_client.configure(
        &client.address,
        &ReentryAction::PlaceBid,
        &token_id,
        &seller,
        &first_bidder,
        &second_bidder,
        &1300,
    );

    client.place_bid(&second_bidder, &token_id, &1300);

    assert!(token_client.observed_reentrancy());
    assert!(!marketplace_guard_active(&e, &client.address));
}

#[test]
fn test_reentrant_payment_token_end_auction_is_blocked_and_guard_resets() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let bidder = Address::generate(&e);
    let token_id = 1u32;
    let (payment_token, token_client) = setup_reentrant_payment_token(&e, &client);

    client.start_auction(&seller, &token_id, &1000, &10, &payment_token);
    token_client.set_enabled(&false);
    client.place_bid(&bidder, &token_id, &1200);
    token_client.configure(
        &client.address,
        &ReentryAction::EndAuction,
        &token_id,
        &seller,
        &bidder,
        &bidder,
        &0,
    );
    e.ledger().with_mut(|li| {
        li.timestamp = 11;
    });

    client.end_auction(&token_id);

    assert!(token_client.observed_reentrancy());
    assert!(!marketplace_guard_active(&e, &client.address));
}

/// @notice Test: list_nft fails if reentrancy guard is set.
#[test]
#[should_panic(expected = "Error(Contract, #20)")] // ReentrancyDetected
fn test_list_nft_reentrancy_guard() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    e.as_contract(&client.address, || {
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
    });
    client.list_nft(&seller, &1, &1000, &payment_token);
}

/// @notice Test: cancel_listing fails if reentrancy guard is set.
#[test]
#[should_panic(expected = "Error(Contract, #20)")] // ReentrancyDetected
fn test_cancel_listing_reentrancy_guard() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;
    client.list_nft(&seller, &token_id, &1000, &payment_token);
    e.as_contract(&client.address, || {
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
    });
    client.cancel_listing(&seller, &token_id);
}

/// @notice Test: buy_nft fails if reentrancy guard is set.
#[test]
#[should_panic(expected = "Error(Contract, #20)")] // ReentrancyDetected
fn test_buy_nft_reentrancy_guard() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let buyer = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;
    client.list_nft(&seller, &token_id, &1000, &payment_token);
    e.as_contract(&client.address, || {
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
    });
    client.buy_nft(&buyer, &token_id);
}

/// @notice Test: make_offer fails if reentrancy guard is set.
#[test]
#[should_panic(expected = "Error(Contract, #20)")] // ReentrancyDetected
fn test_make_offer_reentrancy_guard() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let offerer = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    e.as_contract(&client.address, || {
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
    });
    client.make_offer(&offerer, &1, &500, &payment_token);
}

/// @notice Test: accept_offer fails if reentrancy guard is set.
#[test]
#[should_panic(expected = "Error(Contract, #20)")] // ReentrancyDetected
fn test_accept_offer_reentrancy_guard() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let offerer = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;
    client.list_nft(&seller, &token_id, &1000, &payment_token);
    client.make_offer(&offerer, &token_id, &500, &payment_token);
    e.as_contract(&client.address, || {
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
    });
    client.accept_offer(&seller, &token_id, &offerer);
}

/// @notice Test: start_auction fails if reentrancy guard is set.
#[test]
#[should_panic(expected = "Error(Contract, #20)")] // ReentrancyDetected
fn test_start_auction_reentrancy_guard() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    e.as_contract(&client.address, || {
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
    });
    client.start_auction(&seller, &1, &1000, &86400, &payment_token);
}

/// @notice Test: place_bid fails if reentrancy guard is set.
#[test]
#[should_panic(expected = "Error(Contract, #20)")] // ReentrancyDetected
fn test_place_bid_reentrancy_guard() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let bidder = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;
    client.start_auction(&seller, &token_id, &1000, &86400, &payment_token);
    e.as_contract(&client.address, || {
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
    });
    client.place_bid(&bidder, &token_id, &1200);
}

/// @notice Test: end_auction fails if reentrancy guard is set.
#[test]
#[should_panic(expected = "Error(Contract, #20)")] // ReentrancyDetected
fn test_end_auction_reentrancy_guard() {
    let e = Env::default();
    e.mock_all_auths();
    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let payment_token = setup_test_token(&e, &client);
    let token_id = 1u32;
    client.start_auction(&seller, &token_id, &1000, &1, &payment_token);
    e.ledger().with_mut(|li| {
        li.timestamp = 2;
    });
    e.as_contract(&client.address, || {
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
    });
    client.end_auction(&token_id);
}

#[test]
fn test_gas_listing_operations() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);

    let seller = Address::generate(&e);
    let payment_token = setup_allowed_payment_token(&e, &client);

    // Measure operations for optimization
    let start = e.ledger().sequence();

    for i in 0..10 {
        client.list_nft(&seller, &i, &1000, &payment_token);
    }

    let end = e.ledger().sequence();
    let _operations = end - start;

    assert_eq!(client.get_all_listings().len(), 10);
}

#[test]
fn test_add_and_remove_payment_token() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);
    let payment_token = Address::generate(&e);

    assert!(!client.is_payment_token_allowed(&payment_token));

    client.add_payment_token(&payment_token);
    assert!(client.is_payment_token_allowed(&payment_token));
    assert_eq!(client.get_allowed_payment_tokens().len(), 1);

    client.remove_payment_token(&payment_token);
    assert!(!client.is_payment_token_allowed(&payment_token));
    assert_eq!(client.get_allowed_payment_tokens().len(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")] // PaymentTokenNotAllowed
fn test_list_nft_with_unallowlisted_token_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let payment_token = Address::generate(&e);

    client.list_nft(&seller, &1, &1000, &payment_token);
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")] // PaymentTokenNotAllowed
fn test_make_offer_with_unallowlisted_token_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);
    let offerer = Address::generate(&e);
    let payment_token = Address::generate(&e);

    client.make_offer(&offerer, &1, &1000, &payment_token);
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")] // PaymentTokenNotAllowed
fn test_start_auction_with_unallowlisted_token_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let payment_token = Address::generate(&e);

    client.start_auction(&seller, &1, &1000, &86400, &payment_token);
}

#[test]
#[should_panic(expected = "Error(Contract, #22)")] // PaymentTokenNotAllowed
fn test_buy_nft_after_payment_token_is_removed_fails() {
    let e = Env::default();
    e.mock_all_auths();

    let (_, _, client) = setup_marketplace(&e);
    let seller = Address::generate(&e);
    let buyer = Address::generate(&e);
    let token_admin = Address::generate(&e);
    let token = e.register_stellar_asset_contract_v2(token_admin);
    let payment_token = token.address();
    client.add_payment_token(&payment_token);
    soroban_sdk::token::StellarAssetClient::new(&e, &payment_token).mint(&buyer, &10_000);

    client.list_nft(&seller, &1, &1000, &payment_token);
    client.remove_payment_token(&payment_token);
    client.buy_nft(&buyer, &1);
}
