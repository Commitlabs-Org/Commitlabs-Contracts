# Marketplace emergency pause

The commitment marketplace now has an explicit emergency state. The state is
stored under `DataKey::Paused`, initialized to `false`, and exposed through
`is_paused`. It is deliberately local to the marketplace: pausing the
marketplace does not alter NFT ownership, token balances, listings, offers, or
auction records.

## Authorization and transitions

Only the configured marketplace administrator can call `pause` or `unpause`.
Both methods require the supplied caller to authenticate, then compare that
caller with the stored administrator. The transition table is:

| Current state | Operation | Result | Event |
| --- | --- | --- | --- |
| unpaused | admin `pause` | paused | `Pause(timestamp)` |
| paused | admin `pause` | `AlreadyPaused` error | none |
| paused | admin `unpause` | unpaused | `Unpause(timestamp)` |
| unpaused | admin `unpause` | `NotPaused` error | none |
| either | non-admin toggle | `Unauthorized` error | none |

Failed transitions do not write the pause key and do not emit an event. This
makes repeated operator actions safe to retry and keeps event order equivalent
to successful state transitions only.

## Paused operation policy

The following operations are blocked before their reentrancy guard, storage
writes, or token interactions:

- `update_fee`, `add_payment_token`, and `remove_payment_token`;
- `list_nft` and `buy_nft`;
- `make_offer` and `accept_offer`; and
- `start_auction` and `place_bid`.

Read-only methods continue to work so operators can inspect the state while the
marketplace is stopped. `cancel_listing`, `cancel_offer`, and `end_auction`
remain recovery paths: users can unwind an already-created commitment or end a
settleable auction without opening the marketplace to new exposure. The
recovery methods still authenticate the relevant participant and preserve the
existing checks-effects-interactions and reentrancy protections.

The pause check is intentionally performed before business validation. A
paused response therefore cannot be used to probe whether a listing, offer,
token, or auction exists, and no failed paused request leaves a reentrancy flag
behind.

## Invariants

The pause transition has no balance or ownership side effects. In particular:

1. `get_all_listings`, `get_offers`, and `get_all_auctions` return the same
   records before and after a successful pause.
2. A blocked new operation cannot create a listing, offer, or auction.
3. A recovery operation can remove only the caller’s own object and leaves the
   marketplace paused.
4. A successful unpause changes only the pause flag and allows normal
   operations to resume.
5. Each successful toggle emits exactly one event, and failed repeated toggles
   emit none.

Soroban invocation rollback additionally guarantees that a downstream token
   or NFT transfer failure rolls back the state writes made by that invocation.
   The pause gate does not weaken that atomicity: it only prevents the call from
   reaching the operation’s effects and interactions.

## Operator procedure

1. Call `pause(admin)` and verify `is_paused() == true`.
2. Inspect listings, offers, auctions, and any in-flight commitments using the
   read APIs.
3. Use only `cancel_listing`, `cancel_offer`, or `end_auction` to recover
   settleable state while paused.
4. Confirm the expected state and event history off-chain.
5. Call `unpause(admin)` only after the incident is understood and verify the
   flag and the next normal operation.

The administrator key is a high-impact control. It should be protected by the
deployment’s normal signer policy; this contract does not add a timelock or
multisignature wrapper around the existing administrator role.

## Remaining limitations

Pause state is independent for each deployed marketplace instance. A protocol
operator must pause every instance that shares custody or payment-token
liquidity. The contract also does not retroactively cancel listings or refund
bids; those actions remain explicit recovery transactions and retain their
normal participant authorization checks. Finally, pause events identify the
transition and ledger timestamp but do not contain an incident reason; incident
systems should attach that context off-chain using the transaction hash.
