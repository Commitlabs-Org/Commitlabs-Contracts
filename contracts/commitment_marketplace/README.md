# Commitment NFT Marketplace

A comprehensive secondary market for Commitment NFTs on Soroban/Stellar, featuring fixed-price listings, offers, and auctions with secure escrow.

## Features

### 🏪 Fixed-Price Listings

- List NFTs at a fixed price
- Cancel listings anytime
- Automatic fee calculation
- Secure escrow mechanism

### 💰 Offer System

- Make offers below listing price
- Multiple offers per NFT
- Accept/reject offers
- Automatic offer cancellation on sale

### 🔨 Auction System

- Time-based auctions
- Automatic bid refunds
- Dutch auction support (future)
- Secure escrow for bids

### 🔐 Security Features

- Reentrancy protection on all state-changing functions
- Checks-Effects-Interactions pattern
- Access control on sensitive operations
- Comprehensive input validation

### 💸 Fee Structure

- Configurable marketplace fees (basis points)
- Transparent fee calculation
- Admin-controlled fee updates
- Fee recipient management

## Quick Start

### Installation

```bash
# Clone or navigate to your project
cd your-project

# Run setup script
chmod +x setup_marketplace.sh
./setup_marketplace.sh

# Copy source files
cp commitment_marketplace.rs contracts/commitment_marketplace/src/lib.rs
cp marketplace_tests.rs contracts/commitment_marketplace/src/tests.rs
```

### Build

```bash
cd contracts/commitment_marketplace
cargo build --target wasm32-unknown-unknown --release
```

### Test

```bash
cargo test
```

### Deploy

```bash
# Deploy to testnet
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/commitment_marketplace.wasm \
  --source <YOUR_ACCOUNT> \
  --network testnet

# Initialize
soroban contract invoke \
  --id <MARKETPLACE_CONTRACT_ID> \
  --source <YOUR_ACCOUNT> \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --nft_contract <NFT_CONTRACT_ADDRESS> \
  --fee_basis_points 250 \
  --fee_recipient <FEE_RECIPIENT_ADDRESS>
```

## Usage

### List an NFT

```rust
marketplace.list_nft(
    seller_address,
    token_id,
    price,
    payment_token_address
)
```

```bash
soroban contract invoke \
  --id $MARKETPLACE \
  --source $SELLER \
  -- list_nft \
  --seller $SELLER_ADDR \
  --token_id 1 \
  --price 1000000000 \
  --payment_token $TOKEN_ADDR
```

### Buy an NFT

```rust
marketplace.buy_nft(
    buyer_address,
    token_id
)
```

```bash
# First approve marketplace to spend payment tokens
soroban contract invoke \
  --id $PAYMENT_TOKEN \
  --source $BUYER \
  -- approve \
  --from $BUYER_ADDR \
  --spender $MARKETPLACE \
  --amount 1000000000

# Then buy
soroban contract invoke \
  --id $MARKETPLACE \
  --source $BUYER \
  -- buy_nft \
  --buyer $BUYER_ADDR \
  --token_id 1
```

### Make an Offer

```rust
marketplace.make_offer(
    offerer_address,
    token_id,
    amount,
    payment_token_address
)
```

### Start an Auction

```rust
marketplace.start_auction(
    seller_address,
    token_id,
    starting_price,
    duration_seconds,
    payment_token_address
)
```

```bash
soroban contract invoke \
  --id $MARKETPLACE \
  --source $SELLER \
  -- start_auction \
  --seller $SELLER_ADDR \
  --token_id 1 \
  --starting_price 500000000 \
  --duration_seconds 86400 \
  --payment_token $TOKEN_ADDR
```

### Place a Bid

```rust
marketplace.place_bid(
    bidder_address,
    token_id,
    bid_amount
)
```

### Configure Auction Bidding

```rust
marketplace.update_auction_settings(
    min_bid_increment,
    anti_sniping_window,
    anti_sniping_extension,
    max_anti_sniping_extension
)
```

### End Auction

```rust
marketplace.end_auction(token_id)
```

## API Reference

### Initialization

#### `initialize`

```rust
fn initialize(
    e: Env,
    admin: Address,
    nft_contract: Address,
    fee_basis_points: u32,
    fee_recipient: Address,
) -> Result<(), MarketplaceError>
```

Initialize the marketplace with admin, NFT contract address, fee structure, and fee recipient.

### Listing Management

#### `list_nft`

```rust
fn list_nft(
    e: Env,
    seller: Address,
    token_id: u32,
    price: i128,
    payment_token: Address,
) -> Result<(), MarketplaceError>
```

List an NFT for fixed-price sale.

#### `cancel_listing`

```rust
fn cancel_listing(
    e: Env,
    seller: Address,
    token_id: u32,
) -> Result<(), MarketplaceError>
```

Cancel an active listing (seller only).

#### `buy_nft`

```rust
fn buy_nft(
    e: Env,
    buyer: Address,
    token_id: u32,
) -> Result<(), MarketplaceError>
```

Purchase a listed NFT.

#### `get_listing`

```rust
fn get_listing(
    e: Env,
    token_id: u32,
) -> Result<Listing, MarketplaceError>
```

Get listing details for a specific token.

#### `get_all_listings`

```rust
fn get_all_listings(e: Env) -> Vec<Listing>
```

Get all active listings.

### Offer System

#### `make_offer`

```rust
fn make_offer(
    e: Env,
    offerer: Address,
    token_id: u32,
    amount: i128,
    payment_token: Address,
) -> Result<(), MarketplaceError>
```

Make an offer on an NFT.

#### `accept_offer`

```rust
fn accept_offer(
    e: Env,
    seller: Address,
    token_id: u32,
    offerer: Address,
) -> Result<(), MarketplaceError>
```

Accept a specific offer (seller/owner only).

#### `cancel_offer`

```rust
fn cancel_offer(
    e: Env,
    offerer: Address,
    token_id: u32,
) -> Result<(), MarketplaceError>
```

Cancel your own offer.

#### `get_offers`

```rust
fn get_offers(e: Env, token_id: u32) -> Vec<Offer>
```

Get all offers for a specific token.

### Auction System

#### `start_auction`

```rust
fn start_auction(
    e: Env,
    seller: Address,
    token_id: u32,
    starting_price: i128,
    duration_seconds: u64,
    payment_token: Address,
) -> Result<(), MarketplaceError>
```

Start a time-based auction.

#### `place_bid`

```rust
fn place_bid(
    e: Env,
    bidder: Address,
    token_id: u32,
    bid_amount: i128,
) -> Result<(), MarketplaceError>
```

Place a bid on an active auction. When configured by `update_auction_settings`, bids must satisfy the absolute minimum increment and bids inside the anti-sniping window extend `ends_at` until the per-auction extension cap is reached. With default zero settings, bids only need to remain strictly higher than the current bid.

#### `update_auction_settings`

```rust
fn update_auction_settings(
    e: Env,
    min_bid_increment: i128,
    anti_sniping_window: u64,
    anti_sniping_extension: u64,
    max_anti_sniping_extension: u64,
) -> Result<(), MarketplaceError>
```

Configure auction bid increments and anti-sniping extensions.

#### `end_auction`

```rust
fn end_auction(e: Env, token_id: u32) -> Result<(), MarketplaceError>
```

End an auction after expiry time.

#### `get_auction`

```rust
fn get_auction(
    e: Env,
    token_id: u32,
) -> Result<Auction, MarketplaceError>
```

Get auction details.

#### `get_all_auctions`

```rust
fn get_all_auctions(e: Env) -> Vec<Auction>
```

Get all active auctions.

### Admin Functions

#### `update_fee`

```rust
fn update_fee(
    e: Env,
    fee_basis_points: u32,
) -> Result<(), MarketplaceError>
```

Update marketplace fee (admin only).

#### `get_admin`

```rust
fn get_admin(e: Env) -> Result<Address, MarketplaceError>
```

Get admin address.

## Data Structures

### Listing

```rust
pub struct Listing {
    pub token_id: u32,
    pub seller: Address,
    pub price: i128,
    pub payment_token: Address,
    pub listed_at: u64,
}
```

### Offer

```rust
pub struct Offer {
    pub token_id: u32,
    pub offerer: Address,
    pub amount: i128,
    pub payment_token: Address,
    pub created_at: u64,
}
```

### Auction

```rust
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
```

## Error Codes

| Code | Error               | Description                   |
| ---- | ------------------- | ----------------------------- |
| 1    | NotInitialized      | Marketplace not initialized   |
| 2    | AlreadyInitialized  | Already initialized           |
| 3    | ListingNotFound     | Listing doesn't exist         |
| 4    | NotSeller           | Caller is not the seller      |
| 5    | NFTNotActive        | NFT is not active             |
| 6    | InvalidPrice        | Price must be > 0             |
| 7    | ListingExists       | Listing already exists        |
| 8    | CannotBuyOwnListing | Seller cannot buy own listing |
| 9    | InsufficientPayment | Payment amount too low        |
| 10   | NFTContractError    | NFT contract call failed      |
| 11   | OfferNotFound       | Offer doesn't exist           |
| 12   | InvalidOfferAmount  | Offer amount must be > 0      |
| 13   | OfferExists         | Offer already exists          |
| 14   | NotOfferMaker       | Not the offer creator         |
| 15   | AuctionNotFound     | Auction doesn't exist         |
| 16   | AuctionEnded        | Auction already ended         |
| 17   | AuctionNotEnded     | Auction still active          |
| 18   | BidTooLow           | Bid below current price       |
| 19   | InvalidDuration     | Duration must be > 0          |
| 20   | ReentrancyDetected  | Reentrancy attack prevented   |
| 21   | TransferFailed      | Token transfer failed         |

## Events

### Listing Events

- `ListNFT(token_id)` → `(seller, price, payment_token)`
- `ListCncl(token_id)` → `seller`
- `NFTSold(token_id)` → `(seller, buyer, price)`

### Offer Events

- `OfferMade(token_id)` → `(offerer, amount, payment_token)`
- `OffAccpt(token_id)` → `(seller, offerer, amount)`
- `OfferCanc(token_id)` → `offerer`

### Auction Events

- `AucStart(token_id)` → `(seller, starting_price, ends_at)`
- `BidPlaced(token_id)` → `(bidder, bid_amount)`
- `AucEnd(token_id)` → `(winner, final_bid)`
- `AucNoBid(token_id)` → `seller`

## Testing

### Run All Tests

```bash
cargo test
```

### Run Specific Test

```bash
cargo test test_list_nft
```

### Run with Benchmarks

```bash
cargo test --features benchmark
```

### Test Coverage

- ✅ Initialization
- ✅ Listing management
- ✅ Buy flow
- ✅ Offer system
- ✅ Auction system
- ✅ Access control
- ✅ Edge cases
- ✅ Reentrancy protection

## Security

### Reentrancy Protection

All state-changing functions implement:

1. Reentrancy guard check
2. State updates before external calls
3. Guard cleanup in all paths

### Access Control

- Seller-only: `cancel_listing`, `accept_offer`
- Admin-only: `update_fee`
- Offerer-only: `cancel_offer`

### Input Validation

- Price > 0
- Duration > 0
- Token existence checks
- Ownership verification

## Gas Optimization

- Batch storage operations where possible
- Efficient vector operations
- Minimal external calls
- Optimized data structures

## Contributing

See [IMPLEMENTATION_GUIDE.md](IMPLEMENTATION_GUIDE.md) for development guidelines.

## License

MIT

## Support

- 📚 Documentation: See IMPLEMENTATION_GUIDE.md
- 🐛 Issues: [GitHub Issues]
- 💬 Discord: [Soroban Discord](https://discord.gg/soroban)

---

Built with ❤️ for the Soroban ecosystem
