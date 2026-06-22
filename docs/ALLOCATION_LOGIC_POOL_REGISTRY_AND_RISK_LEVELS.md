# allocation_logic: Pool Registry and Risk Levels

This page is an operational guide for integrators interacting with the `allocation_logic` Soroban contract, focusing on pool registry operations and risk-level behavior.

## Trust Boundaries and Authorization

- Pool registry mutations are **admin-authorized**:
  - `register_pool(admin, ...)`
  - `update_pool_status(admin, ...)`
  - `update_pool_capacity(admin, ...)`
  - All require `admin.require_auth()` and `admin` must match the stored `Admin`.
- Allocation entry points are **caller-authorized**:
  - `allocate(caller, ...)` requires `caller.require_auth()`.
  - `rebalance(caller, commitment_id)` requires `caller.require_auth()` and `caller` must match the stored allocation owner for `commitment_id`.
- Batch allocation is **admin/operator-authorized**:
  - `batch_allocate(admin, params, mode)` requires `admin.require_auth()` and `admin` must match the stored `Admin`.
  - Each batch item still uses the regular `allocate(caller, ...)` path, so the item caller must also authorize and rate-limit/capacity/reentrancy checks remain unchanged.

The contract does not validate commitment ownership against `commitment_core`; allocations are tracked locally within `allocation_logic` (see `docs/ARCHITECTURE.md`).

## Pool Registry

The pool registry is a contract-maintained list of pool ids (instance storage `DataKey::PoolRegistry`). Pool metadata is stored under persistent storage key `DataKey::Pool(pool_id)`.

### Registering a Pool

`register_pool(admin, pool_id, risk_level, apy, max_capacity) -> Result<(), Error>`

Validation and invariants:

- `pool_id` must be unique (registering an existing id fails).
- `max_capacity` must be `> 0`.
- `apy` must be `<= 100_000` (basis points scale in contract docs/comments; see Rustdoc for details).
- Newly registered pools start as `active = true`, with `total_liquidity = 0`.
- The pool id is appended to `PoolRegistry`.

### Updating Pool Active Status

`update_pool_status(admin, pool_id, active) -> Result<(), Error>`

- Sets `pool.active = active` and updates `updated_at`.
- Allocation behavior:
  - `allocate` and `rebalance` select eligible pools from `PoolRegistry` but filter out inactive pools.
  - Inactive pools are not chosen for new allocations.

### Updating Pool Capacity

`update_pool_capacity(admin, pool_id, new_capacity) -> Result<(), Error>`

- `new_capacity` must be `> 0`.
- `new_capacity` must be `>= total_liquidity` (cannot set capacity below already allocated liquidity).

### Listing Pools

`get_all_pools() -> Vec<Pool>`

- Returns pools by iterating `PoolRegistry` and fetching each `Pool(pool_id)`.
- Note: this call is registry-based and returns both active and inactive pools. Use the `Pool.active` field to filter client-side when needed.

## Risk Levels and Strategy Mapping

`RiskLevel` is a coarse risk classification attached to each pool at registration time:

- `Low`
- `Medium`
- `High`

`Strategy` selects which risk levels are eligible during pool selection:

- `Safe`: `Low` pools only
- `Balanced`: `Low` + `Medium` + `High`
- `Aggressive`: `Medium` + `High`

## Allocation Rounding, Determinism, and Capacity Failure

All allocations use integer arithmetic and are deterministic given the same on-chain state.

### Deterministic Remainder Handling

When an amount is split across pools (or across risk-level buckets and then pools), integer division can produce a remainder. The contract assigns remainder units deterministically in the same order pools are iterated (registry order after filtering for strategy and `active`).

Balanced strategy risk split:

- Low bucket: 40% (floor)
- Medium bucket: 40% (floor)
- High bucket: remainder so that `low + medium + high == amount`

Aggressive strategy risk split:

- Medium bucket: 30% (floor)
- High bucket: remainder so that `medium + high == amount`

Within each bucket, the bucket amount is distributed across pools with the same deterministic remainder behavior.

### Capacity Enforcement and Failure Mode

Each pool has an enforced capacity:

- `total_liquidity` is increased on allocation
- `max_capacity` is an upper bound for `total_liquidity`

If the requested amount cannot be fully satisfied across the eligible pools due to capacity constraints, `allocate` fails with:

- `Error::PoolCapacityExceeded`

This is a hard failure (no partial success), and state changes are reverted by the transaction.

## Batch Allocation Semantics

`batch_allocate(admin, params, mode) -> BatchAllocateResult`

Batch allocation accepts a bounded vector of `BatchAllocateParams` and uses `shared_utils::BatchProcessor` limits. The default maximum batch size is 50 unless the shared batch configuration is overridden.

Modes:

- `BatchMode::Atomic`: pre-validates all items against a simulated pool-liquidity view. If any item would fail, the result contains one `BatchError` and no allocations are written.
- `BatchMode::BestEffort`: processes items independently. Successful items are committed, failing items are reported by index in `errors`, and later items continue.

Per-item behavior:

- Each item follows the same allocation logic as `allocate`, including commitment balance/status lookup, pool activity checks, deterministic rounding, capacity enforcement, duplicate-allocation rejection, and reentrancy cleanup.
- Capacity checks account for earlier successful items in the same batch. In atomic mode this happens during preflight; in best-effort mode this happens naturally as successful items update pool liquidity.
- Empty batches and batches over the configured limit return a failed `BatchAllocateResult` with a batch-size validation error.

## Capacity Boundary Test Matrix

The following matrix documents the exhaustive boundary scenarios covered by the test suite (`contracts/allocation_logic/src/tests.rs`, issue #480):

| Scenario | Function | Expected Outcome | Test |
|---|---|---|---|
| `amount == max_capacity` | `allocate` | Accepted (`total_liquidity == max_capacity`) | `test_allocate_exactly_at_max_capacity_accepted` |
| `amount == max_capacity + 1` | `allocate` | `Error::PoolCapacityExceeded` (#7) | `test_allocate_one_over_max_capacity_rejected` |
| Rebalance across pools at cap | `rebalance` | Accepted; all pools remain `<= max_capacity` | `test_rebalance_across_pools_respects_capacity` |
| `new_capacity < total_liquidity` | `update_pool_capacity` | `Error::PoolCapacityExceeded` (#7) | `test_update_pool_capacity_below_total_liquidity_rejected` |
| `total_liquidity` after rebalance | `rebalance` | Never underflows (`>= 0`) | `test_rebalance_total_liquidity_no_underflow` |
