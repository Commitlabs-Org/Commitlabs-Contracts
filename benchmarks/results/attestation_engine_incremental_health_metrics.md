# attestation_engine incremental health metrics cache

## Scope

`get_health_metrics` and `calculate_compliance_score` previously depended on full
`Attestations(commitment_id)` history recomputation whenever no usable cache existed.
`update_health_metrics` also reloaded the full attestation vector after each write.

This change keeps the existing `HealthMetrics(commitment_id)` cache, marks entries
written by the incremental updater, and updates fee totals, latest drawdown,
volatility exposure, last attestation time, and compliance score as each attestation
is written.

## Cost shape

Before:

- Write path: append attestation, then reload and reduce the full commitment history.
- Cached read path: `calculate_compliance_score` could return a stored score, but
  `get_health_metrics` recomputed aggregate fields from full history.
- Complexity: `O(attestations_for_commitment)` on affected writes and cold aggregate reads.

After:

- Write path: append attestation and update cached aggregate fields from the new
  attestation only.
- Cached read path: `get_health_metrics` reads the updater-marked cache for aggregate
  fields while still cross-reading `commitment_core` for canonical current/initial values.
- Legacy/manual cache path: unmarked caches fall back to the previous full recomputation.
- Complexity: `O(1)` for normal cached writes and aggregate reads.

The regression test `test_incremental_health_metrics_match_full_recomputation`
compares cached fields against a full history reduction for mixed fee and drawdown
attestations.
