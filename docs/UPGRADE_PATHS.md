# Upgrade Paths

## Current posture
- Contracts support **native Soroban upgrades** with state preservation.
- Admin-only `upgrade` and `migrate` entrypoints are available on production contracts.

## Recommended upgrade process
1. **Upload new WASM** and obtain the hash.
2. **Call `upgrade`** as admin on the target contract.
3. **Call `migrate`** if `get_version()` is less than `CURRENT_VERSION`.
4. **Verify state** (admin, contract links, counters, and key invariants).
5. **Update off-chain metadata** if needed.

For full details, see `docs/UPGRADES.md`.

## commitment_core
- `upgrade(caller, new_wasm_hash)` requires admin authorization and rejects the all-zero WASM hash.
- `migrate(caller, from_version)` accepts legacy version `0` only when the stored `Version` is also `0`, then writes `CURRENT_VERSION`.
- Migration backfills `TotalCommitments`, `TotalValueLocked`, `AllCommitmentIds`, and `ReentrancyGuard` only when those keys are missing.
- Migration preserves commitments, owner lists, fee state, asset custody, and create/settle/early-exit behavior.

## Migration idempotency guarantees
- `migrate(caller, from_version)` is admin-only and rejects non-admin callers before writing state.
- `from_version` must match the stored `Version`; mismatches return `InvalidVersion` and leave `Version` unchanged.
- After a successful migration, `Version` is set to the current contract version; repeat calls return `AlreadyMigrated` without changing stored data.
- Migration initialises missing analytics/registry keys needed by the current version while preserving existing counters, pool registries, and allocation state.
