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
