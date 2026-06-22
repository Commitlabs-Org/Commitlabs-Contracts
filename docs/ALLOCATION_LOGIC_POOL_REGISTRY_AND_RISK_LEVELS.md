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

## Weighted Allocation, Rounding, and Capacity Failure

All allocations use integer arithmetic and are deterministic given the same on-chain state.

### APY and Headroom Weighting

Within each eligible risk bucket, allocation is weighted by both yield and available capacity:

`pool_weight = pool.apy * (pool.max_capacity - pool.total_liquidity)`

Each pool receives:

`floor(bucket_amount * pool_weight / total_bucket_weight)`

If all pools in a bucket have `apy == 0`, the contract falls back to headroom-only weights so otherwise usable capacity is not stranded:

`pool_weight = pool.max_capacity - pool.total_liquidity`

Checked arithmetic is used for the weight and allocation products. If the eligible pools cannot fully satisfy the requested amount, allocation fails with `Error::PoolCapacityExceeded`.

### Strategy Risk Buckets

Balanced strategy risk split:

- Low bucket: 40% (floor)
- Medium bucket: 40% (floor)
- High bucket: remainder so that `low + medium + high == amount`

Aggressive strategy risk split:

- Medium bucket: 30% (floor)
- High bucket: remainder so that `medium + high == amount`

Within each bucket, the bucket amount is distributed across pools with the same deterministic remainder behavior.

### Deterministic Remainder Handling

When weighted integer division creates dust, remainder units are assigned to pools with the largest fractional remainder first, with registry order as the deterministic tie-breaker. If pool caps still leave a shortfall after weighted targets are exhausted, remaining capacity is filled in registry order.

### Capacity Enforcement and Failure Mode

Each pool has an enforced capacity:

- `total_liquidity` is increased on allocation
- `max_capacity` is an upper bound for `total_liquidity`

If the requested amount cannot be fully satisfied across the eligible pools due to capacity constraints, `allocate` fails with:

- `Error::PoolCapacityExceeded`

This is a hard failure (no partial success), and state changes are reverted by the transaction.


## Capacity Boundary Test Matrix

The following matrix documents the exhaustive boundary scenarios covered by the test suite (`contracts/allocation_logic/src/tests.rs`, issue #480):

| Scenario | Function | Expected Outcome | Test |
|---|---|---|---|
| `amount == max_capacity` | `allocate` | Accepted (`total_liquidity == max_capacity`) | `test_allocate_exactly_at_max_capacity_accepted` |
| `amount == max_capacity + 1` | `allocate` | `Error::PoolCapacityExceeded` (#7) | `test_allocate_one_over_max_capacity_rejected` |
| Rebalance across pools at cap | `rebalance` | Accepted; all pools remain `<= max_capacity` | `test_rebalance_across_pools_respects_capacity` |
| `new_capacity < total_liquidity` | `update_pool_capacity` | `Error::PoolCapacityExceeded` (#7) | `test_update_pool_capacity_below_total_liquidity_rejected` |
| `total_liquidity` after rebalance | `rebalance` | Never underflows (`>= 0`) | `test_rebalance_total_liquidity_no_underflow` |
