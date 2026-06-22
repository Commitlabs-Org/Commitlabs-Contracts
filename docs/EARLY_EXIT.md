# Early Exit Penalty

`commitment_core::early_exit` lets the commitment owner close an active commitment before maturity. The contract returns the post-penalty value to the owner, marks the commitment as `early_exit`, and marks the linked NFT inactive.

## Pro-Rata Penalty

The configured `rules.early_exit_penalty` remains a whole-number percent from `0` to `100`, but the charged amount now decays linearly as the commitment approaches expiry.

```text
max_penalty = current_value * early_exit_penalty / 100
elapsed = min(now - created_at, expires_at - created_at)
remaining = (expires_at - created_at) - elapsed
penalty = ceil(max_penalty * remaining / (expires_at - created_at))
returned = current_value - penalty
```

At creation time the penalty equals the configured maximum. At or after expiry the early-exit penalty is zero. The charged penalty is always bounded to `0..=max_penalty`.

Ceiling division is intentional: fractional penalties round up by one stroop so rounding cannot reduce protocol solvency. Checked arithmetic is used for the scaled numerator and TVL/fee ledger updates, so malformed or extreme stored values fail instead of wrapping.

## Fee Routing

Early-exit penalties are protocol revenue. The penalty is credited to `CollectedFees(asset)` and can be withdrawn to the configured `FeeRecipient` through `withdraw_fees`, matching the rest of the `commitment_core` fee model.

## Events

`early_exit` keeps the legacy `EarlyExt` event for compatibility:

```text
topics: (EarlyExt, commitment_id, caller)
data:   (penalty, returned, timestamp)
```

It also emits `early_exit_settled` with the stable accounting payload:

```text
topics: (early_exit_settled)
data:   (amount, penalty, elapsed_ratio_bps)
```

`elapsed_ratio_bps` is `0` at creation, `10_000` at or after expiry, and otherwise floors `elapsed * 10_000 / duration`.
