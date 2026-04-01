# Commitment Marketplace Contract

This page documents the public entry points, access control, and security notes for the `commitment_marketplace` Soroban contract. It is intended for integrators and auditors.

## Entry Points

| Function                | Summary                                      | Access Control         | Errors / Security Notes                                  |
|------------------------|----------------------------------------------|-----------------------|----------------------------------------------------------|
| initialize             | Set admin, NFT contract, fee, fee recipient  | Admin require_auth    | Fails if already initialized                             |
| update_fee             | Update marketplace fee                       | Admin require_auth    | Fails if not initialized                                 |
| list_nft               | List NFT for sale                            | Seller require_auth   | Fails if price <= 0, listing exists, or not initialized  |
| cancel_listing         | Cancel NFT listing                           | Seller require_auth   | Fails if not found or not seller                         |
| buy_nft                | Buy NFT from listing                         | Buyer require_auth    | Fails if not found, self-buy, or not initialized         |
| make_offer             | Make offer on NFT                            | Offerer require_auth  | Fails if amount <= 0 or duplicate offer                  |
| accept_offer           | Accept offer on NFT                          | Seller require_auth   | Fails if offer not found or not initialized              |
| cancel_offer           | Cancel offer                                 | Offerer require_auth  | Fails if offer not found                                 |
| start_auction          | Start auction for NFT                        | Seller require_auth   | Fails if price/duration invalid or auction exists        |
| place_bid              | Place bid on auction                         | Bidder require_auth   | Fails if bid too low, ended, or self-bid                 |
| end_auction            | End auction and settle                       | Anyone (time-gated)   | Fails if not ended, already ended, or not found          |
| get_listing            | Get listing details                          | View                  | Fails if not found                                      |
| get_all_listings       | Get all active listings                      | View                  |                                                          |
| get_offers             | Get all offers for NFT                       | View                  |                                                          |
| get_auction            | Get auction details                          | View                  | Fails if not found                                      |
| get_all_auctions       | Get all active auctions                      | View                  |                                                          |

## Security
- All state-changing entry points require authentication (`require_auth`) for the relevant actor, except `end_auction` (which is time-gated).
- Reentrancy guard is enforced on all entry points that mutate state and/or make external calls.
- Arithmetic is performed using checked math; integer division truncates toward zero.
- No cross-contract NFT ownership checks or transfers are performed in this implementation (see contract comments).
- All token transfers use Soroban token interface.

## Reentrancy Guard
- All mutating entry points set/check/clear a `ReentrancyGuard` storage key.
- If the guard is set, the contract returns `MarketplaceError::ReentrancyDetected`.
- See contract and tests for explicit coverage.

## Integrator Notes
- Integrators should expect deterministic errors for all invalid operations.
- All public APIs are documented with Rustdoc/NatSpec comments in the contract source.
- See also: `docs/CONTRACT_FUNCTIONS.md` for cross-contract summary.

## Changelog
- 2026-03-25: Added explicit reentrancy guard tests and NatSpec documentation. This page created for integrator clarity.
