# Protocol Fee Structure

This document describes the fee collection mechanism for protocol revenue across CommitLabs contracts.

## Fee Types

| Fee Type | Contract | Description | Rate / Amount |
|----------|----------|-------------|----------------|
| **Commitment creation fee** | commitment_core | Charged when a user creates a new commitment | Basis points (0–10000) of commitment amount |
| **Attestation verification fee** | attestation_engine | Charged when a verifier records an attestation | Fixed amount per attestation (token + amount) |
| **Commitment transformation fee** | commitment_transformation | Charged when creating tranches from a commitment | Basis points (0–10000) of total value |
| **Early exit fee** | commitment_core | Penalty on early exit; goes to protocol | Percentage from commitment rules (stored as protocol revenue) |
| **Marketplace fees** | — | If applicable in future | TBD |

## Basis Points

All percentage-based fees use **basis points (bps)**:

- `10000 bps = 100%`
- `100 bps = 1%`
- `50 bps = 0.5%`

Fee calculation: `fee_amount = (amount * fee_bps) / 10000` (integer division, rounds down).

Shared logic lives in `shared_utils::fees`: `fee_from_bps(amount, bps)` and `BPS_SCALE = 10000`.

## Fee Collection

### commitment_core

- **Creation fee**: On `create_commitment`, if `creation_fee_bps > 0`, a fee is computed from the user’s amount. The user transfers the full amount to the contract; the fee is credited to `CollectedFees(asset)` and the commitment is created with `amount_locked = amount - creation_fee`.
- **Early exit fee**: On `early_exit`, the penalty (from commitment rules) is retained by the contract and added to `CollectedFees(asset)`; the rest is returned to the owner.

### attestation_engine

- **Attestation fee**: If `AttestationFeeAmount` and `AttestationFeeAsset` are set (by admin), each `attest` call transfers that amount of the given token from the caller (verifier) to the contract and adds it to `CollectedFees(asset)`.

### commitment_transformation

- **Transformation fee**: On `create_tranches`, if `TransformationFeeBps > 0`, the caller must send `fee_amount = (total_value * fee_bps) / 10000` of `fee_asset` to the contract. That amount is added to `CollectedFees(fee_asset)`.

## Fee Recipient and Withdrawal

- Each contract that collects fees can have a **fee recipient** (treasury) set by admin.
- **Admin-only** functions:
  - **commitment_core**: `set_fee_recipient(recipient)`, `withdraw_fees(asset_address, amount)`
  - **attestation_engine**: `set_fee_recipient(recipient)`, `withdraw_fees(asset_address, amount)`
  - **commitment_transformation**: `set_fee_recipient(recipient)`, `withdraw_fees(asset_address, amount)`
- Withdrawal sends tokens from the contract to the configured fee recipient. Withdrawable amount is capped by `CollectedFees(asset)` for that asset.
- Fee recipient must be set before `withdraw_fees` can succeed.

## Access Control

- **Admin** sets fee rates and fee recipient:
  - commitment_core: `set_creation_fee_bps(bps)`, `set_fee_recipient(recipient)`
  - attestation_engine: `set_attestation_fee(amount, asset)`, `set_fee_recipient(recipient)`
  - commitment_transformation: `set_transformation_fee(bps)` (already existed), `set_fee_recipient(recipient)`
- Fees are collected automatically on the relevant actions (create, attest, create_tranches, early_exit).
- Only admin can call `withdraw_fees`.

## Storage Summary

- **commitment_core**: `FeeRecipient`, `CreationFeeBps`, `CollectedFees(Address)` (per asset).
- **attestation_engine**: `FeeRecipient`, `AttestationFeeAmount`, `AttestationFeeAsset`, `CollectedFees(Address)`.
- **commitment_transformation**: `FeeRecipient`, `CollectedFees(Address)`; transformation fee rate is `TransformationFeeBps`.

## Getters

- **commitment_core**: `get_creation_fee_bps()`, `get_fee_recipient()`, `get_collected_fees(asset)`.
- **attestation_engine**: `get_attestation_fee()` → `(amount, Option<asset>)`, `get_fee_recipient()`, `get_collected_fees(asset)`.
- **commitment_transformation**: `get_transformation_fee_bps()`, `get_fee_recipient()`, `get_collected_fees(asset)`.

## Fee Tiers (Future)

The design allows for fee tiers (e.g. different bps by commitment size or type) to be added later by extending storage and fee calculation in each contract.
