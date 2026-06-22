# commitment_core created_between bucket index

## Scope

`commitment_core::get_commitments_created_between(from_ts, to_ts)` previously loaded
`DataKey::AllCommitmentIds` and read every commitment before filtering by `created_at`.

This change adds a UTC-day creation index:

- `DataKey::CommitmentCreatedBucketDays`
- `DataKey::CommitmentsCreatedInBucket(day)`

`create_commitment` appends each new commitment ID to its creation-day bucket, and
`get_commitments_created_between` reads only bucket IDs for non-empty days whose bucket
falls inside the requested timestamp range.

## Cost shape

Before:

- Storage reads: `AllCommitmentIds` plus one commitment read for every commitment ever created.
- Query cost: `O(total_commitments)`.

After:

- Storage reads: `CommitmentCreatedBucketDays`, relevant bucket vectors, and commitment records
  inside those relevant buckets.
- Query cost: `O(non_empty_bucket_days + commitments_in_matching_buckets)`.

The new tests cover empty ranges, reversed ranges, single-bucket boundaries, multi-bucket
queries, and equivalence with the old full-scan filter order.
