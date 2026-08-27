# Marketplace royalty accounting

This document describes the settlement rules implemented for issue #555. The
marketplace applies the same accounting policy to fixed-price purchases, offer
acceptance, and auction settlement.

## Configuration

Royalty configuration is stored independently from the listing to avoid
changing the serialized shape of the existing `Listing` type. A listing seller
authorizes both the recipient address and the royalty rate with
`set_royalty(seller, token_id, recipient, basis_points)`.

The rate is expressed in basis points, where 10,000 basis points equals 100%.
The contract policy caps royalties at 1,000 basis points (10%). The cap is
enforced when configuration is written and is checked again while calculating
payouts. The second check protects settlement if an older or manually migrated
storage value is invalid.

Only the current listing seller can update a royalty. A caller cannot replace a
seller's recipient, lower the seller's proceeds, or configure a royalty for a
missing listing. A zero rate is valid and is treated as an explicit no-royalty
configuration, which is useful for integrations that want deterministic
configuration reads.

## Settlement formula

All percentages use integer floor division. For a sale amount `A`, marketplace
fee rate `F`, and royalty rate `R`, the payout calculator produces:

```text
fee       = floor(A * F / 10,000)
royalty   = floor(A * R / 10,000)
seller    = A - fee - royalty
```

The fee rate must be no greater than 10,000 basis points. The royalty cap and
the fee cap together guarantee that the normal configuration cannot create a
negative seller payout. Checked multiplication and subtraction still protect
the contract from integer overflow and corrupted storage. A settlement is
rejected when the arithmetic cannot be represented safely or when deductions
would exceed the sale amount.

The floor policy leaves any fractional basis-point remainder with the seller.
Consequently, the conservation invariant is exact for every integer sale:

```text
seller + fee + royalty == sale amount
```

This is also true for very small sales where one or both deductions round down
to zero. No payout is sent for a zero amount, so the token contract never sees
unnecessary zero-value transfers.

## Transfer ordering and atomicity

The payout tuple is computed before marketplace state is changed. Once the
checks pass, the listing or offer state is marked consumed before external
token calls, preventing reentrant duplicate settlement. Soroban transaction
rollback restores both state and token balances if a payment transfer fails.

Fixed-price settlement sends seller proceeds, marketplace fee, and royalty
from the buyer's payment to their respective recipients. Offer settlement uses
the offer amount and the same calculator. Auction settlement uses the winning
bid and also removes the royalty configuration after a successful or no-bid
end. Cancelling a listing removes its pending royalty configuration without
performing a payout.

## Compatibility and resale

Existing listings without a `Royalty` storage entry use a zero royalty. This
preserves the previous fee-only behavior and avoids requiring a migration for
old listings. A royalty entry is consumed on settlement and cannot be reused
for a duplicate purchase. A later resale creates a new listing and must receive
its own seller-authorized royalty configuration.

The public listing and auction data structures remain unchanged. The new
configuration is an additional storage key and the new errors are appended to
the existing error numbering, preserving compatibility for existing clients.

## Required test coverage

The contract tests cover:

1. zero, maximum, and over-limit royalty percentages;
2. authorized and unauthorized royalty updates;
3. primary sale fee/royalty splits and exact conservation;
4. integer rounding on small sale amounts;
5. invalid marketplace fee configuration;
6. failed payment rollback for listing and royalty state;
7. duplicate settlement and royalty consumption;
8. cancellation cleanup of unsettled royalty state; and
9. compatibility of listings with no royalty configuration.

These tests intentionally use a real Soroban asset contract for payout cases,
so recipient balances verify the accounting at the token boundary rather than
only checking an internal helper.
