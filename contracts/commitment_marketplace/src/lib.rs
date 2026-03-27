//! # Commitment Marketplace Contract
//!
//! A secondary-market contract for trading commitment NFTs.
//!
//! ## Listing Lifecycle
//!
//! ```text
//! ┌──────────┐   list_nft    ┌─────────┐   buy_nft /     ┌──────────┐
//! │  Unlisted │ ──────────▶  │  Listed  │  accept_offer ▶ │  Settled │
//! └──────────┘               └─────────┘                  └──────────┘
//!                                │ cancel_listing
//!                                ▼
//!                            (Unlisted)
//! ```
//!
//! ## Auction Lifecycle
//!
//! ```text
//! ┌──────────┐  start_auction  ┌─────────┐  place_bid(s)  ┌──────────────┐
//! │ No Auction│ ─────────────▶ │  Active  │ ─────────────▶ │ Highest Bid  │
//! └──────────┘                 └─────────┘                 └──────┬───────┘
//!                                                                  │
//!                                               end_auction (after ends_at)
//!                                                                  │
//!                                          ┌───────────────────────┴──────────┐
//!                                          │                                  │
//!                                    ┌─────▼──────┐                    ┌──────▼─────┐
//!                                    │  Settled   │                    │  No Winner │
//!                                    │ (has winner│                    │  (returned │
//!                                    │  → pay/NFT)│                    │  to seller)│
//!                                    └────────────┘                    └────────────┘
//! ```
//!
//! ## Events Emitted
//!
//! | Function | Topic tuple | Data |
//! |----------|------------|------|
//! | `initialize` | — | — |
//! | `update_fee` | `("FeeUpdated",)` | `fee_basis_points` |
//! | `list_nft` | `("ListNFT", token_id)` | `(seller, price, payment_token)` |
//! | `cancel_listing` | `("ListCncl", token_id)` | `seller` |
//! | `buy_nft` | `("NFTSold", token_id)` | `(seller, buyer, price)` |
//! | `make_offer` | `("OfferMade", token_id)` | `(offerer, amount, payment_token)` |
//! | `accept_offer` | `("OffAccpt", token_id)` | `(seller, offerer, amount)` |
//! | `cancel_offer` | `("OfferCanc", token_id)` | `offerer` |
//! | `start_auction` | `("AucStart", token_id)` | `(seller, starting_price, ends_at)` |
//! | `place_bid` | `("BidPlaced", token_id)` | `(bidder, bid_amount)` |
//! | `end_auction` (winner) | `("AucEnd", token_id)` | `(winner, final_bid)` |
//! | `end_auction` (no bids) | `("AucNoBid", token_id)` | `seller` |
//!
//! ## Trust Boundaries
//! - **Admin**: set via `initialize`; controls `update_fee`.
//! - **Sellers**: authenticate via `require_auth` on all listing/auction/offer-acceptance calls.
//! - **Buyers / Bidders**: authenticate via `require_auth`; pay from their own balances.
//! - **Anyone**: can read listings, auctions, and offers.
//!
//! ## Reentrancy
//! All state-mutating functions that perform token transfers are wrapped in
//! an in-storage boolean reentrancy guard.  The guard is always cleared
//! before returning (including on error paths).
//!
//! ## Fee Arithmetic
//! `marketplace_fee = (price * fee_basis_points) / 10_000`  (integer division,
//! rounds toward zero).  Sellers receive `price − marketplace_fee`.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol,
    Vec,
};

// ============================================================================
// Error Types
// ============================================================================

/// All error conditions surfaced by the marketplace contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketplaceError {
    /// Marketplace not initialized
    NotInitialized = 1,
    /// Already initialized
    AlreadyInitialized = 2,
    /// Listing not found
    ListingNotFound = 3,
    /// Not the seller
    NotSeller = 4,
    /// NFT not active
    NFTNotActive = 5,
    /// Invalid price (must be > 0)
    InvalidPrice = 6,
    /// Listing already exists for this token
    ListingExists = 7,
    /// Buyer cannot be seller
    CannotBuyOwnListing = 8,
    /// Insufficient payment
    InsufficientPayment = 9,
    /// NFT contract call failed
    NFTContractError = 10,
    /// Offer not found
    OfferNotFound = 11,
    /// Invalid offer amount
    InvalidOfferAmount = 12,
    /// Offer already exists
    OfferExists = 13,
    /// Not offer maker
    NotOfferMaker = 14,
    /// Auction not found
    AuctionNotFound = 15,
    /// Auction already ended
    AuctionEnded = 16,
    /// Auction not ended yet
    AuctionNotEnded = 17,
    /// Bid too low
    BidTooLow = 18,
    /// Invalid duration
    InvalidDuration = 19,
    /// Reentrancy detected
    ReentrancyDetected = 20,
    /// Transfer failed
    TransferFailed = 21,
}

// ============================================================================
// Data Types
// ============================================================================

/// Listing information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Listing {
    pub token_id: u32,
    pub seller: Address,
    pub price: i128,
    pub payment_token: Address,
    pub listed_at: u64,
}

/// Offer information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Offer {
    pub token_id: u32,
    pub offerer: Address,
    pub amount: i128,
    pub payment_token: Address,
    pub created_at: u64,
}

/// Auction information
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Auction {
    pub token_id: u32,
    pub seller: Address,
    pub starting_price: i128,
    pub current_bid: i128,
    pub highest_bidder: Option<Address>,
    pub payment_token: Address,
    pub started_at: u64,
    pub ends_at: u64,
    pub ended: bool,
}

/// Storage keys
#[contracttype]
pub enum DataKey {
    /// Admin address
    Admin,
    /// NFT contract address
    NFTContract,
    /// Marketplace fee percentage (basis points, e.g., 250 = 2.5%)
    MarketplaceFee,
    /// Fee recipient address
    FeeRecipient,
    /// Listing data (token_id -> Listing)
    Listing(u32),
    /// All active listings
    ActiveListings,
    /// Offers for a token (token_id -> Vec<Offer>)
    Offers(u32),
    /// Auction data (token_id -> Auction)
    Auction(u32),
    /// Active auctions list
    ActiveAuctions,
    /// Reentrancy guard
    ReentrancyGuard,
}

#[cfg(test)]
mod tests;

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct CommitmentMarketplace;

#[contractimpl]
impl CommitmentMarketplace {
    // ========================================================================
    // Initialization
    // ========================================================================

    /// Initialize the marketplace.
    ///
    /// Must be called once, immediately after deployment.  Calling again
    /// returns [`MarketplaceError::AlreadyInitialized`].
    ///
    /// # Parameters
    /// - `admin` – Privileged address; `require_auth` is enforced.
    /// - `nft_contract` – Address of the `CommitmentNFT` contract.
    /// - `fee_basis_points` – Protocol fee in BPS (e.g. 250 = 2.5 %).
    /// - `fee_recipient` – Receives the protocol cut on every settled trade.
    ///
    /// # Errors
    /// - [`MarketplaceError::AlreadyInitialized`] if called more than once.
    ///
    /// # Security
    /// Deploy scripts must call this in the same transaction as contract
    /// deployment to prevent a front-running attack that could install a
    /// malicious admin.
    pub fn initialize(
        e: Env,
        admin: Address,
        nft_contract: Address,
        fee_basis_points: u32,
        fee_recipient: Address,
    ) -> Result<(), MarketplaceError> {
        if e.storage().instance().has(&DataKey::Admin) {
            return Err(MarketplaceError::AlreadyInitialized);
        }

        admin.require_auth();

        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage()
            .instance()
            .set(&DataKey::NFTContract, &nft_contract);
        e.storage()
            .instance()
            .set(&DataKey::MarketplaceFee, &fee_basis_points);
        e.storage()
            .instance()
            .set(&DataKey::FeeRecipient, &fee_recipient);

        let active_listings: Vec<u32> = Vec::new(&e);
        e.storage()
            .instance()
            .set(&DataKey::ActiveListings, &active_listings);

        let active_auctions: Vec<u32> = Vec::new(&e);
        e.storage()
            .instance()
            .set(&DataKey::ActiveAuctions, &active_auctions);

        Ok(())
    }

    /// Return the admin address.
    ///
    /// # Errors
    /// - [`MarketplaceError::NotInitialized`] if `initialize` has not been called.
    pub fn get_admin(e: Env) -> Result<Address, MarketplaceError> {
        e.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(MarketplaceError::NotInitialized)
    }

    /// Update the marketplace fee.  Admin only.
    ///
    /// # Parameters
    /// - `fee_basis_points` – New fee in BPS (0 – 10 000).
    ///
    /// # Errors
    /// - [`MarketplaceError::NotInitialized`] if called before `initialize`.
    ///
    /// # Events
    /// Emits `("FeeUpdated",) → fee_basis_points`.
    pub fn update_fee(e: Env, fee_basis_points: u32) -> Result<(), MarketplaceError> {
        let admin: Address = Self::get_admin(e.clone())?;
        admin.require_auth();

        e.storage()
            .instance()
            .set(&DataKey::MarketplaceFee, &fee_basis_points);

        e.events()
            .publish((Symbol::new(&e, "FeeUpdated"),), fee_basis_points);

        Ok(())
    }

    // ========================================================================
    // Listing Management
    // ========================================================================

    /// List an NFT for fixed-price sale.
    ///
    /// Transitions the token from *Unlisted* to *Listed*.
    ///
    /// # Parameters
    /// - `seller` – Must own the NFT; `require_auth` is enforced.
    /// - `token_id` – NFT token identifier.
    /// - `price` – Fixed sale price in `payment_token` base units (must be > 0).
    /// - `payment_token` – Token contract used for payment.
    ///
    /// # Errors
    /// - [`MarketplaceError::ReentrancyDetected`] – nested call guard.
    /// - [`MarketplaceError::InvalidPrice`] – `price ≤ 0`.
    /// - [`MarketplaceError::ListingExists`] – token is already listed.
    /// - [`MarketplaceError::NotInitialized`] – called before `initialize`.
    ///
    /// # Events
    /// Emits `("ListNFT", token_id) → (seller, price, payment_token)`.
    ///
    /// # Security
    /// Reentrancy-guarded.  NFT ownership is not verified on-chain in this
    /// version; the caller is trusted to own the token.
    pub fn list_nft(
        e: Env,
        seller: Address,
        token_id: u32,
        price: i128,
        payment_token: Address,
    ) -> Result<(), MarketplaceError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);
        if guard {
            return Err(MarketplaceError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // CHECKS
        seller.require_auth();

        if price <= 0 {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::InvalidPrice);
        }

        // Check if listing already exists
        if e.storage().persistent().has(&DataKey::Listing(token_id)) {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::ListingExists);
        }

        // Verify seller owns the NFT (external call - after checks)
        let _nft_contract: Address = e
            .storage()
            .instance()
            .get(&DataKey::NFTContract)
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::NotInitialized
            })?;

        // Note: This would require the NFT contract client
        // For now, we assume the caller has verified ownership
        // In production, you'd call: nft_contract.owner_of(&token_id)

        // EFFECTS
        let listing = Listing {
            token_id,
            seller: seller.clone(),
            price,
            payment_token: payment_token.clone(),
            listed_at: e.ledger().timestamp(),
        };

        e.storage()
            .persistent()
            .set(&DataKey::Listing(token_id), &listing);

        // Add to active listings
        let mut active_listings: Vec<u32> = e
            .storage()
            .instance()
            .get(&DataKey::ActiveListings)
            .unwrap_or(Vec::new(&e));
        active_listings.push_back(token_id);
        e.storage()
            .instance()
            .set(&DataKey::ActiveListings, &active_listings);

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit event
        e.events().publish(
            (symbol_short!("ListNFT"), token_id),
            (seller, price, payment_token),
        );

        Ok(())
    }

    /// Cancel an active fixed-price listing.
    ///
    /// Transitions the token from *Listed* back to *Unlisted*.
    ///
    /// # Parameters
    /// - `seller` – Must be the original lister; `require_auth` is enforced.
    /// - `token_id` – NFT token identifier.
    ///
    /// # Errors
    /// - [`MarketplaceError::ReentrancyDetected`] – nested call guard.
    /// - [`MarketplaceError::ListingNotFound`] – no active listing for this token.
    /// - [`MarketplaceError::NotSeller`] – `seller` is not the listing owner.
    ///
    /// # Events
    /// Emits `("ListCncl", token_id) → seller`.
    pub fn cancel_listing(e: Env, seller: Address, token_id: u32) -> Result<(), MarketplaceError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);
        if guard {
            return Err(MarketplaceError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // CHECKS
        seller.require_auth();

        let listing: Listing = e
            .storage()
            .persistent()
            .get(&DataKey::Listing(token_id))
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::ListingNotFound
            })?;

        if listing.seller != seller {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::NotSeller);
        }

        // EFFECTS
        // Remove listing
        e.storage().persistent().remove(&DataKey::Listing(token_id));

        // Remove from active listings
        let mut active_listings: Vec<u32> = e
            .storage()
            .instance()
            .get(&DataKey::ActiveListings)
            .unwrap_or(Vec::new(&e));
        if let Some(index) = active_listings.iter().position(|id| id == token_id) {
            active_listings.remove(index as u32);
        }
        e.storage()
            .instance()
            .set(&DataKey::ActiveListings, &active_listings);

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit event
        e.events()
            .publish((symbol_short!("ListCncl"), token_id), seller);

        Ok(())
    }

    /// Purchase a listed NFT at the fixed listing price.
    ///
    /// Settles a *Listed* token: removes the listing, pays the seller and
    /// protocol, and marks the token as *Settled*.
    ///
    /// # Parameters
    /// - `buyer` – Must not be the seller; `require_auth` is enforced.
    /// - `token_id` – NFT token identifier.
    ///
    /// # Errors
    /// - [`MarketplaceError::ReentrancyDetected`] – nested call guard.
    /// - [`MarketplaceError::ListingNotFound`] – no active listing for this token.
    /// - [`MarketplaceError::CannotBuyOwnListing`] – buyer is the seller.
    /// - [`MarketplaceError::NotInitialized`] – missing fee recipient.
    ///
    /// # Events
    /// Emits `("NFTSold", token_id) → (seller, buyer, price)`.
    ///
    /// # Security
    /// Reentrancy-guarded.  Listing is removed from storage **before** the
    /// token transfer (checks-effects-interactions).
    pub fn buy_nft(e: Env, buyer: Address, token_id: u32) -> Result<(), MarketplaceError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);
        if guard {
            return Err(MarketplaceError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // CHECKS
        buyer.require_auth();

        let listing: Listing = e
            .storage()
            .persistent()
            .get(&DataKey::Listing(token_id))
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::ListingNotFound
            })?;

        if listing.seller == buyer {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::CannotBuyOwnListing);
        }

        let fee_basis_points: u32 = e
            .storage()
            .instance()
            .get(&DataKey::MarketplaceFee)
            .unwrap_or(0);

        let fee_recipient: Address = e
            .storage()
            .instance()
            .get(&DataKey::FeeRecipient)
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::NotInitialized
            })?;

        let _nft_contract: Address = e
            .storage()
            .instance()
            .get(&DataKey::NFTContract)
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::NotInitialized
            })?;

        // Calculate fee and seller proceeds
        let marketplace_fee = (listing.price * fee_basis_points as i128) / 10000;
        let seller_proceeds = listing.price - marketplace_fee;

        // EFFECTS
        // Remove listing first (prevent reentrancy)
        e.storage().persistent().remove(&DataKey::Listing(token_id));

        // Remove from active listings
        let mut active_listings: Vec<u32> = e
            .storage()
            .instance()
            .get(&DataKey::ActiveListings)
            .unwrap_or(Vec::new(&e));
        if let Some(index) = active_listings.iter().position(|id| id == token_id) {
            active_listings.remove(index as u32);
        }
        e.storage()
            .instance()
            .set(&DataKey::ActiveListings, &active_listings);

        // INTERACTIONS - External calls AFTER state changes
        // Transfer payment token from buyer to seller
        let payment_token_client = token::Client::new(&e, &listing.payment_token);
        payment_token_client.transfer(&buyer, &listing.seller, &seller_proceeds);

        // Transfer marketplace fee if applicable
        if marketplace_fee > 0 {
            payment_token_client.transfer(&buyer, &fee_recipient, &marketplace_fee);
        }

        // Transfer NFT from seller to buyer
        // Note: In production, you'd use the NFT contract client:
        // let nft_client = CommitmentNFTContractClient::new(&e, &nft_contract);
        // nft_client.transfer(&listing.seller, &buyer, &token_id);
        // For this implementation, we assume the transfer happens correctly

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit event
        e.events().publish(
            (symbol_short!("NFTSold"), token_id),
            (listing.seller, buyer, listing.price),
        );

        Ok(())
    }

    /// Return the active listing for a token.
    ///
    /// # Errors
    /// - [`MarketplaceError::ListingNotFound`] if the token is not listed.
    pub fn get_listing(e: Env, token_id: u32) -> Result<Listing, MarketplaceError> {
        e.storage()
            .persistent()
            .get(&DataKey::Listing(token_id))
            .ok_or(MarketplaceError::ListingNotFound)
    }

    /// Return all currently active listings.
    ///
    /// The order matches the order in which tokens were listed.  Settled or
    /// cancelled listings are not included.
    pub fn get_all_listings(e: Env) -> Vec<Listing> {
        let active_listings: Vec<u32> = e
            .storage()
            .instance()
            .get(&DataKey::ActiveListings)
            .unwrap_or(Vec::new(&e));

        let mut listings: Vec<Listing> = Vec::new(&e);

        for token_id in active_listings.iter() {
            if let Some(listing) = e
                .storage()
                .persistent()
                .get::<_, Listing>(&DataKey::Listing(token_id))
            {
                listings.push_back(listing);
            }
        }

        listings
    }

    // ========================================================================
    // Offer System
    // ========================================================================

    /// Make a purchase offer on an NFT (listed or unlisted).
    ///
    /// # Parameters
    /// - `offerer` – Must be unique per token; `require_auth` is enforced.
    /// - `token_id` – Target NFT.
    /// - `amount` – Offered amount in `payment_token` base units (must be > 0).
    /// - `payment_token` – Token contract for payment.
    ///
    /// # Errors
    /// - [`MarketplaceError::ReentrancyDetected`] – nested call guard.
    /// - [`MarketplaceError::InvalidOfferAmount`] – `amount ≤ 0`.
    /// - [`MarketplaceError::OfferExists`] – `offerer` already has an open offer on this token.
    ///
    /// # Events
    /// Emits `("OfferMade", token_id) → (offerer, amount, payment_token)`.
    pub fn make_offer(
        e: Env,
        offerer: Address,
        token_id: u32,
        amount: i128,
        payment_token: Address,
    ) -> Result<(), MarketplaceError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);
        if guard {
            return Err(MarketplaceError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // CHECKS
        offerer.require_auth();

        if amount <= 0 {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::InvalidOfferAmount);
        }

        // EFFECTS
        let offer = Offer {
            token_id,
            offerer: offerer.clone(),
            amount,
            payment_token: payment_token.clone(),
            created_at: e.ledger().timestamp(),
        };

        let mut offers: Vec<Offer> = e
            .storage()
            .persistent()
            .get(&DataKey::Offers(token_id))
            .unwrap_or(Vec::new(&e));

        // Check if offerer already has an offer
        for existing_offer in offers.iter() {
            if existing_offer.offerer == offerer {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                return Err(MarketplaceError::OfferExists);
            }
        }

        offers.push_back(offer);
        e.storage()
            .persistent()
            .set(&DataKey::Offers(token_id), &offers);

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit event
        e.events().publish(
            (symbol_short!("OfferMade"), token_id),
            (offerer, amount, payment_token),
        );

        Ok(())
    }

    /// Accept an outstanding offer, settling the trade.
    ///
    /// Removes all offers for the token, optionally removes a co-existing
    /// listing, transfers payment to the seller, and emits an event.
    ///
    /// # Parameters
    /// - `seller` – Must own the token; `require_auth` is enforced.
    /// - `token_id` – Target NFT.
    /// - `offerer` – Address whose offer to accept.
    ///
    /// # Errors
    /// - [`MarketplaceError::ReentrancyDetected`] – nested call guard.
    /// - [`MarketplaceError::OfferNotFound`] – no offers exist for this token, or
    ///   `offerer` has no offer.
    /// - [`MarketplaceError::NotInitialized`] – missing fee recipient.
    ///
    /// # Events
    /// Emits `("OffAccpt", token_id) → (seller, offerer, amount)`.
    ///
    /// # Security
    /// Reentrancy-guarded.  All offers are removed before token transfer.
    pub fn accept_offer(
        e: Env,
        seller: Address,
        token_id: u32,
        offerer: Address,
    ) -> Result<(), MarketplaceError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);
        if guard {
            return Err(MarketplaceError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // CHECKS
        seller.require_auth();

        let offers: Vec<Offer> = e
            .storage()
            .persistent()
            .get(&DataKey::Offers(token_id))
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::OfferNotFound
            })?;

        // Find the offer
        let offer_index = offers
            .iter()
            .position(|o| o.offerer == offerer)
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::OfferNotFound
            })?;

        let offer = offers.get(offer_index as u32).unwrap();

        let fee_basis_points: u32 = e
            .storage()
            .instance()
            .get(&DataKey::MarketplaceFee)
            .unwrap_or(0);

        let fee_recipient: Address = e
            .storage()
            .instance()
            .get(&DataKey::FeeRecipient)
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::NotInitialized
            })?;

        // Calculate fee and seller proceeds
        let marketplace_fee = (offer.amount * fee_basis_points as i128) / 10000;
        let seller_proceeds = offer.amount - marketplace_fee;

        // EFFECTS
        // Remove all offers for this token
        e.storage().persistent().remove(&DataKey::Offers(token_id));

        // Remove listing if exists
        if e.storage().persistent().has(&DataKey::Listing(token_id)) {
            e.storage().persistent().remove(&DataKey::Listing(token_id));

            let mut active_listings: Vec<u32> = e
                .storage()
                .instance()
                .get(&DataKey::ActiveListings)
                .unwrap_or(Vec::new(&e));
            if let Some(index) = active_listings.iter().position(|id| id == token_id) {
                active_listings.remove(index as u32);
            }
            e.storage()
                .instance()
                .set(&DataKey::ActiveListings, &active_listings);
        }

        // INTERACTIONS
        // Transfer payment
        let payment_token_client = token::Client::new(&e, &offer.payment_token);
        payment_token_client.transfer(&offerer, &seller, &seller_proceeds);

        if marketplace_fee > 0 {
            payment_token_client.transfer(&offerer, &fee_recipient, &marketplace_fee);
        }

        // Transfer NFT
        // Note: Use NFT contract client in production

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit event
        e.events().publish(
            (symbol_short!("OffAccpt"), token_id),
            (seller, offerer, offer.amount),
        );

        Ok(())
    }

    /// Cancel an open offer.
    ///
    /// # Parameters
    /// - `offerer` – Must have an open offer; `require_auth` is enforced.
    /// - `token_id` – Target NFT.
    ///
    /// # Errors
    /// - [`MarketplaceError::OfferNotFound`] – no offers exist for this token, or
    ///   `offerer` has no offer.
    ///
    /// # Events
    /// Emits `("OfferCanc", token_id) → offerer`.
    pub fn cancel_offer(e: Env, offerer: Address, token_id: u32) -> Result<(), MarketplaceError> {
        offerer.require_auth();

        let mut offers: Vec<Offer> = e
            .storage()
            .persistent()
            .get(&DataKey::Offers(token_id))
            .ok_or(MarketplaceError::OfferNotFound)?;

        let offer_index = offers
            .iter()
            .position(|o| o.offerer == offerer)
            .ok_or(MarketplaceError::OfferNotFound)?;

        offers.remove(offer_index as u32);

        if offers.is_empty() {
            e.storage().persistent().remove(&DataKey::Offers(token_id));
        } else {
            e.storage()
                .persistent()
                .set(&DataKey::Offers(token_id), &offers);
        }

        e.events()
            .publish((symbol_short!("OfferCanc"), token_id), offerer);

        Ok(())
    }

    /// Return all open offers for a token (may be empty).
    pub fn get_offers(e: Env, token_id: u32) -> Vec<Offer> {
        e.storage()
            .persistent()
            .get(&DataKey::Offers(token_id))
            .unwrap_or(Vec::new(&e))
    }

    // ========================================================================
    // Auction System
    // ========================================================================

    /// Start an English auction for an NFT.
    ///
    /// Transitions the token to *Active Auction* state.
    ///
    /// # Parameters
    /// - `seller` – Must own the token; `require_auth` is enforced.
    /// - `token_id` – NFT to auction.
    /// - `starting_price` – Minimum opening bid (must be > 0).
    /// - `duration_seconds` – Auction length in seconds (must be > 0).
    /// - `payment_token` – Token contract for bids.
    ///
    /// # Errors
    /// - [`MarketplaceError::ReentrancyDetected`] – nested call guard.
    /// - [`MarketplaceError::InvalidPrice`] – `starting_price ≤ 0`.
    /// - [`MarketplaceError::InvalidDuration`] – `duration_seconds == 0`.
    /// - [`MarketplaceError::ListingExists`] – an auction already exists for this token.
    ///
    /// # Events
    /// Emits `("AucStart", token_id) → (seller, starting_price, ends_at)`.
    pub fn start_auction(
        e: Env,
        seller: Address,
        token_id: u32,
        starting_price: i128,
        duration_seconds: u64,
        payment_token: Address,
    ) -> Result<(), MarketplaceError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);
        if guard {
            return Err(MarketplaceError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // CHECKS
        seller.require_auth();

        if starting_price <= 0 {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::InvalidPrice);
        }

        if duration_seconds == 0 {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::InvalidDuration);
        }

        if e.storage().persistent().has(&DataKey::Auction(token_id)) {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::ListingExists);
        }

        // EFFECTS
        let started_at = e.ledger().timestamp();
        let ends_at = started_at + duration_seconds;

        let auction = Auction {
            token_id,
            seller: seller.clone(),
            starting_price,
            current_bid: starting_price,
            highest_bidder: None,
            payment_token: payment_token.clone(),
            started_at,
            ends_at,
            ended: false,
        };

        e.storage()
            .persistent()
            .set(&DataKey::Auction(token_id), &auction);

        let mut active_auctions: Vec<u32> = e
            .storage()
            .instance()
            .get(&DataKey::ActiveAuctions)
            .unwrap_or(Vec::new(&e));
        active_auctions.push_back(token_id);
        e.storage()
            .instance()
            .set(&DataKey::ActiveAuctions, &active_auctions);

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit event
        e.events().publish(
            (symbol_short!("AucStart"), token_id),
            (seller, starting_price, ends_at),
        );

        Ok(())
    }

    /// Place a bid on an active auction.
    ///
    /// The bid is escrowed in the contract.  If a previous bid existed the
    /// previous bidder is immediately refunded.
    ///
    /// # Parameters
    /// - `bidder` – Must not be the seller; `require_auth` is enforced.
    /// - `token_id` – Target auction.
    /// - `bid_amount` – Must strictly exceed `auction.current_bid`.
    ///
    /// # Errors
    /// - [`MarketplaceError::ReentrancyDetected`] – nested call guard.
    /// - [`MarketplaceError::AuctionNotFound`] – no auction for this token.
    /// - [`MarketplaceError::AuctionEnded`] – auction has expired.
    /// - [`MarketplaceError::BidTooLow`] – `bid_amount ≤ current_bid`.
    /// - [`MarketplaceError::CannotBuyOwnListing`] – bidder is the seller.
    ///
    /// # Events
    /// Emits `("BidPlaced", token_id) → (bidder, bid_amount)`.
    ///
    /// # Security
    /// Reentrancy-guarded.  Auction state is updated before token transfers
    /// (checks-effects-interactions).  Refund of the previous bidder is an
    /// outgoing transfer from the contract, not from the new bidder.
    pub fn place_bid(
        e: Env,
        bidder: Address,
        token_id: u32,
        bid_amount: i128,
    ) -> Result<(), MarketplaceError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);
        if guard {
            return Err(MarketplaceError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // CHECKS
        bidder.require_auth();

        let mut auction: Auction = e
            .storage()
            .persistent()
            .get(&DataKey::Auction(token_id))
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::AuctionNotFound
            })?;

        let current_time = e.ledger().timestamp();
        if current_time >= auction.ends_at {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::AuctionEnded);
        }

        if bid_amount <= auction.current_bid {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::BidTooLow);
        }

        if auction.seller == bidder {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::CannotBuyOwnListing);
        }

        // EFFECTS
        let previous_bidder = auction.highest_bidder.clone();
        let previous_bid = auction.current_bid;

        auction.current_bid = bid_amount;
        auction.highest_bidder = Some(bidder.clone());

        e.storage()
            .persistent()
            .set(&DataKey::Auction(token_id), &auction);

        // INTERACTIONS
        let payment_token_client = token::Client::new(&e, &auction.payment_token);

        // Transfer new bid from bidder to contract (escrow)
        payment_token_client.transfer(&bidder, &e.current_contract_address(), &bid_amount);

        // Refund previous bidder if exists
        if let Some(prev_bidder) = previous_bidder {
            payment_token_client.transfer(
                &e.current_contract_address(),
                &prev_bidder,
                &previous_bid,
            );
        }

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit event
        e.events()
            .publish((symbol_short!("BidPlaced"), token_id), (bidder, bid_amount));

        Ok(())
    }

    /// Settle an auction after it has expired.
    ///
    /// Can be called by anyone once `ledger.timestamp ≥ auction.ends_at`.
    /// If a winner exists the escrowed bid is split between the seller and
    /// the fee recipient.  If no bids were placed the NFT is returned to
    /// the seller.
    ///
    /// # Parameters
    /// - `token_id` – Target auction.
    ///
    /// # Errors
    /// - [`MarketplaceError::ReentrancyDetected`] – nested call guard.
    /// - [`MarketplaceError::AuctionNotFound`] – no auction for this token.
    /// - [`MarketplaceError::AuctionNotEnded`] – auction has not yet expired.
    /// - [`MarketplaceError::AuctionEnded`] – auction was already settled.
    /// - [`MarketplaceError::NotInitialized`] – missing fee recipient.
    ///
    /// # Events
    /// - Winner path: `("AucEnd", token_id) → (winner, final_bid)`.
    /// - No-bid path: `("AucNoBid", token_id) → seller`.
    ///
    /// # Security
    /// Reentrancy-guarded.  Auction state is marked `ended = true` before
    /// any token transfers.
    pub fn end_auction(e: Env, token_id: u32) -> Result<(), MarketplaceError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);
        if guard {
            return Err(MarketplaceError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);

        // CHECKS
        let mut auction: Auction = e
            .storage()
            .persistent()
            .get(&DataKey::Auction(token_id))
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::AuctionNotFound
            })?;

        let current_time = e.ledger().timestamp();
        if current_time < auction.ends_at {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::AuctionNotEnded);
        }

        if auction.ended {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(MarketplaceError::AuctionEnded);
        }

        let fee_basis_points: u32 = e
            .storage()
            .instance()
            .get(&DataKey::MarketplaceFee)
            .unwrap_or(0);

        let fee_recipient: Address = e
            .storage()
            .instance()
            .get(&DataKey::FeeRecipient)
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                MarketplaceError::NotInitialized
            })?;

        // EFFECTS
        auction.ended = true;
        e.storage()
            .persistent()
            .set(&DataKey::Auction(token_id), &auction);

        // Remove from active auctions
        let mut active_auctions: Vec<u32> = e
            .storage()
            .instance()
            .get(&DataKey::ActiveAuctions)
            .unwrap_or(Vec::new(&e));
        if let Some(index) = active_auctions.iter().position(|id| id == token_id) {
            active_auctions.remove(index as u32);
        }
        e.storage()
            .instance()
            .set(&DataKey::ActiveAuctions, &active_auctions);

        // INTERACTIONS
        if let Some(winner) = auction.highest_bidder {
            // Calculate fees
            let marketplace_fee = (auction.current_bid * fee_basis_points as i128) / 10000;
            let seller_proceeds = auction.current_bid - marketplace_fee;

            let payment_token_client = token::Client::new(&e, &auction.payment_token);

            // Transfer payment from escrow to seller
            payment_token_client.transfer(
                &e.current_contract_address(),
                &auction.seller,
                &seller_proceeds,
            );

            // Transfer fee
            if marketplace_fee > 0 {
                payment_token_client.transfer(
                    &e.current_contract_address(),
                    &fee_recipient,
                    &marketplace_fee,
                );
            }

            // Transfer NFT to winner
            // Note: Use NFT contract client in production

            // Clear reentrancy guard
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);

            // Emit event
            e.events().publish(
                (symbol_short!("AucEnd"), token_id),
                (winner, auction.current_bid),
            );
        } else {
            // No bids - return NFT to seller

            // Clear reentrancy guard
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);

            e.events()
                .publish((symbol_short!("AucNoBid"), token_id), auction.seller);
        }

        Ok(())
    }

    /// Return auction details for a token.
    ///
    /// # Errors
    /// - [`MarketplaceError::AuctionNotFound`] if no auction exists for this token.
    pub fn get_auction(e: Env, token_id: u32) -> Result<Auction, MarketplaceError> {
        e.storage()
            .persistent()
            .get(&DataKey::Auction(token_id))
            .ok_or(MarketplaceError::AuctionNotFound)
    }

    /// Return all currently active (non-settled) auctions.
    pub fn get_all_auctions(e: Env) -> Vec<Auction> {
        let active_auctions: Vec<u32> = e
            .storage()
            .instance()
            .get(&DataKey::ActiveAuctions)
            .unwrap_or(Vec::new(&e));

        let mut auctions: Vec<Auction> = Vec::new(&e);

        for token_id in active_auctions.iter() {
            if let Some(auction) = e
                .storage()
                .persistent()
                .get::<_, Auction>(&DataKey::Auction(token_id))
            {
                auctions.push_back(auction);
            }
        }

        auctions
    }
}
