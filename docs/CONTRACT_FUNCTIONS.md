# Contract Functions

This document summarizes public entry points for each contract and their access control expectations.

## commitment_core

| Function                                                              | Summary                                          | Access control                            | Notes                                              |
| --------------------------------------------------------------------- | ------------------------------------------------ | ----------------------------------------- | -------------------------------------------------- |
| initialize(admin, nft_contract)                                       | Set admin, NFT contract, and counters.           | None (single-use).                        | Panics if already initialized.                     |
| create_commitment(owner, amount, asset_address, rules) -> String      | Creates commitment, transfers assets, mints NFT. | No require_auth; caller supplies owner.   | Uses reentrancy guard and rate limiting per owner. |
| get_commitment(commitment_id) -> Commitment                           | Fetch commitment details.                        | View.                                     | Panics if not found.                               |
| get_owner_commitments(owner) -> Vec<String>                           | List commitment IDs for owner.                   | View.                                     | Returns empty Vec if none.                         |
| get_total_commitments() -> u64                                        | Total commitments count.                         | View.                                     | Reads instance storage counter.                    |
| get_total_value_locked() -> i128                                      | Total value locked across commitments.           | View.                                     | Aggregate stored in instance storage.              |
| get_admin() -> Address                                                | Fetch admin address.                             | View.                                     | Panics if not initialized.                         |
| get_nft_contract() -> Address                                         | Fetch NFT contract address.                      | View.                                     | Panics if not initialized.                         |
| update_value(commitment_id, new_value)                                | Emit value update event.                         | No require_auth.                          | Does not update stored commitment value.           |
| check_violations(commitment_id) -> bool                               | Evaluate loss or duration violations.            | View.                                     | Emits violation event when violated.               |
| get_violation_details(commitment_id) -> (bool, bool, bool, i128, u64) | Detailed violation info.                         | View.                                     | Calculates loss percent and time remaining.        |
| settle(commitment_id)                                                 | Settle expired commitment and NFT.               | No require_auth.                          | Transfers assets and calls NFT settle.             |
| early_exit(commitment_id, caller)                                     | Exit early with penalty.                         | Checks caller == owner (no require_auth). | Uses SafeMath to compute penalty.                  |
| allocate(caller, commitment_id, target_pool, amount)                          | Allocate assets from commitment to pool.                         | Admin/authorized allocator only.                          | Validates amount > 0, sufficient balance, active commitment, and reentrancy guard. Transfers assets to target pool.                   |
| set_rate_limit(caller, function, window, max_calls)                   | Configure rate limits.                           | Admin only.                               | Uses shared RateLimiter.                           |
| set_rate_limit_exempt(caller, address, exempt)                        | Configure rate limit exemption.                  | Admin only.                               | Uses shared RateLimiter.                           |

### commitment_core cross-contract notes

- `create_commitment` is the main outbound write edge into `commitment_nft`; it also moves user assets into core custody.
- `settle` and `early_exit` both depend on downstream NFT lifecycle calls to keep mirrored state aligned.
- `get_commitment` is the canonical read edge consumed by `attestation_engine`.
- Cross-contract review reference: `docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md`

## commitment_interface

`commitment_interface` is an ABI-only crate. It should mirror the live
`commitment_core` commitment schema and a narrow set of production entrypoints.
CI drift tests compare its source-defined types and expected signatures against
`commitment_core` and `attestation_engine`.

| Function                                                            | Summary                                      | Access control            | Notes                                                                    |
| ------------------------------------------------------------------- | -------------------------------------------- | ------------------------- | ------------------------------------------------------------------------ |
| initialize(admin, nft_contract) -> Result                           | Initialize admin and linked NFT contract.    | Interface only.           | Live core contract is single-use; no state exists in this crate.         |
| create_commitment(owner, amount, asset_address, rules) -> Result<String> | Create a commitment and return string id.    | Interface only.           | Mirrors live `commitment_core` types, including `CommitmentRules`.       |
| get_commitment(commitment_id) -> Result<Commitment>                 | Fetch the canonical commitment record.       | View in live contract.    | `Commitment` shape is drift-checked against `commitment_core`.           |
| get_owner_commitments(owner) -> Result<Vec<String>>                 | List commitment ids owned by an address.     | View in live contract.    | Used by UIs and indexers.                                                |
| get_total_commitments() -> Result<u64>                              | Read the total commitment counter.           | View in live contract.    | Counter is stored by the live core contract.                             |
| settle(commitment_id) -> Result                                     | Settle an expired commitment.                | Mutating in live contract | Live implementation performs token and NFT cross-contract interactions.  |
| early_exit(commitment_id, caller) -> Result                         | Exit a commitment early with penalty logic.  | Mutating in live contract | Live implementation must enforce caller auth and overflow-safe math.     |

## commitment_nft

| Function                                                                                                                                       | Summary                            | Access control      | Notes                                       |
| ---------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | ------------------- | ------------------------------------------- |
| initialize(admin) -> Result                                                                                                                    | Set admin and token counters.      | None (single-use).  | Returns AlreadyInitialized on repeat.       |
| set_core_contract(core_contract) -> Result                                                                                                     | Set authorized core contract.      | Admin require_auth. | Emits CoreContractSet event.                |
| get_core_contract() -> Result<Address>                                                                                                         | Fetch core contract address.       | View.               | Fails if not initialized.                   |
| get_admin() -> Result<Address>                                                                                                                 | Fetch admin address.               | View.               | Fails if not initialized.                   |
| mint(owner, commitment_id, duration_days, max_loss_percent, commitment_type, initial_amount, asset_address, early_exit_penalty) -> Result<u32> | Mint NFT for a commitment.         | No require_auth.    | Validates inputs and uses reentrancy guard. |
| get_metadata(token_id) -> Result<CommitmentNFT>                                                                                                | Fetch NFT metadata.                | View.               | Fails if token missing.                     |
| owner_of(token_id) -> Result<Address>                                                                                                          | Fetch NFT owner.                   | View.               | Fails if token missing.                     |
| transfer(from, to, token_id) -> Result                                                                                                         | Transfer NFT ownership.            | from.require_auth.  | Updates owner balances and token lists.     |
| is_active(token_id) -> Result<bool>                                                                                                            | Check active status.               | View.               | Returns error if token missing.             |
| total_supply() -> u32                                                                                                                          | Total minted NFTs.                 | View.               | Reads token counter.                        |
| balance_of(owner) -> u32                                                                                                                       | NFT balance for owner.             | View.               | Returns 0 if no NFTs.                       |
| get_all_metadata() -> Vec<CommitmentNFT>                                                                                                       | List all NFTs.                     | View.               | Iterates token IDs.                         |
| get_nfts_by_owner(owner) -> Vec<CommitmentNFT>                                                                                                 | List NFTs for owner.               | View.               | Returns empty Vec if none.                  |
| settle(token_id) -> Result                                                                                                                     | Mark NFT settled after expiry.     | No require_auth.    | Uses reentrancy guard.                      |
| is_expired(token_id) -> Result<bool>                                                                                                           | Check expiry based on ledger time. | View.               | Requires token exists.                      |
| token_exists(token_id) -> bool                                                                                                                 | Check if token exists.             | View.               | Uses persistent storage.                    |

## attestation_engine

| Function                                                                      | Summary                           | Access control         | Notes                                                          |
| ----------------------------------------------------------------------------- | --------------------------------- | ---------------------- | -------------------------------------------------------------- |
| initialize(admin, commitment_core) -> Result                                  | Set admin and core contract.      | None (single-use).     | Returns AlreadyInitialized on repeat.                          |
| add_verifier(caller, verifier) -> Result                                      | Authorize verifier address.       | Admin require_auth.    | Stores verifier flag.                                          |
| remove_verifier(caller, verifier) -> Result                                   | Remove verifier authorization.    | Admin require_auth.    | Removes verifier flag.                                         |
| is_verifier(address) -> bool                                                  | Check verifier authorization.     | View.                  | Admin is implicitly authorized.                                |
| get_admin() -> Result<Address>                                                | Fetch admin address.              | View.                  | Fails if not initialized.                                      |
| get_core_contract() -> Result<Address>                                        | Fetch core contract address.      | View.                  | Fails if not initialized.                                      |
| get_stored_health_metrics(commitment_id) -> Option<HealthMetrics>             | Fetch cached health metrics.      | View.                  | Returns None if missing.                                       |
| attest(caller, commitment_id, attestation_type, data, is_compliant) -> Result | Record attestation.               | Verifier require_auth. | Validates commitment, uses rate limiting and reentrancy guard. |
| get_attestations(commitment_id) -> Vec<Attestation>                           | List attestations for commitment. | View.                  | Returns empty Vec if none.                                     |
| get_attestations_page(commitment_id, offset, limit) -> AttestationsPage        | Paginated attestations.           | View.                  | Order: timestamp (oldest first). Max page size MAX_PAGE_SIZE=100. next_offset=0 when no more. |
| get_attestation_count(commitment_id) -> u64                                   | Count attestations.               | View.                  | Stored in persistent storage.                                  |
| get_health_metrics(commitment_id) -> HealthMetrics                            | Compute current health metrics.   | View.                  | Reads commitment_core data.                                    |
| verify_compliance(commitment_id) -> bool                                      | Check compliance vs rules.        | View.                  | Uses health metrics and rules.                                 |
| record_fees(caller, commitment_id, fee_amount) -> Result                      | Convenience fee attestation.      | Verifier require_auth. | Calls attest() internally.                                     |
| record_drawdown(caller, commitment_id, drawdown_percent) -> Result            | Convenience drawdown attestation. | Verifier require_auth. | Calls attest() internally.                                     |
| calculate_compliance_score(commitment_id) -> u32                              | Compute compliance score.         | View.                  | Emits ScoreUpd event.                                          |
| get_protocol_statistics() -> (u64, u64, u64, i128)                            | Aggregate protocol stats.         | View.                  | Reads commitment_core counters.                                |
| get_verifier_statistics(verifier) -> u64                                      | Per-verifier attestation count.   | View.                  | Stored in instance storage.                                    |
| set_rate_limit(caller, function, window, max_calls) -> Result                 | Configure rate limits.            | Admin require_auth.    | Uses shared RateLimiter.                                       |
| set_rate_limit_exempt(caller, verifier, exempt) -> Result                     | Configure rate limit exemption.   | Admin require_auth.    | Uses shared RateLimiter.                                       |

### attestation_engine cross-contract notes

- `attest`, `verify_compliance`, `get_health_metrics`, and analytics helpers treat `commitment_core` as the source of truth for commitment existence and status.
- The call graph is intentionally read-oriented from attestation to core in this integration.
- Cross-contract review reference: `docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md`

## allocation_logic

| Function                                                                       | Summary                                 | Access control       | Notes                                     |
| ------------------------------------------------------------------------------ | --------------------------------------- | -------------------- | ----------------------------------------- |
| initialize(admin, commitment_core) -> Result                                   | Set admin, core contract, and registry. | Admin require_auth.  | Returns AlreadyInitialized on repeat.     |
| register_pool(admin, pool_id, risk_level, apy, max_capacity) -> Result         | Register investment pool.               | Admin require_auth.  | Validates capacity and APY.               |
| update_pool_status(admin, pool_id, active) -> Result                           | Activate/deactivate pool.               | Admin require_auth.  | Updates pool timestamps.                  |
| update_pool_capacity(admin, pool_id, new_capacity) -> Result                   | Update pool capacity.                   | Admin require_auth.  | Ensures capacity >= liquidity.            |
| allocate(caller, commitment_id, amount, strategy) -> Result<AllocationSummary> | Allocate funds across pools.            | caller.require_auth. | Validates commitment against core; uses rate limiting. |
| rebalance(caller, commitment_id) -> Result<AllocationSummary>                  | Reallocate using stored strategy.       | caller.require_auth. | Requires caller matches owner; validates core. |
| get_allocation(commitment_id) -> AllocationSummary                             | Fetch allocation summary.               | View.                | String ID; returns empty summary if missing.         |
| get_pool(pool_id) -> Result<Pool>                                              | Fetch pool info.                        | View.                | Returns PoolNotFound if missing.          |
| get_all_pools() -> Vec<Pool>                                                   | Fetch all pools.                        | View.                | Iterates registry.                        |
| is_initialized() -> bool                                                       | Check initialization flag.              | View.                | Returns false if uninitialized.           |
| set_rate_limit(admin, function, window, max_calls) -> Result                   | Configure rate limits.                  | Admin require_auth.  | Uses shared RateLimiter.                  |
| set_rate_limit_exempt(admin, address, exempt) -> Result                        | Configure rate limit exemption.         | Admin require_auth.  | Uses shared RateLimiter.                  |

## price_oracle

| Function                                               | Summary                                          | Access control            | Notes                                                                          |
| ------------------------------------------------------ | ------------------------------------------------ | ------------------------- | ------------------------------------------------------------------------------ |
| initialize(admin) -> Result                            | Set admin and default staleness window.          | None (single-use).        | Initializes whitelist authority and versioned config.                          |
| add_oracle(caller, oracle_address) -> Result           | Add a trusted price publisher.                   | Admin require_auth.       | Whitelisted oracle can overwrite the latest price for any asset it updates.    |
| remove_oracle(caller, oracle_address) -> Result        | Remove a trusted price publisher.                | Admin require_auth.       | Prevents further updates from that address.                                    |
| is_oracle_whitelisted(address) -> bool                 | Check whitelist membership.                      | View.                     | Reads the admin-managed trust list.                                            |
| set_price(caller, asset, price, decimals) -> Result    | Publish latest price for an asset.               | Oracle require_auth.      | Validates non-negative price; does not aggregate or reconcile multiple feeds.  |
| get_price(asset) -> PriceData                          | Read the raw latest price snapshot.              | View.                     | Returns zeroed `PriceData` if unset; does not enforce freshness.               |
| get_price_valid(asset, max_staleness_override) -> Result<PriceData> | Read a fresh price snapshot or fail. | View.                     | Rejects stale and future-dated data; preferred for security-sensitive reads.   |
| set_max_staleness(caller, seconds) -> Result           | Update default freshness window.                 | Admin require_auth.       | Tunes rejection threshold for delayed oracle updates.                          |
| get_max_staleness() -> u64                             | Read default freshness window.                   | View.                     | Used when `get_price_valid` has no override.                                   |
| get_admin() -> Address                                 | Read oracle admin.                               | View.                     | Returns the current whitelist/config authority.                                |
| set_admin(caller, new_admin) -> Result                 | Transfer oracle admin authority.                 | Admin require_auth.       | Transfers control over whitelist and configuration.                            |
| upgrade(caller, new_wasm_hash) -> Result               | Upgrade contract code.                           | Admin require_auth.       | Validates non-zero WASM hash.                                                  |
| migrate(caller, from_version) -> Result                | Migrate legacy storage to current version.       | Admin require_auth.       | Replays are blocked once current version is installed.                         |

### price_oracle manipulation-resistance notes

- `price_oracle` is a trusted-publisher registry, not an on-chain price discovery engine.
- A whitelisted oracle may unilaterally replace the latest price for an asset.
- Freshness protection is enforced by `get_price_valid`; integrators should prefer it over `get_price`.
- Downstream contracts should pick staleness windows that fit the asset’s liquidity and their own liquidation or settlement risk.
- Threat model reference: `docs/THREAT_MODEL.md#price-oracle-manipulation-resistance-assumptions`

## commitment_nft - Edge Cases and Error Codes

### Transfer Function Edge Cases

The `transfer()` function enforces strict validation to prevent ambiguous or unsafe states. All edge cases are documented and tested.

#### Edge Case 1: Self-Transfer Rejection

- **Scenario**: `transfer(owner, owner, token_id)` where from == to
- **Error Code**: #18 - `TransferToZeroAddress`
- **Rationale**: Prevents accidental no-ops and maintains explicit state transitions
- **Test Coverage**: `test_transfer_edge_case_self_transfer`
- **Behavior**: Transaction rejected, no state changes

#### Edge Case 2: Non-Owner Transfer

- **Scenario**: `transfer(non_owner, recipient, token_id)` where non_owner != current owner
- **Error Code**: #5 - `NotOwner`
- **Rationale**: Only the current owner can initiate transfers, preventing unauthorized transfers
- **Test Coverage**: `test_transfer_edge_case_from_non_owner`
- **Behavior**: Transaction rejected, no state changes

#### Edge Case 3: Invalid/Zero Address

- **Scenario**: `transfer(owner, invalid_address, token_id)`
- **Error Code**: Prevented at SDK level (compile-time safety)
- **Rationale**: Soroban SDK's strongly-typed `Address` prevents invalid addresses at the type level
- **Test Coverage**: `test_transfer_edge_case_address_validation_by_sdk` (defensive documentation)
- **Behavior**: Cannot construct invalid Address at compile time; SDK enforces invariants

#### Edge Case 4: Locked NFT Transfer

- **Scenario**: `transfer(owner, recipient, token_id)` where NFT has active commitment
- **Error Code**: #19 - `NFTLocked`
- **Rationale**: Active commitments cannot be transferred to prevent commitment state conflicts
- **Behavior**: Transaction rejected, no state changes

#### Edge Case 5: Non-Existent Token

- **Scenario**: `transfer(owner, recipient, 999)` where token_id doesn't exist
- **Error Code**: #3 - `TokenNotFound`
- **Rationale**: Cannot transfer tokens that don't exist
- **Behavior**: Transaction rejected, no state changes

### NFT Transfer Error Code Reference

| Error Code | Name                  | Meaning                                                    | When Returned                                             |
| ---------- | --------------------- | ---------------------------------------------------------- | --------------------------------------------------------- |
| #3         | TokenNotFound         | NFT token does not exist                                   | `transfer()` called with non-existent token_id            |
| #5         | NotOwner              | Caller is not the token owner                              | `transfer()` called from address other than current owner |
| #18        | TransferToZeroAddress | Invalid transfer destination (semantically: self-transfer) | `transfer()` called with from == to                       |
| #19        | NFTLocked             | NFT cannot be transferred (active commitment)              | `transfer()` called on NFT with active commitment         |

### Transfer State Machine

```
Initial State: owner = A
         ↓
transfer(A, B, token_id)
  ├─ CHECKS:
  │  ├─ from.require_auth() → A must authorize
  │  ├─ from != to → prevent self-transfer (#18)
  │  ├─ owner == from → prevent non-owner transfer (#5)
  │  ├─ is_active == false → prevent locked transfer (#19)
  │  └─ token exists → prevent non-existent token (#3)
  │
  └─ EFFECTS:
     └─ owner = B
         token_lists updated
         balances updated
         Transfer event emitted
         ↓
Final State: owner = B
```

### Transfer Validation Philosophy

1. **Fail-Fast**: All validations occur in the CHECKS phase before any state modifications
2. **Clear Semantics**: Error codes clearly indicate what went wrong
3. **SDK Guarantees**: Leverage Soroban SDK's type safety for address validation
4. **Lock Enforcement**: Active commitments cannot be transferred to maintain consistency
5. **Ownership Verification**: Only the current owner can initiate transfers

### Testing Edge Cases

All edge cases are tested in `contracts/commitment_nft/src/tests.rs`:

- `test_transfer_edge_case_self_transfer()` - Verifies self-transfer rejection
- `test_transfer_edge_case_from_non_owner()` - Verifies non-owner rejection
- `test_transfer_edge_case_address_validation_by_sdk()` - Documents SDK-level safety
- `test_transfer_edge_cases_comprehensive()` - Comprehensive multi-step transfer sequences

Run all tests:

| Function | Summary | Access control | Notes |
| -------- | -------- | ------------- | ------ |
| initialize(admin, nft_contract, fee_basis_points, fee_recipient) | Set admin, NFT contract, and marketplace fee | None (single-use) | Returns AlreadyInitialized on repeat |
| get_admin() -> Address | Fetch admin address | View | Fails if not initialized |
| update_fee(fee_basis_points) | Update marketplace fee percentage | Admin require_auth | Emits FeeUpdated event |
| list_nft(seller, token_id, price, payment_token) | List NFT for sale | Seller require_auth | Uses reentrancy guard, validates price > 0 |
| cancel_listing(seller, token_id) | Cancel active listing | Seller require_auth | Removes listing and active listing entry |
| buy_nft(buyer, token_id) | Purchase NFT from listing | Buyer require_auth | Critical - handles token/NFT transfers with reentrancy guard |
| get_listing(token_id) -> Listing | Get specific listing details | View | Returns ListingNotFound if not found |
| get_all_listings() -> Vec<Listing> | Get all active listings | View | Returns empty vector if none |
| make_offer(offerer, token_id, amount, payment_token) | Make offer on NFT | Offerer require_auth | One offer per user per token, uses reentrancy guard |
| accept_offer(seller, token_id, offerer) | Accept specific offer | Seller require_auth | Removes all offers, handles transfers with reentrancy guard |
| cancel_offer(offerer, token_id) | Cancel your offer | Offerer require_auth | Removes specific offer |
| get_offers(token_id) -> Vec<Offer> | Get all offers for token | View | Returns empty vector if none |
| start_auction(seller, token_id, starting_price, duration, payment_token) | Start NFT auction | Seller require_auth | Uses reentrancy guard, validates price and duration |
| place_bid(bidder, token_id, bid_amount) | Place bid on auction | Bidder require_auth | Refunds previous bidder, uses reentrancy guard |
| end_auction(token_id) | Finalize ended auction | View | Transfers NFT to winner, handles fees, uses reentrancy guard |
| get_auction(token_id) -> Auction | Get auction details | View | Returns AuctionNotFound if not found |
| get_all_auctions() -> Vec<Auction> | Get all active auctions | View | Returns empty vector if none |

### commitment_marketplace cross-contract notes

- `buy_nft` is the primary outbound write edge into payment token contracts and NFT contracts.
- `list_nft` depends on NFT contract for ownership verification (not implemented in current version).
- `accept_offer` and `end_auction` handle complex multi-contract interactions with fee distribution.
- All functions that handle external calls use reentrancy guards and checks-effects-interactions pattern.
- Fee calculations use safe arithmetic to prevent overflow/underflow.

### Buy Flow Security Considerations

The `buy_nft` function implements comprehensive security measures:

1. **Reentrancy Protection**: Uses reentrancy guard to prevent recursive calls
2. **Checks-Effects-Interactions Pattern**: Removes listing before external calls
3. **Input Validation**: Validates buyer != seller and listing exists
4. **Fee Safety**: Calculates fees using overflow-safe arithmetic
5. **Error Propagation**: Transparently propagates external contract errors
6. **Atomic Operations**: All transfers happen in a single transaction

### Payment Token Handling

- Supports any token contract that implements the standard Soroban token interface
- Fee calculation: `(price * fee_basis_points) / 10000`
- Two transfers if fee > 0: buyer→seller and buyer→fee_recipient
- One transfer if fee = 0: buyer→seller
- Transfer failures are propagated as `TransferFailed`

### NFT Transfer Failure Handling

The marketplace handles NFT transfer failures through:

1. **Error Propagation**: NFT contract errors are propagated as `NFTContractError`
2. **State Consistency**: Uses checks-effects-interactions pattern
3. **Partial Failure Recovery**: Documented behavior for partial transfer failures
4. **Manual Recovery**: Some failure scenarios may require manual intervention
5. **Event Logging**: All failures emit events for debugging

### Failure Scenarios

| Scenario | Error Type | Recovery Method |
| --------- | ----------- | -------------- |
| Insufficient token balance | `TransferFailed` | Buyer needs to acquire more tokens |
| NFT not owned by seller | `NFTContractError` | Seller needs to verify ownership |
| NFT contract error | `NFTContractError` | Check NFT contract status |
| Reentrancy attack | `ReentrancyDetected` | Attack blocked by guard |
| Listing not found | `ListingNotFound` | Verify token ID is correct |
| Buyer equals seller | `CannotBuyOwnListing` | Use different buyer address |

### Gas Optimization

- Storage operations minimized where possible
- External calls batched when safe
- Event emission optimized for indexing
- Reentrancy guard uses efficient storage pattern
- Runbook reference: `docs/TIMELOCK_RUNBOOK.md#timelock-parameter-runbook`

## shared_utils

| Module         | Functions                                                              | Notes                                     |
| -------------- | ---------------------------------------------------------------------- | ----------------------------------------- |
| access_control | require_admin, require_owner, require_owner_or_admin                   | Uses Storage::get_admin and require_auth. |
| errors         | log_error, panic_with_log, require                                     | Centralized error logging helpers.        |
| events         | emit_created, emit_updated, emit_transfer, emit_violation              | Standard event wrappers.                  |
| math           | add, sub, mul, div, percent, loss_percent, gain_percent                | Safe arithmetic with checked operations.  |
| rate_limiting  | set_limit, clear_limit, check, set_exempt                              | Fixed-window rate limiter.                |
| storage        | set_initialized, get_admin, get_or_default                             | Instance storage helpers.                 |
| time           | now, calculate_expiration, is_expired                                  | Ledger time utilities.                    |
| validation     | require_positive, require_valid_percent, require_valid_commitment_type | Common validation guards.                 |
