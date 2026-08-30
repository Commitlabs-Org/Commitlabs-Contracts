# Oracle quorum and median policy

The price oracle stores a latest-price value for compatibility with existing
clients and, from this change onward, also stores the latest observation for
each authorized source. A reader that sees source metadata evaluates the source
set instead of trusting the last writer.

## Trust boundary

The contract administrator chooses the oracle addresses. Whitelisting is an
authorization decision, not evidence that two addresses are independent. The
operations team should use providers with separate signing keys, data paths,
and market-data dependencies where possible.

The contract enforces the following boundary:

| Actor | Allowed operation | Required condition |
| --- | --- | --- |
| Administrator | Add or remove a source | Authenticated admin |
| Administrator | Set an asset quorum | Authenticated admin and quorum > 0 |
| Whitelisted source | Publish an observation | Authenticated source |
| Any caller | Read a median | Enough fresh whitelisted observations |
| Any caller | Read the legacy latest value | Only when no source metadata exists |

Removing a source immediately removes its observations from the eligible read
set. The stored observation is retained so that a reviewed source can be
re-added without inventing a new price history. Re-adding a source does not
change the configured quorum.

## Observation lifecycle

Each successful `set_price` call performs these actions:

1. Authenticate the caller against the whitelist.
2. Reject negative prices and decimal precision above 18.
3. Stamp the observation with the current ledger timestamp.
4. Replace that source's observation for the asset.
5. Add the source to the asset's source list exactly once.
6. Preserve the legacy `Price(asset)` snapshot for older integrations.

The source list is an ordered Soroban vector only because persistent storage
needs a deterministic way to enumerate sources. Its order is not a price
weight and does not influence the median. A repeated source update replaces a
single observation; it cannot satisfy a quorum twice.

## Freshness rules

Readers calculate freshness against the current ledger timestamp. An
observation is eligible only when:

```text
updated_at <= now
now - updated_at <= max_staleness
```

Future timestamps are invalid. Stale observations are skipped rather than
silently used as a fallback. If the configured quorum cannot be met, the read
returns `InsufficientObservations`. A single-source compatibility read keeps
the established `StalePrice` error when its only observation is stale or
future-dated.

Consumers should select a window based on the asset's liquidity and update
cadence. A short window reduces the time an old market view can be used but
increases the chance of a liveness failure during an oracle outage. A longer
window improves availability while extending the stale-price risk.

The `get_price_valid` method applies the configured default unless a caller
provides an explicit override. The override is per read and does not alter the
on-chain policy. High-value consumers should use the smallest operationally
safe window and handle a failed read by pausing or using an independently
reviewed fallback.

## Quorum policy

Every asset defaults to quorum one to preserve existing deployments. Admins
can raise an asset's quorum with `set_quorum`. The value means the number of
distinct sources that must have an authorized, present, non-negative, fresh
observation. It is not the number of submissions or the number of entries in
the source vector.

Example configuration for three independent feeds:

```text
add_oracle(admin, source_a)
add_oracle(admin, source_b)
add_oracle(admin, source_c)
set_quorum(admin, btc, 2)
```

With that configuration, one source can be delayed or removed while the other
two keep the asset readable. A quorum of three requires all three sources and
therefore provides stronger agreement at the cost of lower availability.

An asset should not be configured with a quorum greater than the number of
sources that can independently publish it. Configuration changes should be
staged and monitored before a critical market is switched to a stricter
threshold.

## Decimal normalization

Source prices are non-negative integers paired with a decimal count. The
aggregator chooses the greatest decimal count among the eligible observations
and converts every value to that precision before sorting. Increasing precision
uses checked multiplication; reducing precision uses integer division.

For example:

```text
source A: 12      @ 0 decimals -> 1200 @ 2 decimals
source B: 1300    @ 2 decimals -> 1300 @ 2 decimals
median:   1250    @ 2 decimals
```

The accepted precision is capped at 18. Every power-of-ten factor is built with
checked multiplication, and every scaled price is checked before it is stored
in the signed 128-bit representation. A scaling overflow returns
`ArithmeticOverflow`; it never wraps into a low or negative price.

Rounding when reducing precision is deterministic truncation toward zero. The
contract currently accepts non-negative prices, so this is floor-like behavior
for all valid values. Providers should publish compatible precision when a
market needs sub-unit accuracy.

## Median calculation

The values are sorted with deterministic insertion sort after normalization.
For an odd number of observations, the center value is returned. For an even
number, the two center values are averaged using:

```text
lower + (upper - lower) / 2
```

This avoids adding two values near `i128::MAX`. The output timestamp is the
oldest accepted observation, so the aggregate cannot claim freshness beyond
the least-fresh input. The output decimal count is the normalization precision.

The median is resilient to a single extreme outlier when the quorum has at
least three sources. It is not a proof that the value is economically correct:
two colluding or correlated sources can still agree on a bad value. Operators
should combine the median with source independence, deviation alarms, and an
emergency pause or upgrade process.

## Failure behavior

The following conditions fail closed:

| Condition | Result |
| --- | --- |
| No latest value and no source metadata | `PriceNotFound` |
| Fewer eligible fresh sources than quorum | `InsufficientObservations` |
| One-source observation is stale or future-dated | `StalePrice` |
| Negative stored price | `InvalidPrice` |
| Decimal count above 18 | `InvalidDecimals` |
| Duplicate source in corrupted metadata | `DuplicateOracle` |
| Checked scaling or midpoint overflow | `ArithmeticOverflow` |
| Zero quorum configuration | `InvalidQuorum` |

Corrupt duplicate metadata is treated as an error instead of being silently
deduplicated. This gives operators a detectable signal that storage migration or
an upgrade has violated an invariant.

## Consumer guidance

Security-sensitive consumers should use `get_price_valid`,
`get_price_for_commitment`, or `get_price_for_marketplace`, which now benefit
from source aggregation whenever source metadata exists. They should not use
`get_price` for settlement decisions because that method intentionally remains
a non-fresh, compatibility snapshot.

Consumers should:

- handle `InsufficientObservations` as a liveness or pause condition;
- handle `StalePrice` as a freshness failure, not as a zero price;
- record the returned timestamp and decimal count with the business event;
- avoid treating a successful read as authorization to bypass local limits;
- use an explicit asset policy rather than one global staleness value when
  assets have different market hours or update frequencies.

Batch reads are atomic from the caller's perspective: if any requested asset
does not satisfy its freshness policy, the batch returns an error rather than a
partial collection. This prevents a caller from accidentally processing only
the assets that happened to have live feeds.

## Operational runbook

Before increasing quorum:

1. Confirm each source is independently operated.
2. Confirm each source can publish at the asset's required precision.
3. Publish test observations and inspect timestamps on the target network.
4. Read the median with the intended staleness override.
5. Verify monitoring can distinguish stale data from insufficient quorum.
6. Raise quorum during a controlled change window.

During an incident:

1. Check whether the failure is source freshness, source authorization, or
   decimal/overflow validation.
2. Do not lower quorum solely to make a settlement pass without recording the
   risk decision.
3. Rotate or remove a compromised source through the admin control.
4. Re-publish only after the source has been independently reviewed.
5. Reconcile downstream events using the returned timestamp and source policy.

The contract does not identify the source in the aggregate `PriceData` return
value because the value is a consensus result. Indexers that need attribution
should observe `PriceSet` events and retain the publisher from the transaction
authorization context.

## Compatibility and migration

Existing deployments can continue publishing through `set_price`; the method
now records source metadata in addition to the legacy snapshot. Existing
single-source deployments behave as before because quorum defaults to one.
Legacy storage containing only `Price(asset)` remains readable through
`get_price_valid` until a source observation is published for that asset.

Once source metadata exists, the reader no longer falls back to the legacy
snapshot when the source quorum fails. This is deliberate: fallback would make
the new quorum policy advisory and could reintroduce a stale or superseded
price during an outage.

## Verification matrix

The regression suite covers:

- odd and even source counts;
- equal values and submission-order independence;
- extreme outliers;
- mixed decimal precision and deterministic truncation;
- safe midpoint calculation near the signed integer limit;
- scaling overflow and unsupported precision;
- stale and future timestamps;
- insufficient quorum and zero-quorum configuration;
- admin-only policy changes;
- source replacement without duplicate counting;
- source removal and restoration of a failed quorum;
- legacy storage compatibility and unauthorized writes.

The suite uses the Soroban test environment and the generated contract client,
so each test exercises the same public authorization and storage boundaries as
an integration caller. Full workspace execution remains subject to the
repository's Rust toolchain and dependency compatibility.
