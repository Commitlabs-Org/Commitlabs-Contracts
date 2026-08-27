# Commitment identity policy

Commitment ids are namespaced by the contract instance and a non-zero
monotonic sequence. A local counter alone is not a cross-instance identity.
The identity policy keeps the instance bytes beside the sequence and returns a
stable collision error when a stored key does not match the requested record.

Retries carry a request fingerprint. An exact identity and fingerprint returns
the original identity without writing a second commitment. The same identity
with a changed owner, amount, duration, or asset is a typed fingerprint
conflict. A different instance with the same sequence is a distinct identity.

The registry adapter is fixed-capacity for deterministic contract tests. The
production Soroban adapter should persist the same fields under the existing
commitment key and use an atomic existence check before writing. A migration
may only move an existing record forward within its instance namespace; it may
not change the instance or move backwards.

## Required invariants

- zero instance and zero sequence are rejected;
- every stored identity is unique;
- retries never increment the commitment counter;
- changed request data never returns a successful replay;
- equal counters in different contract instances do not collide;
- migration preserves the request fingerprint;
- migration advances, rather than reuses, a sequence;
- malformed identifiers fail before storage writes;
- cross-contract lookups include the originating instance;
- the original identity remains the source of truth for downstream references.

## Compatibility and rollback

Existing `COMMIT_<sequence>` display strings can remain as a human-facing
alias, but cross-contract references must carry the instance namespace. The
policy is additive to existing stored records. Rollback retains the identity
fields and stops new migrations; deleting them would make old retries
ambiguous. The test matrix covers creation, exact replay, changed payload,
instance collision, capacity, migration, and malformed identity paths.

## Call-site guidance

At creation, read the instance namespace and sequence in one storage context.
Validate the generated identity before transferring assets or minting an NFT.
Write the commitment under the identity key before publishing a creation event.
If a write fails, do not advance the sequence counter.

At retry, reconstruct the request fingerprint from the exact original inputs.
Look up the namespace and sequence before attempting a second transfer. Return
the stored commitment identity when the fingerprint matches. Return a stable
conflict when the caller changes any financially relevant field.

At cross-contract lookup, pass the originating instance with the display id.
Never resolve a bare display string against whichever contract happens to be
calling. This prevents an NFT or attestation contract from binding to a
same-number commitment created by another instance.

At migration, preserve owner, amount, duration, asset, and fingerprint. Only
advance a sequence when the target key is unused. Emit the old and new identity
in the migration event and keep an audit link to the original record.

## Review matrix

| Scenario | Expected result |
| --- | --- |
| first request | new identity and one stored record |
| identical retry | original identity, no new transfer |
| changed amount | fingerprint conflict |
| changed owner | fingerprint conflict |
| changed asset | fingerprint conflict |
| same sequence, other instance | distinct identity |
| zero sequence | malformed-input error |
| zero instance | malformed-input error |
| target migration occupied | collision error |
| backwards migration | invalid-migration error |

The matrix is explicit so future call sites can add a case rather than
bypassing the shared resolver. It also provides a compatibility contract for
off-chain clients that cache commitment references.

## Operational monitoring

Track creation, replay, collision, fingerprint-conflict, and migration-failure
counts independently. A rise in collisions can indicate counter corruption or
an instance-namespace deployment error. A rise in fingerprint conflicts can
indicate provider retries with mutated payloads or an integration bug. Neither
condition should be silently converted into a new commitment.

Do not put owner addresses, full request payloads, or secret signing material
in metrics labels. The identity and short error code are sufficient for
diagnostics; detailed audit records belong in access-controlled storage.

## Migration checklist

- snapshot the current counter and namespace before deployment;
- verify every legacy record has a resolvable sequence;
- reserve target identities before moving any record;
- preserve fingerprints and downstream references;
- emit migration events only after the target write succeeds;
- run duplicate and cross-instance lookups in a staging environment;
- compare record counts before and after the migration;
- keep the old resolver available for display-only reads;
- monitor collision and conflict errors during the canary;
- stop the rollout if any identity maps to two records.

## Security note

An identifier is not authorization. Callers must still pass the existing owner,
admin, or authorized-contract checks after resolving an identity. The namespace
prevents ambiguity; it does not grant access to a commitment. Fingerprints are
used for retry equality only and are not a substitute for signature checks.

The policy intentionally uses fixed-size values and checked sequence movement.
This keeps behavior deterministic in Soroban WASM and avoids allocation-based
formatting in the contract boundary. Human-readable ids may be produced by an
off-chain indexer, but the on-chain identity remains the typed pair.

## Adapter contract

The policy is deliberately independent of storage so that every entry point
can use the same decision table. An adapter should perform the following work
in one transaction:

1. validate the instance namespace and the next sequence number;
2. derive the request fingerprint from the canonical request fields;
3. read the record addressed by the identity;
4. call `resolve_retry` before writing anything;
5. write the new record only for `ResolveResult::New`;
6. return the stored identity for both a new request and an exact replay.

The read and write must share the same authorization context. A caller must
not be able to probe another instance's records by supplying an arbitrary
namespace. The namespace is normally derived from the contract address, or
from a deployment registry controlled by the contract administrator.

## Canonical request fields

Fingerprints are meaningful only when the input encoding is canonical. The
adapter must use one representation for each field and document it:

- addresses use their fixed 32-byte contract representation;
- amounts use unsigned little-endian bytes of the contract integer type;
- durations use the bounded integer used by the public interface;
- optional fields use an explicit presence marker;
- lists are ordered and are not deduplicated implicitly;
- versioned requests include the schema version in the digest input.

Changing any of these rules is a migration, even when the public method name
stays the same. During a rolling upgrade, accept the old version only for
records already present and write new records with the new version. Never
interpret a failed fingerprint comparison as permission to create a second
record under the same identity.

## Failure handling

`Collision` is a safety stop. The adapter should surface it, record the
identity and deployment version in an audit event, and leave both records
unchanged. `FingerprintConflict` is a client-visible idempotency error; the
client can recover by retrieving the original result using its request key.
`Capacity` indicates an operational limit and must not be retried in a tight
loop. `InvalidMigration` requires an administrator-reviewed migration plan.

All failures happen before the commitment payload is mutated. This ordering
is important for Soroban callers because a host-level retry may repeat the
whole invocation after a timeout. A timeout alone is not evidence that the
first invocation failed. The caller should retry the same canonical request,
and the resolver should return `Replay` when the first write was committed.

## Compatibility and rollout

Existing display-only APIs may continue to expose a legacy short identifier,
but they should also expose the instance namespace and sequence separately.
Indexers should store the pair as a composite key. A migration tool should
reject duplicate legacy ids instead of choosing the first record encountered.
This makes ambiguous historical data visible for manual repair.

Before enabling writes, deployers should run a dry-run over historical records,
verify that every record receives one namespace, and compare the number of
unique composite keys with the number of records. The canary should exercise
two contract instances with identical local sequences, repeated submissions,
altered payloads, and a sequence near the integer limit.

After rollout, retain the old read path for a bounded observation period. It
must be read-only and must never become a fallback for a failed new write.
Remove it only after downstream indexers and client SDKs have switched to the
composite identity. The release notes should include the policy version,
namespace derivation rule, fingerprint schema, and the rollback limitation:
already-issued identities cannot be safely reassigned to another instance.
