# Fee accounting and rounding policy

This document defines the fee arithmetic shared by commitment creation, early
exit, settlement, and marketplace payment flows. The policy is intentionally
small and deterministic so a reviewer can check the accounting identity at
each boundary.

## Invariant

Every successful fee operation starts with a positive gross amount and ends
with two non-negative whole-unit amounts:

```text
gross = fee + net
```

The fee is protocol revenue and the net amount is the amount sent to the
beneficiary. The equality is checked by `FeeSplit::conserves`; callers should
not reconstruct either side with an unchecked multiplication or subtraction.

## Rate representation

Protocol fees use basis points. The denominator is 10,000, so 100 bps means
one percent and 10,000 bps means the complete gross amount. Early-exit rules
use percentage points with a denominator of 100. Rates are inclusive at zero
and at their maximum, and any larger rate is rejected before state mutation.

The accepted ranges are:

| Flow | Rate type | Minimum | Maximum | Denominator |
| --- | --- | ---: | ---: | ---: |
| commitment creation | basis points | 0 | 10,000 | 10,000 |
| marketplace listing | basis points | 0 | 10,000 | 10,000 |
| marketplace offer | basis points | 0 | 10,000 | 10,000 |
| marketplace auction | basis points | 0 | 10,000 | 10,000 |
| early exit penalty | percent | 0 | 100 | 100 |

## Rounding

The whole-token fee uses floor division. The fractional numerator is retained
in the returned split and can be persisted by an adapter that wants to carry
dust across repeated operations. This is a deliberate choice:

- the user never pays more than the configured rate for one operation;
- the protocol never creates a fractional token unit;
- the beneficiary receives the exact remainder after the fee;
- repeated small operations can converge by using `split_*_with_carry`;
- callers can audit the fractional remainder without hidden global state.

Adapters that do not persist remainders still conserve every whole token unit.
They must document that sub-unit fractions are discarded at each operation.
Adapters that do persist them must store the denominator alongside the value
and reject a remainder from a different denominator.

## Overflow strategy

Naively evaluating `gross * rate / denominator` can overflow even when the
final result is representable. `split_with_denominator` instead evaluates:

```text
whole = gross / denominator
input_remainder = gross % denominator
fee = whole * rate + (input_remainder * rate) / denominator
```

The first product is bounded by the gross amount for valid rates. The second
product is bounded by the small denominator and rate. Every addition,
subtraction, and accumulation remains checked. A failed check returns
`FeeError::ArithmeticOverflow`; no caller should convert that into a wrapped
value or silently reduce the rate.

## Lifecycle requirements

Each caller follows the same order:

1. load and validate the configured rate;
2. calculate one `FeeSplit` before effects;
3. verify `split.conserves()`;
4. remove or lock the source balance as appropriate;
5. transfer `split.net` to the beneficiary;
6. transfer `split.fee` to the fee recipient when non-zero;
7. emit an event containing gross, fee, net, rate, and rounding policy.

The commitment contract applies the policy to creation and early exit. The
marketplace applies it to listing purchases, accepted offers, and auction
settlement. Keeping these paths on the same implementation prevents a future
fee change from fixing one lifecycle while leaving another vulnerable to an
overflow or a different rounding rule.

## Event and reconciliation fields

Operational events should expose enough information to reconcile token
transfers without replaying application logic. The recommended fields are:

- operation kind (`create`, `early_exit`, `listing`, `offer`, or `auction`);
- gross amount and asset address;
- configured rate and denominator;
- whole-unit fee and net amounts;
- fractional remainder, when carry is enabled;
- policy version and transaction timestamp.

The event is evidence, not authorization. Authorization and token ownership
checks remain in the surrounding contract. A reconciliation job should compare
the sum of fee and net transfers with gross, flag missing events, and treat an
overflow or invalid-rate error as a failed operation rather than an empty
settlement.

## Test matrix

The shared tests cover zero and maximum rates, one-unit amounts, rates that
produce dust, repeated ledger records, invalid amounts, invalid rates, invalid
remainders, and values close to `i128::MAX`. Contract-level tests should also
exercise each external call path with:

- a small amount whose fee floors to zero;
- a small amount whose fee leaves a remainder;
- a maximum valid rate;
- a rate one unit above the maximum;
- a repeated settlement attempt;
- a failed transfer after the split is calculated;
- multiple assets and independent fee recipients.

Property tests should assert `gross == fee + net`, `0 <= fee <= gross`, and
`0 <= remainder < denominator` for every successful calculation. They should
also assert that a rejected input does not alter the fee ledger. These checks
are more valuable than asserting only one representative percentage because
rounding and integer limits are the risk surface.

## Upgrade and compatibility notes

The denominator and rounding policy are part of the accounting schema. A
future change must version the policy and include the version in audit output.
Changing from floor to ceiling would alter beneficiary balances and cannot be
treated as a refactor. Existing stored remainders must never be interpreted
with a new denominator. A migration should either convert them explicitly or
zero them with an operator-approved reconciliation adjustment.

The current implementation preserves the existing public fee ranges and
storage keys. Its change is the arithmetic boundary and its tests. Consumers
that previously called a helper which panicked on overflow should migrate to a
`Result`-returning invariant helper before changing any state. This ensures a
bad configuration fails before transfers and is observable to clients.
