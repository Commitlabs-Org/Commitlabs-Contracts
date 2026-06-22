# Protocol Fee Model

End-to-end reference for protocol revenue across all fee-collecting contracts. Every fact below is verified against the current `lib.rs` of each crate.

## Summary Table

| Fee | Contract | Collection entrypoint | Rate key | Rate validation | Recipient key | Accumulation | Withdrawal |
|-----|----------|----------------------|----------|-----------------|---------------|--------------|------------|
| Commitment creation | `commitment_core` | `create_commitment` | `CreationFeeBps` | 0–10000 bps (`set_creation_fee_bps`) | `FeeRecipient` | `CollectedFees(asset)` | `withdraw_fees` (Treasurer/Admin) |
| Early exit penalty | `commitment_core` | `early_exit` | `rules.early_exit_penalty` (per-commitment, percent 0–100) | Validated at creation by commitment type | `FeeRecipient` | `CollectedFees(asset)` | `withdraw_fees` (Treasurer/Admin) |
| Attestation verification | `attestation_engine` | `write_attestation` (via `record_fees`, `record_drawdown`, `_attest_internal`) | `AttestationFeeAmount` + `AttestationFeeAsset` | `amount >= 0` (`set_attestation_fee`); `0` disables | `FeeRecipient` | `CollectedFees(asset)` | `withdraw_fees` (Admin) |
| Transformation | `commitment_transformation` | `create_tranches` | `TransformationFeeBps` | 0–10000 bps (`set_transformation_fee`) | `FeeRecipient` | `CollectedFees(fee_asset)` | `withdraw_fees` (Admin) |
| Marketplace sale | `commitment_marketplace` | `buy_nft`, `accept_offer`, `end_auction` | `MarketplaceFee` | **No validation on set**; `end_auction` caps at 10000 bps at settlement | `FeeRecipient` | Direct transfer (no `CollectedFees`) | N/A — paid to recipient at sale time |

> **Not protocol revenue:** `attestation_engine::record_fees` records manager `fee_generation` attestation data and updates the `TotalFees` analytics counter. It does **not** define the protocol verification fee; verification fees are collected inside `write_attestation` when `AttestationFeeAmount > 0`.

> **Batch gap:** `batch_attest` persists attestations inline and does **not** call `write_attestation`, so it does **not** collect attestation verification fees.

---

## Basis Points and Rounding

### Basis points (bps)

Percentage-based protocol fees use **basis points**:

- `10_000 bps = 100%`
- `100 bps = 1%`
- `50 bps = 0.5%`

Shared helper: `shared_utils::fees::fee_from_bps(amount, bps)` with `BPS_SCALE = BPS_MAX = 10_000`.

```rust
fee_amount = (amount * bps) / 10_000   // integer division, rounds toward zero (floor for positive values)
```

Used by: `commitment_core` (creation fee), `commitment_transformation` (transformation fee), `commitment_marketplace` (sale fee via `SafeMath::div(SafeMath::mul(price, bps), 10_000)` or equivalent).

### Early exit penalty (percent, not bps)

Early exit uses **whole-number percent** (0–100), not basis points:

```rust
penalty = SafeMath::penalty_amount(current_value, early_exit_penalty)
        = (current_value * early_exit_penalty) / 100
```

Integer division truncates toward zero. Small `current_value` values can yield a zero penalty (documented in `commitment_core::early_exit` rustdoc).

### Dust and tranche rounding (`commitment_transformation`)

Per the transformation contract module docs, both fee and tranche splits use the same truncation rule:

```rust
fee           = (total_value * fee_bps) / 10_000
tranche_amount = (net_value * share_bps) / 10_000
```

Both round toward zero (floor for positive values). The sum of tranche amounts can be up to `n − 1` stroops less than `net_value` where `n` is the number of tranches; residual dust may remain in the contract.

---

## Per-Contract Detail

### `commitment_core`

#### Fees collected

| Fee | When | Calculation | Token flow |
|-----|------|-------------|------------|
| Creation | `create_commitment` | `fuzzing::checked_fee_from_bps(amount, CreationFeeBps)`; default bps `0`; overflow maps to `ArithmeticOverflow` with guard reset | Owner transfers full `amount` to contract; `creation_fee` credited to `CollectedFees(asset_address)`; NFT minted with `net_amount = amount - creation_fee`; TVL incremented by `net_amount` |
| Early exit | `early_exit` | `SafeMath::penalty_amount(current_value, rules.early_exit_penalty)` | Penalty added to `CollectedFees(asset)`; `returned = current_value - penalty` transferred to owner when `returned > 0` |

#### Storage keys

| Key | Type | Set by | Default |
|-----|------|--------|---------|
| `CreationFeeBps` | `u32` | `set_creation_fee_bps` | `0` (via `get_creation_fee_bps`) |
| `FeeRecipient` | `Address` | `set_fee_recipient` | unset (`None`) |
| `CollectedFees(Address)` | `i128` per asset | `create_commitment`, `early_exit` | `0` |

#### Admin / withdrawal

| Function | Auth | Notes |
|----------|------|-------|
| `set_creation_fee_bps(caller, bps)` | Treasurer or Admin (`is_treasurer`) | Rejects `bps > 10_000` → `InvalidFeeBps` |
| `set_fee_recipient(caller, recipient)` | Treasurer or Admin | Rejects zero address |
| `withdraw_fees(caller, asset_address, amount)` | Treasurer or Admin | Reentrancy-guarded; requires `FeeRecipient` set; `amount` must be positive; `amount <= CollectedFees(asset)`; decrements ledger then transfers to recipient |

#### Getters

- `get_creation_fee_bps()` → `u32`
- `get_fee_recipient()` → `Option<Address>`
- `get_collected_fees(asset_address)` → `i128`

---

### `attestation_engine`

#### Protocol verification fee

Charged per attestation written through `write_attestation`:

1. Read `AttestationFeeAmount` (default `0`).
2. If `amount > 0` and `AttestationFeeAsset` is set, transfer `amount` of that token from `caller` to the contract and increment `CollectedFees(fee_asset)`.

**Entrypoints that collect:**

| Entrypoint | Path | Fee charged |
|------------|------|-------------|
| `record_fees` | `_attest_internal` → `write_attestation` | Yes (once per call) |
| `record_drawdown` | `write_attestation` directly | Yes (once per `write_attestation` call; up to twice when a violation attestation is also recorded) |
| `batch_attest` | Inline persistence (no `write_attestation`) | **No** |

#### Storage keys

| Key | Type | Set by | Default |
|-----|------|--------|---------|
| `AttestationFeeAmount` | `i128` | `set_attestation_fee` | `0` |
| `AttestationFeeAsset` | `Address` | `set_attestation_fee` | unset |
| `FeeRecipient` | `Address` | `set_fee_recipient` | unset (`None`) |
| `CollectedFees(Address)` | `i128` per asset | `write_attestation` | `0` |

`TotalFees` is an analytics counter for `fee_generation` attestation payloads; it is **not** withdrawable protocol revenue.

#### Admin / withdrawal

| Function | Auth | Notes |
|----------|------|-------|
| `set_attestation_fee(caller, amount, asset)` | Admin | Rejects `amount < 0`; set `amount = 0` to disable |
| `set_fee_recipient(caller, recipient)` | Admin | |
| `withdraw_fees(caller, asset_address, amount)` | Admin | Requires `FeeRecipient`; `amount > 0`; `amount <= CollectedFees(asset)`; decrements ledger then token transfer |

#### Getters

- `get_attestation_fee()` → `(i128, Option<Address>)`
- `get_fee_recipient()` → `Option<Address>`
- `get_collected_fees(asset_address)` → `i128`

---

### `commitment_transformation`

#### Fee collected

On `create_tranches`:

1. Read `TransformationFeeBps` (initialized to `0`).
2. `fee_amount = fees::fee_from_bps(total_value, fee_bps)`.
3. If `fee_amount > 0`, caller transfers `fee_amount` of `fee_asset` to the contract; increment `CollectedFees(fee_asset)`.
4. `net_value = total_value - fee_amount` is split across tranches.

#### Storage keys

| Key | Type | Set by | Default |
|-----|------|--------|---------|
| `TransformationFeeBps` | `u32` | `initialize`, `set_transformation_fee` | `0` |
| `FeeRecipient` | `Address` | `set_fee_recipient` | unset (`None`) |
| `CollectedFees(Address)` | `i128` per asset | `create_tranches` | `0` |

#### Admin / withdrawal

| Function | Auth | Notes |
|----------|------|-------|
| `set_transformation_fee(caller, fee_bps)` | Admin | Rejects `fee_bps > 10_000` → `InvalidFeeBps` |
| `set_fee_recipient(caller, recipient)` | Admin | |
| `withdraw_fees(caller, asset_address, amount)` | Admin | Requires `FeeRecipient`; positive `amount`; `amount <= CollectedFees(asset)` |

#### Getters

- `get_transformation_fee_bps()` → `u32`
- `get_fee_recipient()` → `Option<Address>`
- `get_collected_fees(asset_address)` → `i128`

---

### `commitment_marketplace`

Marketplace fees are **settled at trade time** to `FeeRecipient`. There is no `CollectedFees` ledger and no `withdraw_fees`.

#### Fee collected

| Entrypoint | Base amount | Calculation |
|------------|-------------|-------------|
| `buy_nft` | `listing.price` | `SafeMath::div(SafeMath::mul(price, MarketplaceFee), 10_000)` |
| `accept_offer` | `offer.amount` | `(offer.amount * MarketplaceFee) / 10_000` |
| `end_auction` | `auction.current_bid` | Uses `min(MarketplaceFee, 10_000)` before `SafeMath::div(SafeMath::mul(bid, bps), 10_000)` |

Buyer/offerer pays seller proceeds plus marketplace fee; fee token transfer goes directly from payer (or contract escrow on auction) to `FeeRecipient`.

#### Storage keys

| Key | Type | Set by | Default |
|-----|------|--------|---------|
| `MarketplaceFee` | `u32` | `initialize`, `update_fee` | set at `initialize` |
| `FeeRecipient` | `Address` | `initialize` only | set at `initialize` |

`update_fee` is Admin-only but does **not** enforce `fee_basis_points <= 10_000` (unlike other contracts). Only `end_auction` clamps above 100%.

There are no public getters for `MarketplaceFee` or `FeeRecipient`.

#### Admin

| Function | Auth | Notes |
|----------|------|-------|
| `initialize(..., fee_basis_points, fee_recipient)` | `admin` | One-time setup |
| `update_fee(fee_basis_points)` | Admin | No bps range check |

---

## Withdrawal Model Comparison

| Contract | Ledger | Withdraw function | Recipient required | Who can withdraw |
|----------|--------|-------------------|--------------------|------------------|
| `commitment_core` | `CollectedFees(asset)` | `withdraw_fees` | Yes (`FeeRecipientNotSet`) | Treasurer or Admin |
| `attestation_engine` | `CollectedFees(asset)` | `withdraw_fees` | Yes | Admin |
| `commitment_transformation` | `CollectedFees(asset)` | `withdraw_fees` | Yes (`FeeRecipientNotSet`) | Admin |
| `commitment_marketplace` | None | N/A | `FeeRecipient` set at init | Fees sent on each sale |

All `withdraw_fees` implementations: validate sufficient ledger balance, decrement `CollectedFees`, then transfer tokens to `FeeRecipient`. Amount must be positive.

---

## Access Control Summary

| Contract | Set rate | Set recipient | Withdraw |
|----------|----------|---------------|----------|
| `commitment_core` | `set_creation_fee_bps` (Treasurer/Admin) | `set_fee_recipient` (Treasurer/Admin) | `withdraw_fees` (Treasurer/Admin) |
| `attestation_engine` | `set_attestation_fee` (Admin) | `set_fee_recipient` (Admin) | `withdraw_fees` (Admin) |
| `commitment_transformation` | `set_transformation_fee` (Admin) | `set_fee_recipient` (Admin) | `withdraw_fees` (Admin) |
| `commitment_marketplace` | `update_fee` (Admin) | `initialize` only | N/A (direct payout) |

---

## Future Extensions

The design allows fee tiers (e.g. different bps by commitment size or type) by extending storage and calculation in each contract. `commitment_marketplace` could add `CollectedFees` + `withdraw_fees` if deferred settlement is desired.
