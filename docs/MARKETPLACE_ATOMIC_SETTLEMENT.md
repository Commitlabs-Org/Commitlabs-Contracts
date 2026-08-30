# Marketplace atomic settlement

Fixed-price marketplace settlement is a single business operation even though
it contains several external token calls. A purchase must either distribute
the complete sale amount or leave the listing and all accounting configuration
available for retry.

## Preconditions

`buy_nft` performs all checks before the first payment transfer:

1. The marketplace is initialized and not paused.
2. The buyer has authenticated the call.
3. The listing exists and the buyer is not the seller.
4. The listing payment token is currently allowlisted.
5. Fee and royalty configuration is within policy limits.
6. Checked payout arithmetic produces a non-negative seller amount.
7. The buyer balance covers the full sale price.

The buyer-balance preflight is intentionally based on the gross sale price,
not the seller proceeds. Fees and royalties are recipients of the same debit;
checking only the seller amount would allow an underfunded purchase to reach a
partial payout attempt.

## Commit protocol

The function uses a guarded check/interact/commit sequence:

```text
check listing, identities, asset, fees, balances
  │
  ├─ transfer buyer → seller
  ├─ transfer buyer → marketplace fee recipient
  └─ transfer buyer → royalty recipient (when configured)
       │
       └─ consume listing + royalty + active-list index
          clear reentrancy guard
          emit NFTSold
```

The listing is intentionally consumed after every transfer succeeds. The
reentrancy guard remains active during the external calls, so a callback cannot
buy or mutate the same listing while settlement is in flight. Soroban
transaction semantics additionally roll back the complete invocation if an
external contract panics; placing the commit after interactions makes the
ordering explicit for adapters that return failures as well.

The final event is emitted only after the listing is absent. Consumers can
therefore treat `NFTSold` as a committed settlement signal, not as an intent
or a transfer-start notification.

## Asset and recipient policy

Payment contracts must be added by the marketplace administrator. Removing an
asset blocks new purchases, including existing listings that reference the
removed asset, until an operator reviews and re-allowlists it. This prevents a
listing from silently switching to an unreviewed token contract.

The buyer/seller identity check is performed from the authenticated buyer and
the immutable listing seller. Fee and royalty basis points use checked,
conservation-preserving arithmetic. A sale is rejected if the deductions
cannot be represented safely or exceed the gross amount. Royalty state is
deleted only with the successfully consumed listing and remains available on
any failed attempt.

The contract does not fabricate an NFT transfer: the current marketplace
interface stores the configured NFT contract but deliberately leaves the
cross-contract NFT transfer integration to the deployed NFT integration layer.
That limitation is retained to avoid changing the public interface or adding
an incompatible transfer authority model in this issue.

## Failure matrix

| Failure point | Expected result |
| --- | --- |
| missing listing | `ListingNotFound`; no storage mutation |
| buyer is seller | `CannotBuyOwnListing`; listing remains |
| removed/unallowlisted token | `PaymentTokenNotAllowed`; listing remains |
| invalid fee/royalty math | policy error; no transfer or storage mutation |
| buyer balance below gross price | `InsufficientPayment`; buyer and listing unchanged |
| seller transfer failure | invocation fails; no consumed listing |
| fee transfer failure | invocation fails; no consumed listing |
| royalty transfer failure | invocation fails; no consumed listing |
| successful all-recipient transfers | listing consumed exactly once and event emitted |
| second purchase after success | `ListingNotFound`; no second debit |

## Test evidence

The dedicated regression module uses real Soroban Stellar asset contracts and
covers underfunded buyers, zero balances, removed assets, allowlist rejection,
royalty preservation, gross-balance conservation, fee/royalty distribution,
successful one-time consumption, duplicate settlement, and post-commit event
ordering. Existing marketplace tests continue to cover offers, auctions,
pause behavior, reentrancy, and fee arithmetic.

## Trade-offs and limitations

Preflight balance checks add one read-only token call, but provide a stable
error and avoid an avoidable external transfer attempt. The configured
allowlist is administrator-controlled; deployments must define an operational
review process for adding assets and must monitor removals because they can
pause existing listings. The contract-level all-or-nothing behavior protects
marketplace state, while downstream indexers should still process the final
`NFTSold` event idempotently by token id and transaction hash.

## Authorization matrix

| Operation | Required signer | Resource checked |
| --- | --- | --- |
| create listing | seller | seller address in the call |
| configure royalty | seller | active listing seller |
| add/remove payment asset | administrator | initialized admin |
| purchase | buyer | buyer address in the call |
| update fee | administrator | initialized admin |

The contract never accepts a seller or buyer identity from a separate
untrusted field. Soroban `require_auth` binds the address argument to a signed
authorization entry. A buyer cannot purchase its own listing, and a seller
cannot use a listing configuration belonging to another token to alter a
completed sale.

## Accounting proof

For a gross sale amount `G`, marketplace fee `F`, and royalty `R`, the checked
split requires:

```text
0 <= F
0 <= R
F + R <= G
seller_proceeds = G - F - R
buyer_debit = G
```

The marketplace and royalty percentages are calculated independently using
the shared basis-point policy and then combined with checked addition and
subtraction. Integer division floors each recipient's fee; the seller receives
the remaining whole units, so no token unit is silently created or lost.

The preflight debit is the gross amount, even when the marketplace fee is
100% or a royalty is configured. A successful settlement therefore satisfies:

```text
buyer_before - buyer_after == seller_received + fee_received + royalty_received
```

The dedicated tests exercise the normal split, zero royalty, non-zero royalty,
rounding, and full-fee boundary. The existing shared fee-invariant tests cover
large values and arithmetic overflow behavior before the marketplace reaches
the transfer section.

## Why commit after interactions

Removing a listing before calling the token contract is a common
checks-effects-interactions pattern, but it makes the business commit visually
precede the actual settlement. It also makes the safety argument dependent on
every downstream token implementation panicking on failure. This contract
keeps the reentrancy guard as the first state change, performs all read-only
preconditions, calls the payment token while the guard is active, and consumes
the listing only after all recipient transfers return.

There is no unguarded callback window: a reentrant call sees the active guard
and returns `ReentrancyDetected`. If a token transfer fails, the commit block
is never reached. If a host-level failure aborts the invocation, Soroban rolls
back the invocation's storage and token effects together. Both protections
are tested as separate invariants: no successful sale event and no consumed
listing on failure.

## Operator checklist

Before enabling a payment asset:

- verify the token contract implements the Soroban token interface;
- verify the asset's decimals and transfer behavior on the target network;
- add the token from the initialized administrator account;
- perform a small test listing and inspect the `ListNFT` event;
- confirm the buyer gross debit and each recipient credit;
- retain the transaction hash for indexer reconciliation.

When an asset is removed, existing listings are intentionally not silently
settled. The operator should review each affected listing, decide whether the
asset can be restored, and communicate that settlement is paused. Re-adding
the token restores the existing reviewed token identity; it does not rewrite a
listing's payment-token field.

## Recovery and monitoring

Indexers should key a settlement record by `(marketplace, token_id,
transaction_hash)` and treat duplicate `NFTSold` observations as idempotent.
An alert should fire when a listing remains active after an attempted purchase
or when token debits do not reconcile with the sale event. Operators should
inspect the original invocation and token contract before retrying; they
should not create a replacement listing automatically.

The marketplace contract does not maintain an off-chain retry queue. A failed
invocation leaves the original listing available, which is the recoverable
state. The buyer can retry after funding the account or after the operator
restores a reviewed payment token. A successful invocation removes the listing
so a second attempt fails closed with `ListingNotFound`.

## Verification scope

This issue changes the fixed-price purchase path and its preconditions. Offers
and auctions use their own settlement routines and remain covered by their
existing tests. The marketplace currently stores the NFT contract address but
does not invent a cross-contract NFT transfer authority; integrating that
transfer requires an explicit escrow/approval design and is outside this
atomic payment-state change. The payment, fee, royalty, listing, allowlist,
reentrancy, and event-order invariants described here are enforced now.
