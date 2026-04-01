# Contract Functions

This document summarizes public entry points for each contract and their access control expectations.

## commitment_core

| Function                                                              | Summary                                          | Access control                            | Notes                                              |
| --------------------------------------------------------------------- | ------------------------------------------------ | ----------------------------------------- | -------------------------------------------------- |
| initialize(admin, nft_contract)                                       | Set admin, NFT contract, and counters.           | None (single-use).                        | Panics if already initialized.                     |
| create_commitment(owner, amount, asset_address, rules) -> String      | Creates commitment, transfers assets, mints NFT. | owner.require_auth; caller supplies owner.   | Uses reentrancy guard and rate limiting per owner. |
| get_commitment(commitment_id) -> Commitment                           | Fetch commitment details.                        | View.                                     | Panics if not found.                               |
| list_commitments_by_owner(owner) -> Vec<String>                       | List commitment IDs for owner (convenience).     | View.                                     | Wrapper around get_owner_commitments.              |
| get_owner_commitments(owner) -> Vec<String>                           | List commitment IDs for owner.                   | View.                                     | Returns empty Vec if none.                         |
| list_commitments_by_owner(owner) -> Vec<String>                     | List commitment IDs for owner (alias).           | View.                                     | Same as get_owner_commitments.                      |
| get_commitments_created_between(from_ts, to_ts) -> Vec<String>    | Get commitments created in time range.           | View.                                     | O(n) cost; use for analytics.                      |
| get_total_commitments() -> u64                                        | Total commitments count.                         | View.                                     | Reads instance storage counter.                    |
| get_total_value_locked() -> i128                                      | Total value locked across commitments.           | View.                                     | Aggregate stored in instance storage.              |
| get_commitments_created_between(from_ts, to_ts) -> Vec<String>        | Get commitments created in time range.           | View.                                     | O(n) cost; use pagination for large datasets.     |
| get_admin() -> Address                                                | Fetch admin address.                             | View.                                     | Panics if not initialized.                         |
| pause(caller)                                                          | Pause contract operations.                      | Admin require_auth.                        | Prevents state-changing operations.                |
| unpause(caller)                                                        | Unpause contract operations.                    | Admin require_auth.                        | Re-enables state-changing operations.              |
| is_paused() -> bool                                                    | Check if contract is paused.                     | View.                                     | Returns pause state.                               |
| get_nft_contract() -> Address                                         | Fetch NFT contract address.                      | View.                                     | Panics if not initialized.                         |
| pause(caller)                                                         | Pause contract operations.                       | Admin require_auth.                        | Uses Pausable utility.                             |
| unpause(caller)                                                       | Unpause contract operations.                     | Admin require_auth.                        | Uses Pausable utility.                             |
| is_paused() -> bool                                                   | Check if contract is paused.                     | View.                                     | Returns pause state.                               |
| add_authorized_contract(caller, contract_address)                    | Add authorized allocator contract.               | Admin require_auth.                        | Stores authorization flag.                          |
| remove_authorized_contract(caller, contract_address)                 | Remove authorized allocator contract.            | Admin require_auth.                        | Removes authorization flag.                        |
| is_authorized(contract_address) -> bool                              | Check if contract is authorized.                 | View.                                     | Admin is implicitly authorized.                    |
| update_value(commitment_id, new_value)                                | Emit value update event.                         | No require_auth.                          | Updates stored commitment value and TVL.           |
| check_violations(commitment_id) -> bool                               | Evaluate loss or duration violations.            | View.                                     | Emits violation event when violated.               |
| get_violation_details(commitment_id) -> (bool, bool, bool, i128, u64) | Detailed violation info.                         | View.                                     | Calculates loss percent and time remaining.        |
| settle(commitment_id)                                                 | Settle expired commitment and NFT.               | No require_auth.                          | Transfers assets and calls NFT settle.             |
| early_exit(commitment_id, caller)                                     | Exit early with penalty.                         | caller.require_auth + owner check.         | Uses SafeMath to compute penalty.                  |
| allocate(caller, commitment_id, target_pool, amount)                          | Allocate assets to pool.                         | caller.require_auth + admin or authorized allocator. | Transfers assets to target pool.                   |
| set_rate_limit(caller, function, window, max_calls)                   | Configure rate limits.                           | Admin only.                               | Uses shared RateLimiter.                           |
| set_rate_limit_exempt(caller, address, exempt)                        | Configure rate limit exemption.                  | Admin only.                               | Uses shared RateLimiter.                           |
| set_creation_fee_bps(caller, bps)                                     | Set creation fee rate in basis points.           | Admin only.                               | Fee rate 0-10000 bps (100 bps = 1%).               |
| set_fee_recipient(caller, recipient)                                  | Set protocol treasury for fee withdrawals.       | Admin only.                               | Validates recipient is not zero address.           |
| withdraw_fees(caller, asset_address, amount)                          | Withdraw collected fees to recipient.            | Admin only.                               | Requires recipient set, sufficient fees collected. |
| get_creation_fee_bps() -> u32                                         | Get current creation fee rate.                   | View.                                     | Returns 0 if not set.                              |
| get_fee_recipient() -> Option<Address>                                | Get configured fee recipient.                    | View.                                     | Returns None if not set.                           |
| get_collected_fees(asset_address) -> i128                             | Get collected fees for an asset.                 | View.                                     | Returns 0 if none collected.                       |

### commitment_core cross-contract notes

- `create_commitment` is the main outbound write edge into `commitment_nft`; it also moves user assets into core custody.
- `settle` and `early_exit` both depend on downstream NFT lifecycle calls to keep mirrored state aligned.
- `get_commitment` is the canonical read edge consumed by `attestation_engine`.
- `allocate` transfers assets to authorized pool contracts and requires caller authorization.
- `update_value` modifies stored commitment state and TVL, emitting events for downstream consumers.
- Fee management functions (`set_creation_fee_bps`, `withdraw_fees`, etc.) handle protocol revenue collection.
- Emergency control functions (`emergency_withdraw`, `set_emergency_mode`) provide admin safety controls.
- Cross-contract review reference: `docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md`

## commitment_interface

`commitment_interface` is an ABI-only crate. It mirrors the live
`commitment_core` commitment schema, event payloads, and the core read-only
entrypoints that downstream bindings commonly consume. CI drift checks compare
its source-defined types and expected signatures against `commitment_core` and
`attestation_engine`.

| Function                                                                 | Summary                                     | Access control            | Notes                                                                   |
| ------------------------------------------------------------------------ | ------------------------------------------- | ------------------------- | ----------------------------------------------------------------------- |
| initialize(admin, nft_contract) -> Result                                | Initialize admin and linked NFT contract.   | Interface only.           | Live core contract is single-use; no state exists in this crate.        |
| create_commitment(owner, amount, asset_address, rules) -> Result<String> | Create a commitment and return string id.   | Interface only.           | Mirrors live `commitment_core` types, including `CommitmentRules`.      |
| get_commitment(commitment_id) -> Result<Commitment>                      | Fetch the canonical commitment record.      | View in live contract.    | `Commitment` shape is drift-checked against `commitment_core`.          |
| get_owner_commitments(owner) -> Result<Vec<String>>                      | List commitment ids owned by an address.    | View in live contract.    | Used by UIs and indexers.                                               |
| get_total_commitments() -> Result<u64>                                   | Read the total commitment counter.          | View in live contract.    | Counter is stored by the live core contract.                            |
| settle(commitment_id) -> Result                                          | Settle an expired commitment.               | Mutating in live contract | Live implementation performs token and NFT cross-contract interactions. |
| early_exit(commitment_id, caller) -> Result                              | Exit a commitment early with penalty logic. | Mutating in live contract | Live implementation must enforce caller auth and overflow-safe math.    |

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

| Function                                                                      | Summary                           | Access control         | Notes                                                                                         |
| ----------------------------------------------------------------------------- | --------------------------------- | ---------------------- | --------------------------------------------------------------------------------------------- |
| initialize(admin, commitment_core) -> Result                                  | Set admin and core contract.      | None (single-use).     | Returns AlreadyInitialized on repeat.                                                         |
| add_verifier(caller, verifier) -> Result                                      | Authorize verifier address.       | Admin require_auth.    | Stores verifier flag.                                                                         |
| remove_verifier(caller, verifier) -> Result                                   | Remove verifier authorization.    | Admin require_auth.    | Removes verifier flag.                                                                        |
| is_verifier(address) -> bool                                                  | Check verifier authorization.     | View.                  | Admin is implicitly authorized.                                                               |
| get_admin() -> Result<Address>                                                | Fetch admin address.              | View.                  | Fails if not initialized.                                                                     |
| get_core_contract() -> Result<Address>                                        | Fetch core contract address.      | View.                  | Fails if not initialized.                                                                     |
| get_stored_health_metrics(commitment_id) -> Option<HealthMetrics>             | Fetch cached health metrics.      | View.                  | Returns None if missing.                                                                      |
| attest(caller, commitment_id, attestation_type, data, is_compliant) -> Result | Record attestation.               | Verifier require_auth. | Validates commitment, uses rate limiting and reentrancy guard.                                |
| get_attestations(commitment_id) -> Vec<Attestation>                           | List attestations for commitment. | View.                  | Returns empty Vec if none.                                                                    |
| get_attestations_page(commitment_id, offset, limit) -> AttestationsPage       | Paginated attestations.           | View.                  | Order: timestamp (oldest first). Max page size MAX_PAGE_SIZE=100. next_offset=0 when no more. |
| get_attestation_count(commitment_id) -> u64                                   | Count attestations.               | View.                  | Stored in persistent storage.                                                                 |
| get_health_metrics(commitment_id) -> HealthMetrics                            | Compute current health metrics.   | View.                  | Reads commitment_core data.                                                                   |
| verify_compliance(commitment_id) -> bool                                      | Check compliance vs rules.        | View.                  | Uses health metrics and rules.                                                                |
| record_fees(caller, commitment_id, fee_amount) -> Result                      | Convenience fee attestation.      | Verifier require_auth. | Calls attest() internally.                                                                    |
| record_drawdown(caller, commitment_id, drawdown_percent) -> Result            | Convenience drawdown attestation. | Verifier require_auth. | Calls attest() internally.                                                                    |
| calculate_compliance_score(commitment_id) -> u32                              | Compute compliance score.         | View.                  | Emits ScoreUpd event.                                                                         |
| get_protocol_statistics() -> (u64, u64, u64, i128)                            | Aggregate protocol stats.         | View.                  | Reads commitment_core counters.                                                               |
| get_verifier_statistics(verifier) -> u64                                      | Per-verifier attestation count.   | View.                  | Stored in instance storage.                                                                   |
| set_rate_limit(caller, function, window, max_calls) -> Result                 | Configure rate limits.            | Admin require_auth.    | Uses shared RateLimiter.                                                                      |
| set_rate_limit_exempt(caller, verifier, exempt) -> Result                     | Configure rate limit exemption.   | Admin require_auth.    | Uses shared RateLimiter.                                                                      |

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

Operational guide: `docs/ALLOCATION_LOGIC_POOL_REGISTRY_AND_RISK_LEVELS.md`

## price_oracle

| Function                                                            | Summary                                    | Access control       | Notes                                                                         |
| ------------------------------------------------------------------- | ------------------------------------------ | -------------------- | ----------------------------------------------------------------------------- |
| initialize(admin) -> Result                                         | Set admin and default staleness window.    | None (single-use).   | Initializes whitelist authority and versioned config.                         |
| add_oracle(caller, oracle_address) -> Result                        | Add a trusted price publisher.             | Admin require_auth.  | Whitelisted oracle can overwrite the latest price for any asset it updates.   |
| remove_oracle(caller, oracle_address) -> Result                     | Remove a trusted price publisher.          | Admin require_auth.  | Prevents further updates from that address.                                   |
| is_oracle_whitelisted(address) -> bool                              | Check whitelist membership.                | View.                | Reads the admin-managed trust list.                                           |
| set_price(caller, asset, price, decimals) -> Result                 | Publish latest price for an asset.         | Oracle require_auth. | Validates non-negative price; does not aggregate or reconcile multiple feeds. |
| get_price(asset) -> PriceData                                       | Read the raw latest price snapshot.        | View.                | Returns zeroed `PriceData` if unset; does not enforce freshness.              |
| get_price_valid(asset, max_staleness_override) -> Result<PriceData> | Read a fresh price snapshot or fail.       | View.                | Rejects stale and future-dated data; preferred for security-sensitive reads.  |
| set_max_staleness(caller, seconds) -> Result                        | Update default freshness window.           | Admin require_auth.  | Tunes rejection threshold for delayed oracle updates.                         |
| get_max_staleness() -> u64                                          | Read default freshness window.             | View.                | Used when `get_price_valid` has no override.                                  |
| get_admin() -> Address                                              | Read oracle admin.                         | View.                | Returns the current whitelist/config authority.                               |
| set_admin(caller, new_admin) -> Result                              | Transfer oracle admin authority.           | Admin require_auth.  | Transfers control over whitelist and configuration.                           |
| upgrade(caller, new_wasm_hash) -> Result                            | Upgrade contract code.                     | Admin require_auth.  | Validates non-zero WASM hash.                                                 |
| migrate(caller, from_version) -> Result                             | Migrate legacy storage to current version. | Admin require_auth.  | Replays are blocked once current version is installed.                        |

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

```bash
cargo test --package commitment_nft test_transfer
```

## time_lock

| Function                                                      | Summary                                               | Access control               | Notes                                                                          |
| ------------------------------------------------------------- | ----------------------------------------------------- | ---------------------------- | ------------------------------------------------------------------------------ |
| initialize(admin)                                             | Set the initial timelock admin.                       | None (single-use).           | Establishes the authority allowed to queue and cancel actions.                 |
| queue_action(action_type, target, data, delay) -> Result<u64> | Queue a delayed governance action.                    | Stored admin `require_auth`. | Delay must be at least the action-type minimum and no more than 30 days.       |
| execute_action(action_id) -> Result                           | Execute a matured action.                             | Permissionless after delay.  | Anyone may execute once `executable_at` is reached.                            |
| cancel_action(action_id) -> Result                            | Cancel a queued action.                               | Stored admin `require_auth`. | Fails if the action already executed or was already cancelled.                 |
| get_action(action_id) -> Result<QueuedAction>                 | Read queued action metadata.                          | View.                        | Includes `queued_at`, `executable_at`, and execution state.                    |
| get_all_actions() -> Vec<u64>                                 | Read all queued action ids.                           | View.                        | Includes executed and cancelled actions.                                       |
| get_pending_actions() -> Vec<u64>                             | Read actions that are neither executed nor cancelled. | View.                        | Useful for operator review and execution scans.                                |
| get_executable_actions() -> Vec<u64>                          | Read pending actions whose delay has elapsed.         | View.                        | Actions are executable at exactly `executable_at`.                             |
| get_admin() -> Address                                        | Read the current admin.                               | View.                        | Returns the authority for queue/cancel operations.                             |
| get_min_delay(action_type) -> u64                             | Read the minimum delay for an action type.            | View.                        | Current floors: 1 day for parameter/fee, 2 days for admin, 3 days for upgrade. |
| get_max_delay() -> u64                                        | Read the global maximum allowed delay.                | View.                        | Hard cap is 30 days.                                                           |
| get_action_count() -> u64                                     | Read total number of queued actions.                  | View.                        | Monotonic counter for action ids.                                              |

### time_lock operational notes

- Queueing and cancellation are admin-authorized operations; execution is intentionally permissionless after the delay.
- Operators should record `action_id`, `queued_at`, and `executable_at` immediately after queueing.
- Use the smallest action type that accurately reflects blast radius, but not the smallest delay by default.
- Runbook reference: `docs/TIMELOCK_RUNBOOK.md#timelock-parameter-runbook`

## commitment_transformation

Transforms commitments into risk tranches, collateralized assets, and secondary market instruments.
All mutating functions require the caller to be the admin or an explicitly authorized transformer address.
A reentrancy guard is active for every state-changing call.

| Function | Summary | Access control | Notes |
| --- | --- | --- | --- |
| initialize(admin, core_contract) | Set admin, core contract reference, and counters. | None (single-use). | Panics with `AlreadyInitialized` on repeat. |
| set_transformation_fee(caller, fee_bps) | Set protocol fee in basis points (0–10 000). | Admin `require_auth`. | Panics with `InvalidFeeBps` if fee_bps > 10 000. |
| set_authorized_transformer(caller, transformer, allowed) | Grant or revoke transformer authorization. | Admin `require_auth`. | Emits `AuthSet` event. |
| create_tranches(caller, commitment_id, total_value, tranche_share_bps, risk_levels, fee_asset) -> String | Split a commitment into risk tranches. | Authorized transformer `require_auth`. | `tranche_share_bps` must sum to exactly 10 000; lengths must match; `total_value` must be positive. Returns `transformation_id`. |
| collateralize(caller, commitment_id, collateral_amount, asset_address) -> String | Create a collateralized asset record. | Authorized transformer `require_auth`. | `collateral_amount` must be positive. Returns `asset_id`. |
| create_secondary_instrument(caller, commitment_id, instrument_type, amount) -> String | Create a secondary market instrument (receivable, option, warrant). | Authorized transformer `require_auth`. | `amount` must be positive. Returns `instrument_id`. |
| add_protocol_guarantee(caller, commitment_id, guarantee_type, terms_hash) -> String | Attach a protocol guarantee to a commitment. | Authorized transformer `require_auth`. | Returns `guarantee_id`. |
| get_tranche_set(transformation_id) -> TrancheSet | Fetch a tranche set by ID. | View. | Panics with `TransformationNotFound` if missing. |
| get_collateralized_asset(asset_id) -> CollateralizedAsset | Fetch a collateralized asset by ID. | View. | Panics with `TransformationNotFound` if missing. |
| get_secondary_instrument(instrument_id) -> SecondaryInstrument | Fetch a secondary instrument by ID. | View. | Panics with `TransformationNotFound` if missing. |
| get_protocol_guarantee(guarantee_id) -> ProtocolGuarantee | Fetch a protocol guarantee by ID. | View. | Panics with `TransformationNotFound` if missing. |
| get_commitment_tranche_sets(commitment_id) -> Vec\<String\> | List tranche set IDs for a commitment. | View. | Returns empty Vec if none. |
| get_commitment_collateral(commitment_id) -> Vec\<String\> | List collateralized asset IDs for a commitment. | View. | Returns empty Vec if none. |
| get_commitment_instruments(commitment_id) -> Vec\<String\> | List secondary instrument IDs for a commitment. | View. | Returns empty Vec if none. |
| get_commitment_guarantees(commitment_id) -> Vec\<String\> | List protocol guarantee IDs for a commitment. | View. | Returns empty Vec if none. |
| get_admin() -> Address | Fetch admin address. | View. | Panics with `NotInitialized` if unset. |
| get_transformation_fee_bps() -> u32 | Fetch current fee in basis points. | View. | Returns 0 if unset. |
| set_fee_recipient(caller, recipient) | Set protocol treasury address for fee withdrawals. | Admin `require_auth`. | Emits `FeeRecip` event. |
| withdraw_fees(caller, asset_address, amount) | Withdraw collected fees to the fee recipient. | Admin `require_auth`. | Panics with `FeeRecipientNotSet` or `InsufficientFees` on invalid state. |
| get_fee_recipient() -> Option\<Address\> | Fetch fee recipient address. | View. | Returns `None` if not set. |
| get_collected_fees(asset_address) -> i128 | Fetch accumulated fees for an asset. | View. | Returns 0 if no fees collected. |

### commitment_transformation — tranche ratio invariant

`create_tranches` enforces a strict invariant: **`sum(tranche_share_bps) == 10 000`** (representing 100%).
Any deviation — including off-by-one, empty vectors, all-zero entries, or mismatched lengths — panics with
`InvalidTrancheRatios` ("Tranche ratios must sum to 100") and rolls back the reentrancy guard before returning.

Fee formula: `fee_amount = (total_value × fee_bps) / 10 000`. When `fee_bps == 0` no token transfer occurs.
Each tranche amount is computed as `(net_value × share_bps) / 10 000` where `net_value = total_value − fee_amount`.

### commitment_transformation — error codes

| Code | Name | Trigger |
| --- | --- | --- |
| 1 | `InvalidAmount` | `total_value ≤ 0` or `withdraw_fees` amount ≤ 0 |
| 2 | `InvalidTrancheRatios` | `sum(bps) ≠ 10 000`, empty vector, or length mismatch |
| 3 | `InvalidFeeBps` | `fee_bps > 10 000` |
| 4 | `Unauthorized` | Caller is neither admin nor authorized transformer |
| 5 | `NotInitialized` | Contract not yet initialized |
| 6 | `AlreadyInitialized` | `initialize` called more than once |
| 8 | `TransformationNotFound` | Requested ID does not exist in storage |
| 10 | `ReentrancyDetected` | Reentrant call detected via guard flag |
| 11 | `FeeRecipientNotSet` | `withdraw_fees` called before `set_fee_recipient` |
| 12 | `InsufficientFees` | Requested withdrawal exceeds collected balance |

### commitment_transformation — test coverage (issue #257)

Tests live in `contracts/commitment_transformation/src/tests.rs`.

**Success paths (`sum == 10 000`):**

| Test | Scenario |
| --- | --- |
| `test_create_tranches_single_100pct` | One tranche at 10 000 bps |
| `test_create_tranches_two_equal_halves` | Two tranches at 5 000 + 5 000 bps |
| `test_create_tranches_classic_three_way` | 6 000 + 3 000 + 1 000 bps; verifies per-tranche amounts |
| `test_create_tranches_four_tranches` | 4 000 + 3 000 + 2 000 + 1 000 bps |
| `test_create_tranches_amounts_sum_to_net_value` | Non-round total_value; verifies bps sum and non-negative amounts |
| `test_create_tranches_multiple_sets_same_commitment` | Two sets on same commitment_id accumulate correctly |
| `test_transformation_with_zero_fee` | fee_bps = 0; fee_paid = 0, total_value preserved |

**Error paths (`sum ≠ 10 000`):**

| Test | Scenario |
| --- | --- |
| `test_create_tranches_sum_below_10000` | 5 000 + 3 000 = 8 000 |
| `test_create_tranches_sum_above_10000` | 6 000 + 5 000 = 11 000 |
| `test_create_tranches_all_zeros` | Three entries all 0 |
| `test_create_tranches_empty_bps_vector` | Empty Vec |
| `test_create_tranches_off_by_one_below` | 5 000 + 4 999 = 9 999 |
| `test_create_tranches_off_by_one_above` | 5 001 + 5 000 = 10 001 |
| `test_create_tranches_mismatched_lengths` | bps.len = 2, risk_levels.len = 1 |
| `test_create_tranches_unauthorized` | Caller not in authorized set |

Run:

```bash
cargo test -p commitment_transformation
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
