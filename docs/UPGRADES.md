# Upgrades

This repository uses Soroban's native in-place upgrade mechanism. Each contract:
- Stores an on-chain version in instance storage.
- Exposes `get_version`, `upgrade`, and `migrate`.
- Requires admin authorization for upgrades and migrations.
- Preserves state across upgrades.

## Upgrade Procedure (Step-by-Step)
1. Build new WASM binaries (see `scripts/build-contracts.sh`).
2. Upload the WASM to the target network to obtain a hash.
3. Call `upgrade(admin, new_wasm_hash)` on the target contract.
4. Read `get_version()` to determine the stored version.
5. If `get_version() < CURRENT_VERSION`, call `migrate(admin, from_version)`.
6. Verify invariants and key state (admin, core contract links, counters).
7. Update off-chain configuration and deployment metadata if needed.

## Version History (Current)
- `commitment_core`: `CURRENT_VERSION = 1` - version tracking + upgrade entrypoints (no storage layout changes).
- `commitment_nft`: `CURRENT_VERSION = 1` - version tracking + upgrade entrypoints (no storage layout changes).
- `attestation_engine`: `CURRENT_VERSION = 1` - version tracking + upgrade entrypoints (no storage layout changes).
- `allocation_logic`: `CURRENT_VERSION = 1` - version tracking + upgrade entrypoints (no storage layout changes).
- `price_oracle`: `CURRENT_VERSION = 1` - introduces `OracleConfig` storage and migrates from legacy `MaxStalenessSeconds`.

## Migration Requirements
- `commitment_core`: ensures counters/guards exist; preserves commitments and owner lists.
- `commitment_nft`: ensures token counters and registries exist; preserves NFTs and ownership data.
- `attestation_engine`: ensures analytics counters exist; preserves attestations and metrics.
- `allocation_logic`: ensures pool registry exists; preserves pools and allocations.
- `price_oracle`: migrates `MaxStalenessSeconds` (legacy) into `OracleConfig` and removes the legacy key.

Migrations are admin-only and guarded:
- Downgrades are rejected.
- Replays are rejected once the current version is reached.
- `from_version` must match the stored version.

## Security Risks and Mitigations
- **Admin key compromise**: Upgrades are admin-gated. Use multisig or timelock governance for production deployments.
- **Malicious upgrades**: Require code review, reproducible builds, and independent verification of WASM hashes.
- **Operational mistakes**: Run migrations immediately after upgrades and validate state invariants.

## Breaking Changes Policy
- External interface changes must be documented and versioned.
- Any storage layout change must bump `CURRENT_VERSION` and include a `migrate` step.
- Downgrades are not supported.
