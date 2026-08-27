# Attestation metric consistency

This document describes the accounting guarantees implemented for issue #549.
The attestation engine stores the individual records and exposes health metrics
derived from those records. The two representations must agree after every
successful transaction and must remain unchanged after every rejected record.

## Invariants

The following invariants are part of the contract's storage interface:

1. `fees_generated` is the sum of accepted `fee_generation` records for the
   commitment. Fee values are non-negative integers.
2. `drawdown_percent` is a percentage in the inclusive range `0..=100`.
3. `compliance_score` is a score in the inclusive range `0..=100`.
4. `total_attestations` counts accepted records only.
5. `total_violations` counts accepted violations and accepted non-compliant
   records only.
6. A verifier's count contains accepted records only.
7. The per-commitment attestation counter equals the number of records in the
   commitment's history.
8. A rejected transaction does not leave a partial history, metric, or counter
   update behind.

These rules apply to both the general `attest` entrypoint and the convenience
entrypoints `record_fees` and `record_drawdown`. Keeping validation in the
shared writer prevents a convenience path from accidentally bypassing a bound
that applies to the general path.

## Validation order

An accepted record follows this order:

1. Authenticate the caller when the selected entrypoint requires it.
2. Validate the commitment identifier.
3. Resolve the commitment from the configured commitment core contract.
4. Validate the attestation type and payload.
5. Validate metric-specific numeric bounds.
6. Append the record to the commitment history.
7. Update the cached commitment metrics.
8. Update global and verifier counters.
9. Emit the corresponding event.

Metric-specific validation happens before the history append. This is
important because malformed or out-of-range data must not be stored merely to
be ignored by a later aggregation pass.

## Fee accounting

Fee-generation records carry a `fee_amount` string. The value must parse as an
`i128` and must be greater than or equal to zero. A malformed amount returns
`InvalidAttestationData`; a negative amount returns `InvalidFeeAmount`.

The per-commitment fee total and the protocol-wide fee total use checked
addition. If either addition would overflow, the transaction returns
`StorageError`. Soroban transaction rollback then restores the history,
metrics, counters, and pre-existing total to their values from before the
attempt.

The global total is updated only for a fee-generation attestation. Generic
attestations that merely contain an unrelated field named `fee_amount` do not
change the protocol-wide fee total unless their type is `fee_generation`.

## Drawdown accounting

Drawdown records carry a `drawdown_percent` string and accept only values from
zero through one hundred, inclusive. The boundary values are valid because a
zero drawdown is a meaningful healthy observation and one hundred represents a
complete drawdown. Negative values and values above one hundred are rejected.

The cached drawdown metric is updated from accepted records. Read-time
aggregation also clamps the resulting percentage to the documented range, so
legacy records cannot cause an out-of-contract value to be exposed.

## Compliance score

The compliance score is stored as an unsigned integer and capped at one
hundred. A compliant non-violation record can increase the score, while
violation and non-compliant records lower it according to the existing scoring
rules. The cap is applied at the point of increment and the read-time score
calculation also preserves the same upper bound.

This makes repeated successful observations safe: a long history cannot wrap
the score or expose a value higher than the documented maximum.

## Counters and atomicity

The contract maintains counters in three places: per commitment, globally, and
per verifier. Each counter uses checked addition. The batch path performs the
same checks before persisting its in-memory aggregate counters.

All writes belong to the same contract invocation. If a checked operation
returns an error, the invocation fails and Soroban reverts writes from the
failed transaction. Callers can therefore retry a rejected record after fixing
the input without first repairing partially updated metrics.

The consistency guarantee is intentionally transaction-scoped. A successful
transaction may update several related storage keys, but it never publishes a
new history length without the corresponding cached metrics and counters.

## Read behavior

`get_stored_health_metrics` returns the cached value and does not recalculate
or mutate storage. It returns `None` until the first record is accepted.

`get_health_metrics` performs read-time aggregation against the canonical
commitment record and stored history. Calling it repeatedly is observational:
the result and stored attestation count remain unchanged. This prevents
monitoring dashboards or compliance checks from changing the accounting they
are observing.

## Test matrix

The metric consistency test module covers the following cases:

| Case | Expected result |
| --- | --- |
| Empty history | Bounded zero baseline values |
| One accepted fee | Fee total and count increase once |
| One accepted drawdown | Boundary-safe drawdown is preserved |
| Mixed history | Score, drawdown, and count stay bounded |
| Repeated reads | Values and counters remain unchanged |
| Equivalent fee orderings | Same accepted total and count |
| Negative fee | `InvalidFeeAmount`, no history mutation |
| Invalid drawdown | `InvalidAttestationData`, no history mutation |
| Generic negative fee field | Rejected before append |
| Maximum drawdown | `100` is accepted |
| Fee accumulator overflow | `StorageError`, atomic rollback |

The tests use a deterministic mock commitment-core contract. This keeps the
tests focused on the attestation engine's storage and aggregation behavior
while still exercising the production cross-contract lookup.

## Operational guidance

Clients should treat `InvalidAttestationData` and `InvalidFeeAmount` as input
errors and should not retry the same payload unchanged. A `StorageError` is a
contract-level failure; callers may retry after checking the account and
transaction context, but should not assume that a partial record was written.

Indexers should use accepted-record events and counters as transaction-level
signals. They should not infer acceptance from a submitted transaction alone,
because a rejected invocation emits no successful record event and rolls back
its storage changes.

For upgrades, existing histories remain readable. New writes are subject to
the bounds and checked arithmetic described above, while read-time aggregation
keeps exposed percentages and scores within their public ranges.

## Verification

Run the focused metric tests with:

```text
cargo +1.88.0 test -p attestation_engine metric_consistency_tests
```

Run the complete Rust library test suite with:

```text
cargo +1.88.0 test -p attestation_engine --lib
```

The library suite includes the existing attestation tests and the consistency
tests in this document. Repository documentation examples are maintained
separately and are not required to execute contract code in the library test
run.
