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
| allocate(commitment_id, target_pool, amount)                          | Allocate assets to pool.                         | No require_auth.                          | Transfers assets to target pool.                   |
| set_rate_limit(caller, function, window, max_calls)                   | Configure rate limits.                           | Admin only.                               | Uses shared RateLimiter.                           |
| set_rate_limit_exempt(caller, address, exempt)                        | Configure rate limit exemption.                  | Admin only.                               | Uses shared RateLimiter.                           |

## commitment_interface

`commitment_interface` is an ABI-only crate. It mirrors the live
`commitment_core` commitment schema, event payloads, and the core read-only
entrypoints that downstream bindings commonly consume. CI drift checks compare
its source-defined types and expected signatures against `commitment_core` and
`attestation_engine`.

| Function                                                            | Summary                                      | Access control            | Notes                                                                    |
| ------------------------------------------------------------------- | -------------------------------------------- | ------------------------- | ------------------------------------------------------------------------ |
| initialize(admin, nft_contract) -> Result                           | Initialize admin and linked NFT contract.    | Interface only.           | Live core contract is single-use; no state exists in this crate.         |
| create_commitment(owner, amount, asset_address, rules) -> Result<String> | Create a commitment and return string id.    | Interface only.           | Mirrors live `commitment_core` types, including `CommitmentRules`.       |
| get_commitment(commitment_id) -> Result<Commitment>                 | Fetch the canonical commitment record.       | View in live contract.    | `Commitment` shape is drift-checked against `commitment_core`.           |
| list_commitments_by_owner(owner) -> Result<Vec<String>>             | Alias for owner-indexed commitment lookup.   | View in live contract.    | Mirrors the live helper exposed by `commitment_core`.                    |
| get_owner_commitments(owner) -> Result<Vec<String>>                 | List commitment ids owned by an address.     | View in live contract.    | Used by UIs and indexers.                                                |
| get_total_commitments() -> Result<u64>                              | Read the total commitment counter.           | View in live contract.    | Counter is stored by the live core contract.                             |
| get_total_value_locked() -> Result<i128>                            | Read aggregate active TVL.                   | View in live contract.    | Derived from mutable storage in create/update/settle/exit paths.         |
| get_commitments_created_between(from_ts, to_ts) -> Result<Vec<String>> | Read commitment ids created in a time range. | View in live contract.    | Useful for analytics and indexers.                                       |
| get_admin() -> Result<Address>                                      | Read configured admin.                       | View in live contract.    | Panics in live core if contract is not initialized.                      |
| get_nft_contract() -> Result<Address>                               | Read linked NFT contract address.            | View in live contract.    | Panics in live core if contract is not initialized.                      |
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
| get_health_metrics(commitment_id) -> HealthMetrics                            | Compute current health metrics.   | View.                  | Cross-reads `commitment_core` for canonical commitment values and aggregates local attestations. |
| verify_compliance(commitment_id) -> bool                                      | Check compliance vs rules.        | View.                  | Uses health metrics and rules.                                 |
| record_fees(caller, commitment_id, fee_amount) -> Result                      | Convenience fee attestation.      | Verifier require_auth. | Calls attest() internally.                                     |
| record_drawdown(caller, commitment_id, drawdown_percent) -> Result            | Convenience drawdown attestation. | Verifier require_auth. | Calls attest() internally.                                     |
| calculate_compliance_score(commitment_id) -> u32                              | Compute compliance score.         | View.                  | Emits ScoreUpd event.                                          |
| get_protocol_statistics() -> (u64, u64, u64, i128)                            | Aggregate protocol stats.         | View.                  | Reads commitment_core counters.                                |
| get_verifier_statistics(verifier) -> u64                                      | Per-verifier attestation count.   | View.                  | Stored in instance storage.                                    |
| set_rate_limit(caller, function, window, max_calls) -> Result                 | Configure rate limits.            | Admin require_auth.    | Uses shared RateLimiter.                                       |
| set_rate_limit_exempt(caller, verifier, exempt) -> Result                     | Configure rate limit exemption.   | Admin require_auth.    | Uses shared RateLimiter.                                       |

## allocation_logic

| Function                                                                       | Summary                                 | Access control       | Notes                                     |
| ------------------------------------------------------------------------------ | --------------------------------------- | -------------------- | ----------------------------------------- |
| initialize(admin, commitment_core) -> Result                                   | Set admin, core contract, and registry. | Admin require_auth.  | Returns AlreadyInitialized on repeat.     |
| register_pool(admin, pool_id, risk_level, apy, max_capacity) -> Result         | Register investment pool.               | Admin require_auth.  | Validates capacity and APY.               |
| update_pool_status(admin, pool_id, active) -> Result                           | Activate/deactivate pool.               | Admin require_auth.  | Updates pool timestamps.                  |
| update_pool_capacity(admin, pool_id, new_capacity) -> Result                   | Update pool capacity.                   | Admin require_auth.  | Ensures capacity >= liquidity.            |
| allocate(caller, commitment_id, amount, strategy) -> Result<AllocationSummary> | Allocate funds across pools.            | caller.require_auth. | Uses rate limiting and reentrancy guard.  |
| rebalance(caller, commitment_id) -> Result<AllocationSummary>                  | Reallocate using stored strategy.       | caller.require_auth. | Requires caller matches allocation owner. |
| get_allocation(commitment_id) -> AllocationSummary                             | Fetch allocation summary.               | View.                | Returns empty summary if missing.         |
| get_pool(pool_id) -> Result<Pool>                                              | Fetch pool info.                        | View.                | Returns PoolNotFound if missing.          |
| get_all_pools() -> Vec<Pool>                                                   | Fetch all pools.                        | View.                | Iterates registry.                        |
| is_initialized() -> bool                                                       | Check initialization flag.              | View.                | Returns false if uninitialized.           |
| set_rate_limit(admin, function, window, max_calls) -> Result                   | Configure rate limits.                  | Admin require_auth.  | Uses shared RateLimiter.                  |
| set_rate_limit_exempt(admin, address, exempt) -> Result                        | Configure rate limit exemption.         | Admin require_auth.  | Uses shared RateLimiter.                  |

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

```bash
cargo test --package commitment_nft test_transfer
```

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
