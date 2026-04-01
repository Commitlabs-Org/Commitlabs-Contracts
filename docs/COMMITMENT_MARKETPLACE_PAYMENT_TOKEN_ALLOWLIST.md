# Commitment Marketplace Payment Token Allowlist

`commitment_marketplace` rejects arbitrary `Address` values as payment tokens.

## Admin-managed configuration

Admins must explicitly manage the allowlist with:

- `add_payment_token(payment_token)`
- `remove_payment_token(payment_token)`
- `is_payment_token_allowed(payment_token) -> bool`
- `get_allowed_payment_tokens() -> Vec<Address>`

The contract enforces `require_auth` on the stored admin address before any
allowlist mutation.

## Marketplace behavior

The marketplace now rejects non-allowlisted token addresses with
`MarketplaceError::PaymentTokenNotAllowed` in:

- `list_nft`
- `make_offer`
- `start_auction`

The contract also re-checks the allowlist before token settlement in:

- `buy_nft`
- `accept_offer`
- `place_bid`
- `end_auction` when there is a winning bidder

## Operational note

Removing a token blocks both new marketplace activity and settlement of any
existing listings, offers, or auctions that still reference that token until
the admin re-allowlists it. This is intentional so administrators can stop
future outbound calls to an untrusted or deprecated token contract.
