# Commitment NFT storage migration

This note defines the storage contract used by `CommitmentNFTContract` and the
preflight rules for moving an already-deployed contract to the current WASM.
The implementation is in `src/lib.rs`; executable fixtures are in
`src/migration_guard_tests.rs`.

## Why a guard is necessary

Soroban contract upgrades preserve storage, but they do not automatically
interpret old storage as new storage. A new field can be absent, and a key can
also have moved between instance and persistent storage. Treating an absent
value as a safe default is only correct when that value cannot affect identity,
ownership, balances, or future allocation.

The NFT contract has an especially important index:

```text
TokenIds -> [token_id, ...]
```

NFT records and owner balances are stored separately. If a migration writes an
empty `TokenIds` list over a populated legacy index, the records still exist,
but enumeration and analytics stop seeing them. The next upgrade may then
appear to have no commitments even though ownership and balances remain in
storage. This is a semantic data-loss bug without deleting a single record.

## Schema inventory

| Key | Namespace | Meaning | Required by migration |
| --- | --- | --- | --- |
| `Admin` | instance | privileged authority | yes |
| `TokenCounter` | instance | next token/supply counter | yes |
| `TokenIds` | instance in v1, persistent after index move | enumeration index | yes, either namespace |
| `NFT(token_id)` | persistent | complete NFT record | preserved, not rewritten |
| `OwnerBalance(owner)` | persistent | owner token count | preserved, not rewritten |
| `OwnerTokens(owner)` | persistent | owner-to-token index | preserved, not rewritten |
| `ActiveStatus(token_id)` | persistent | legacy/auxiliary activity flag | preserved, not rewritten |
| `CoreContract` | instance | trusted lifecycle caller | optional |
| `AuthorizedMinter(address)` | instance | mint authorization | optional |
| `ReentrancyGuard` | instance | mutation guard | initialized if absent |
| `Version` | instance | schema marker | written last |

The migration only creates the destination `TokenIds` index when a valid source
index is already present in the old instance namespace. It never fabricates a
counter, owner balance, NFT record, or owner index. Those values have semantic
meaning and cannot be safely inferred from a partial fixture.

## Version meanings

The current binary uses schema version `2`.

| Version | Interpretation | Migration action |
| --- | --- | --- |
| `0` | legacy deployment with no `Version` marker | validate legacy keys, copy index if needed, initialize guard, mark v2 |
| `1` | versioned pre-v2 deployment | validate legacy keys, copy index if needed, initialize guard, mark v2 |
| `2` | current schema | reject as `AlreadyMigrated` |
| greater than `2` | newer or unknown schema | reject as `UnsupportedStorageVersion` |

Version `0` is not the same as an uninitialized contract. The migration still
requires the `Admin`, `TokenCounter`, and token index keys. A contract with no
admin cannot authorize a migration, and a contract with no counter or index
cannot prove that existing token identity will not be reused or hidden.

New deployments write version `2` during initialization. This removes the
ambiguous window in which a newly created contract looked like an old v0
contract while still allowing genuine v0 deployments to migrate explicitly.

## Preflight and write ordering

Every migration follows this order:

1. Authenticate the caller and compare it with the stored admin.
2. Read the stored version without changing state.
3. Reject current, unsupported, or mismatched source versions.
4. Validate all required source keys without writing destination keys.
5. Read the legacy index from persistent storage, or from the v1 instance
   location when the persistent index is absent.
6. Copy the index only when the persistent destination is absent.
7. Add a default `ReentrancyGuard=false` only when that key is absent.
8. Write `Version=2` as the final migration marker.

If steps 1–5 fail, no migration-owned key has been written. If a later write
fails, Soroban invocation rollback prevents a partial commit. Writing the
version marker last also makes the marker an accurate statement that the
preflight and destination initialization completed.

## Authorization model

`migrate` uses the same `require_admin` path as other administrative NFT
operations. The caller must both:

- provide an on-chain authorization, and
- equal the `Admin` address stored by the contract.

An arbitrary caller cannot initialize a missing admin through migration. A
failed authorization attempt returns `NotAuthorized` and leaves the source
version unchanged. The migration does not accept a deployer-supplied admin or
trust a version argument as proof of authority.

## Existing commitment and balance preservation

Migration does not deserialize and rewrite `NFT(token_id)`,
`OwnerBalance(owner)`, or `OwnerTokens(owner)`. This is deliberate: changing
the representation of a live record is a separate schema operation that needs
its own version and fixture set. The migration under this issue is limited to
the index-location compatibility fix and guard initialization.

The representative fixture test writes an active NFT with a non-zero amount,
an owner balance, and an owner-token index before migration. It then verifies
the NFT record and amount remain unchanged after the old index is copied. The
test also uses two token IDs in the source index so an implementation that
silently keeps only one entry cannot pass by accident.

## Failure matrix

| Condition | Error | Expected state |
| --- | --- | --- |
| source marker is `2` | `AlreadyMigrated` | unchanged |
| source marker is `3+` | `UnsupportedStorageVersion` | unchanged |
| caller is not admin | `NotAuthorized` | unchanged |
| requested source differs from stored marker | `InvalidVersion` | unchanged |
| admin or counter missing | `MigrationSchemaMismatch` | unchanged |
| both token-index locations missing | `MigrationSchemaMismatch` | unchanged |
| valid v0/v1 source | success | index and guard ready, marker is `2` |

“Unchanged” includes the version marker and destination index. In particular,
an invalid partial source must not leave an empty persistent index behind, as
that empty index would mask the reason the migration was rejected and could
make a later operator retry unsafe.

## Idempotency and operational procedure

Operators should inspect `get_version` and confirm the deployment’s expected
source schema before submitting a migration. For a v0 deployment, submit
`migrate(admin, 0)`. For a v1 deployment, submit `migrate(admin, 1)`. A second
submission after success returns `AlreadyMigrated` and performs no writes.

The source token index must be retained until the successful migration is
observed on-chain. This implementation leaves the old instance key intact
after copying it, which makes rollback and forensic comparison easier and
avoids an unnecessary destructive cleanup step. Future migrations may remove
legacy keys only after a separately reviewed retention policy exists.

## Test evidence

The migration fixture suite covers:

- fresh initialization at the current version;
- v0 migration with an absent version marker;
- v1 migration with the old instance token index;
- v1 migration when the persistent index already exists;
- representative NFT data surviving migration;
- repeated migration rejection;
- unsupported stored versions;
- mismatched source-version arguments;
- missing counter and missing index partial states;
- unauthorized callers;
- absence of destination writes after preflight rejection.

The package validation command is:

```text
cargo +1.88.0 test -p commitment_nft
```

The repository contains an older feature-gated test file with a pre-existing
parse error, so the normal recursive formatter cannot be used safely. The
changed Rust file is checked with `rustfmt --config skip_children=true`, and
the focused package suite is run under Rust 1.88, the toolchain compatible with
the repository’s locked Soroban dependency graph.

## Future schema changes

Any future storage change should:

1. increment `CURRENT_VERSION`;
2. document every key added, removed, or moved;
3. add a source fixture representing the previous version;
4. preflight all values whose absence could change identity or balances;
5. preserve existing records unless an explicit transformation is specified;
6. write the new version marker last;
7. test authorization, malformed versions, repeated calls, and partial state.

A migration is complete only when both the data model and the operational
failure behavior are defined. A version integer by itself is not a storage
compatibility strategy.
