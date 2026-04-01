# Commitment Marketplace — Listing Lifecycle & Events

> **Contract**: `contracts/commitment_marketplace/src/lib.rs`  
> **Package name**: `commitment-marketplace`

---

## 1. Overview

The `CommitmentMarketplace` contract provides three independent trading
mechanisms for commitment NFTs:

| Mechanism | Description |
|-----------|-------------|
| **Fixed-price listing** | Seller sets a price; any buyer can purchase immediately |
| **Offer system** | Buyers propose prices; the seller accepts whichever offer they prefer |
| **English auction** | Seller sets a starting price; bidding is open until `ends_at`; highest bid wins |

All three mechanisms share the same pool of token IDs.  A token can be
simultaneously listed *and* have open offers, but it cannot have both a
listing and an auction open at the same time (the auction guard will reject
a duplicate auction, and the listing guard rejects a duplicate listing).

---

## 2. Fixed-Price Listing Lifecycle

```
          list_nft                 buy_nft / accept_offer
Unlisted ──────────▶ Listed ─────────────────────────────▶ Settled (delisted)
              │
              └──── cancel_listing ──▶ Unlisted
```

### 2.1 States

| State | Storage key | Description |
|-------|-------------|-------------|
| Unlisted | *(absent)* | No `DataKey::Listing(token_id)` entry |
| Listed | `DataKey::Listing(token_id)` | Active listing with price and seller |
| Settled | *(absent)* | Listing removed after `buy_nft` or `accept_offer` |

### 2.2 Function Reference

#### `list_nft(seller, token_id, price, payment_token)`

- **Auth**: `seller.require_auth()`
- **Reentrancy guard**: yes
- **Preconditions**: `price > 0`, no existing listing for `token_id`
- **Effects**: stores `Listing`, appends `token_id` to `ActiveListings`
- **Event**: `("ListNFT", token_id) → (seller, price, payment_token)`

#### `cancel_listing(seller, token_id)`

- **Auth**: `seller.require_auth()`
- **Preconditions**: listing exists, `seller` is the lister
- **Effects**: removes `Listing`, removes from `ActiveListings`
- **Event**: `("ListCncl", token_id) → seller`

#### `buy_nft(buyer, token_id)`

- **Auth**: `buyer.require_auth()`
- **Reentrancy guard**: yes
- **Preconditions**: listing exists, `buyer ≠ seller`
- **Effects** (in order):
  1. Remove listing from storage
  2. Remove from `ActiveListings`
  3. Transfer `seller_proceeds` from buyer → seller
  4. Transfer `marketplace_fee` from buyer → fee recipient (if fee > 0)
  5. *(NFT transfer — stub; use NFT contract client in production)*
- **Event**: `("NFTSold", token_id) → (seller, buyer, price)`

---

## 3. Offer System Lifecycle

```
               make_offer
Any address ──────────────▶ Offer stored (per token, per offerer)
                                 │
                  ┌──────────────┴──────────────┐
                  │                             │
            cancel_offer                   accept_offer (by seller)
                  │                             │
             Offer removed                All offers removed
                                          + optional listing removed
                                          + payment transferred
                                          Event: "OffAccpt"
```

### 3.1 Function Reference

#### `make_offer(offerer, token_id, amount, payment_token)`

- **Auth**: `offerer.require_auth()`
- **Reentrancy guard**: yes
- **Preconditions**: `amount > 0`, no existing offer from `offerer` for this token
- **Effect**: appends `Offer` to `DataKey::Offers(token_id)`
- **Event**: `("OfferMade", token_id) → (offerer, amount, payment_token)`

#### `cancel_offer(offerer, token_id)`

- **Auth**: `offerer.require_auth()`
- **Preconditions**: offer exists
- **Effect**: removes offer; removes `Offers` key if list becomes empty
- **Event**: `("OfferCanc", token_id) → offerer`

#### `accept_offer(seller, token_id, offerer)`

- **Auth**: `seller.require_auth()`
- **Reentrancy guard**: yes
- **Preconditions**: offer by `offerer` exists for `token_id`
- **Effects** (in order):
  1. Remove all offers for `token_id`
  2. Remove listing for `token_id` if present
  3. Transfer `seller_proceeds` from offerer → seller
  4. Transfer `marketplace_fee` from offerer → fee recipient (if fee > 0)
- **Event**: `("OffAccpt", token_id) → (seller, offerer, offer.amount)`

---

## 4. English Auction Lifecycle

```
                  start_auction
Seller ────────────────────────────▶ Active (ends_at in future)
                                           │
                                  place_bid (repeatable)
                                           │
                                    current_bid updated
                                    prev bidder refunded
                                           │
                               ◀── ledger.timestamp ≥ ends_at ──▶
                                           │
                                      end_auction
                                           │
                         ┌─────────────────┴─────────────────┐
                         │ highest_bidder.is_some()          │ highest_bidder.is_none()
                         ▼                                    ▼
                    Pay seller + fee                    No payment (NFT returned)
                    Event: "AucEnd"                     Event: "AucNoBid"
```

### 4.1 Auction Storage Fields

| Field | Type | Description |
|-------|------|-------------|
| `token_id` | `u32` | Token being auctioned |
| `seller` | `Address` | Original lister |
| `starting_price` | `i128` | Minimum opening bid |
| `current_bid` | `i128` | Highest bid so far (starts at `starting_price`) |
| `highest_bidder` | `Option<Address>` | `None` until first bid |
| `payment_token` | `Address` | Token used for bids |
| `started_at` | `u64` | Ledger timestamp at `start_auction` |
| `ends_at` | `u64` | `started_at + duration_seconds` |
| `ended` | `bool` | Set to `true` by `end_auction` |

### 4.2 Function Reference

#### `start_auction(seller, token_id, starting_price, duration_seconds, payment_token)`

- **Auth**: `seller.require_auth()`
- **Reentrancy guard**: yes
- **Preconditions**: `starting_price > 0`, `duration_seconds > 0`, no existing auction
- **Event**: `("AucStart", token_id) → (seller, starting_price, ends_at)`

#### `place_bid(bidder, token_id, bid_amount)`

- **Auth**: `bidder.require_auth()`
- **Reentrancy guard**: yes
- **Preconditions**: auction is active (`timestamp < ends_at`), `bid_amount > current_bid`, `bidder ≠ seller`
- **Effects** (in order):
  1. Update `auction.current_bid` and `auction.highest_bidder`
  2. Transfer `bid_amount` from bidder → contract (escrow)
  3. Refund previous bidder from contract (if any)
- **Event**: `("BidPlaced", token_id) → (bidder, bid_amount)`

#### `end_auction(token_id)`

- **Auth**: none required (permissionless settlement)
- **Reentrancy guard**: yes
- **Preconditions**: `timestamp ≥ ends_at`, auction not already ended
- **Effects** (winner path):
  1. Mark `auction.ended = true`
  2. Remove from `ActiveAuctions`
  3. Transfer `seller_proceeds` from contract → seller
  4. Transfer `marketplace_fee` from contract → fee recipient (if fee > 0)
- **Effects** (no-bid path):
  1. Mark `auction.ended = true`
  2. Remove from `ActiveAuctions`
- **Events**:
  - Winner: `("AucEnd", token_id) → (winner, current_bid)`
  - No bids: `("AucNoBid", token_id) → seller`

---

## 5. Error Codes

| Code | Variant | Trigger |
|------|---------|---------|
| 1 | `NotInitialized` | Any call before `initialize` |
| 2 | `AlreadyInitialized` | Second call to `initialize` |
| 3 | `ListingNotFound` | `cancel_listing`, `buy_nft`, `get_listing` with unknown token |
| 4 | `NotSeller` | `cancel_listing` by non-owner |
| 5 | `NFTNotActive` | Reserved |
| 6 | `InvalidPrice` | `list_nft` / `start_auction` with `price ≤ 0` |
| 7 | `ListingExists` | `list_nft` twice; `start_auction` twice |
| 8 | `CannotBuyOwnListing` | `buy_nft` / `place_bid` by seller |
| 9 | `InsufficientPayment` | Reserved |
| 10 | `NFTContractError` | Reserved |
| 11 | `OfferNotFound` | `cancel_offer`, `accept_offer` with unknown offerer |
| 12 | `InvalidOfferAmount` | `make_offer` with `amount ≤ 0` |
| 13 | `OfferExists` | `make_offer` when offerer already has an open offer |
| 14 | `NotOfferMaker` | Reserved |
| 15 | `AuctionNotFound` | `place_bid`, `end_auction`, `get_auction` with unknown token |
| 16 | `AuctionEnded` | `place_bid` after expiry; `end_auction` on already-settled auction |
| 17 | `AuctionNotEnded` | `end_auction` before `ends_at` |
| 18 | `BidTooLow` | `place_bid` ≤ `current_bid` |
| 19 | `InvalidDuration` | `start_auction` with `duration_seconds == 0` |
| 20 | `ReentrancyDetected` | Nested call while guard is set |
| 21 | `TransferFailed` | Reserved |

---

## 6. Fee Arithmetic

```
marketplace_fee  = (price * fee_basis_points) / 10_000
seller_proceeds  = price - marketplace_fee
```

Integer division rounds toward zero.  With a 2.5 % fee (`fee_basis_points = 250`)
on a price of `1_000_0000001` stroops:

```
marketplace_fee = 1_000_0000001 * 250 / 10_000 = 25_000_000   (truncated)
seller_proceeds = 1_000_0000001 - 25_000_000   = 975_000_001
```

The one-stroop truncation accumulates in the seller's favor.

---

## 7. Security Assumptions

1. **NFT ownership** is not verified on-chain in this version.  Sellers are
   trusted to own the token they list.  Production deployments should add a
   cross-contract ownership check via the `CommitmentNFT` client.
2. **Reentrancy**: The boolean guard prevents re-entrant calls from any
   external token or NFT contract called within the function.
3. **Bid escrow**: Bids are held by the marketplace contract.  If `end_auction`
   is never called (e.g. the contract is paused), escrowed funds remain locked.
   Consider adding an admin-controlled emergency withdrawal.
4. **Unsigned-overflow**: All fee calculations use `i128`, which can hold the
   maximum Stellar asset balance (`922_337_203_685_477_580_7`).  The only
   risk is if `price * fee_basis_points` overflows `i128`, which would require
   a price near `i128::MAX / 10_000 ≈ 9.2 × 10^33` — far above any realistic
   Stellar balance.
