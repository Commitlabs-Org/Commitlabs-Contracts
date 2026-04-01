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

