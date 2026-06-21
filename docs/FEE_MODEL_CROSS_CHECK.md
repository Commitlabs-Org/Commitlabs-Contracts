# Fee Model Cross-Check: Documentation vs Implementation

Reconciled against `docs/FEES.md` and the current `lib.rs` of each fee-collecting crate. Last verified: implementation on branch `master` at time of `docs/unified-fee-model`.

## Executive Summary

| Contract | `docs/FEES.md` coverage | Implementation status | Notes |
|----------|-------------------------|----------------------|-------|
| `commitment_core` | Creation + early exit | **Implemented** | Treasurer/Admin auth (not Admin-only as older docs stated) |
| `attestation_engine` | Verification fee | **Implemented** | `batch_attest` does not collect verification fees |
| `commitment_transformation` | Transformation fee | **Implemented** | Matches module-level rounding docs |
| `commitment_marketplace` | Marketplace sale fee | **Implemented** | No `CollectedFees` / `withdraw_fees`; direct payout model |

All four contracts are now documented in a single reconciled view in `docs/FEES.md`.

---

## `commitment_core`

### Documented vs code

| Item | `docs/FEES.md` | `contracts/commitment_core/src/lib.rs` | Match |
|------|----------------|----------------------------------------|-------|
| `CreationFeeBps` storage | Yes | `DataKey::CreationFeeBps` | ✅ |
| `CollectedFees(Address)` | Yes | `DataKey::CollectedFees(Address)` | ✅ |
| `FeeRecipient` | Yes | `DataKey::FeeRecipient` | ✅ |
| Creation fee on `create_commitment` | `fee_from_bps`, credit `CollectedFees` | Lines ~478–635 | ✅ |
| Early exit penalty to `CollectedFees` | `SafeMath::penalty_amount` (percent / 100) | Lines ~1190–1204 | ✅ |
| `set_creation_fee_bps` validates 0–10000 | Yes | `bps > fees::BPS_MAX` | ✅ |
| `withdraw_fees` semantics | Treasurer/Admin, recipient required, cap by ledger | Lines ~1495–1537 | ✅ |
| Getters | `get_creation_fee_bps`, `get_fee_recipient`, `get_collected_fees` | Lines ~1540–1558 | ✅ |

### Corrections from prior cross-check

The previous version of this file listed commitment_core fee infrastructure as **missing**. That gap has been closed:

- Storage keys exist.
- `create_commitment` collects creation fees.
- `early_exit` credits penalties to `CollectedFees`.
- Admin functions and getters are implemented with tests in `fee_tests.rs`.

### Minor doc nuance

- Fee admin uses **`is_treasurer`** (Admin is implicitly Treasurer), not bare Admin-only.
- Early exit uses **percent (÷ 100)**, not basis points (÷ 10_000).

---

## `attestation_engine`

### Documented vs code

| Item | `docs/FEES.md` | `contracts/attestation_engine/src/lib.rs` | Match |
|------|----------------|-------------------------------------------|-------|
| `AttestationFeeAmount` / `AttestationFeeAsset` | Yes | `DataKey` enum ~125–128 | ✅ |
| `CollectedFees(Address)` | Yes | `DataKey::CollectedFees(Address)` | ✅ |
| `FeeRecipient` | Yes | `DataKey::FeeRecipient` | ✅ |
| Fee collection in `write_attestation` | Transfer when `amount > 0` and asset set | Lines ~935–956 | ✅ |
| `set_attestation_fee` | Admin, `amount >= 0` | Lines ~2103–2131 | ✅ |
| `withdraw_fees` | Admin, positive amount, ledger cap | Lines ~2160–2196 | ✅ |
| Getters | `get_attestation_fee`, `get_fee_recipient`, `get_collected_fees` | Lines ~2200–2220 | ✅ |

### Discrepancies / caveats

| Topic | Detail |
|-------|--------|
| `record_fees` vs protocol fee | `record_fees` records `fee_generation` attestation **data** and updates `TotalFees` analytics. Protocol revenue is the fixed `AttestationFeeAmount` charged inside `write_attestation`. |
| `batch_attest` | Does **not** call `write_attestation`; verification fees are **not** collected on batch path. |
| `record_drawdown` | May invoke `write_attestation` twice (drawdown + violation), charging verification fee up to twice per call when configured. |

---

## `commitment_transformation`

### Documented vs code

| Item | `docs/FEES.md` | `contracts/commitment_transformation/src/lib.rs` | Match |
|------|----------------|--------------------------------------------------|-------|
| `TransformationFeeBps` | Yes | `DataKey::TransformationFeeBps`, default `0` at init | ✅ |
| `CollectedFees(asset)` | Yes | Credited in `create_tranches` | ✅ |
| `FeeRecipient` | Yes | `set_fee_recipient` | ✅ |
| Fee formula | `(total_value * bps) / 10_000` via `fuzzing::checked_fee_from_bps` | Lines ~529–543 | ✅ |
| Rounding / dust | Floor toward zero; tranche dust documented | Module docs lines ~27–35 | ✅ |
| `set_transformation_fee` | 0–10000 bps | `fee_bps > 10000` → `InvalidFeeBps` | ✅ |
| `withdraw_fees` | Admin, ledger cap | Lines ~1009–1031 | ✅ |
| Getters | `get_transformation_fee_bps`, `get_fee_recipient`, `get_collected_fees` | Lines ~961–1043 | ✅ |

No material discrepancies.

### Tranche fee/dust invariants

Regression tests in `contracts/commitment_transformation/src/tests.rs` pin the
following properties for `create_tranches`:

- accepted ratios must sum to exactly `10_000` bps; `9_999`, `10_001`, empty
  vectors, and mismatched risk-level lengths are rejected;
- `fee_paid == floor(total_value * TransformationFeeBps / 10_000)` and the same
  amount is credited to `CollectedFees(asset)`;
- tranche allocation conserves the post-fee net value as
  `sum(tranche.amount) + dust == total_value - fee_paid`; and
- integer-division dust is deterministic and bounded by
  `0 <= dust <= tranche_count - 1`.

---

## `commitment_marketplace`

### Documented vs code

| Item | `docs/FEES.md` | `contracts/commitment_marketplace/src/lib.rs` | Match |
|------|----------------|-----------------------------------------------|-------|
| `MarketplaceFee` | Yes | `DataKey::MarketplaceFee` | ✅ |
| `FeeRecipient` | Yes | `DataKey::FeeRecipient`, set at `initialize` | ✅ |
| Sale entrypoints | `buy_nft`, `accept_offer`, `end_auction` | Fee deducted from price/bid | ✅ |
| `CollectedFees` / `withdraw_fees` | Not present (by design) | No such keys or functions | ✅ |
| Direct payout | Fee transferred to `FeeRecipient` at settlement | `buy_nft` ~640–642, `accept_offer` ~905–906, `end_auction` ~1302–1308 | ✅ |

### Discrepancies / caveats

| Topic | Detail |
|-------|--------|
| Bps validation on set | `initialize` and `update_fee` do **not** reject `fee_basis_points > 10_000`. |
| Bps validation on settle | Only `end_auction` clamps fee bps to `10_000`; `buy_nft` and `accept_offer` use stored value as-is. |
| Fee recipient updates | No `set_fee_recipient` after init; recipient is fixed at `initialize`. |
| Getters | No public `get_marketplace_fee` or `get_fee_recipient`. |

Prior `docs/FEES.md` listed marketplace fees as "TBD". They are now documented as implemented with a direct-payout model.

---

## Shared utilities

| Item | `shared_utils::fees` | Used by |
|------|---------------------|---------|
| `BPS_SCALE` / `BPS_MAX` = 10_000 | `fees.rs` | `commitment_core`, `commitment_transformation` |
| `checked_fee_from_bps` — checked floor division | `commitment_core::fuzzing` | Creation fees |
| `fee_from_bps` — floor division | `fees.rs` | Transformation fees |
| `SafeMath::penalty_amount` — percent ÷ 100 | `math.rs` | `commitment_core::early_exit` |
| `SafeMath::div(mul(price, bps), 10_000)` | `math.rs` | `commitment_marketplace` listings/auctions |

---

## Verification checklist

Use this when changing fee logic or updating docs:

- [ ] `commitment_core`: `create_commitment` credits `CollectedFees`; `early_exit` credits penalty; `withdraw_fees` caps at ledger
- [ ] `attestation_engine`: `write_attestation` collects `AttestationFeeAmount`; `batch_attest` behavior documented if changed
- [ ] `commitment_transformation`: `create_tranches` fee transfer + ledger; tranche dust note still accurate
- [ ] `commitment_marketplace`: sale paths pay `FeeRecipient`; document any new bps validation
- [ ] `docs/FEES.md` summary table matches all four crates
- [ ] Run `cargo test --target wasm32v1-none --release`

---

## Historical note

An earlier draft of this cross-check (pre-implementation) tracked missing `commitment_core` fee infrastructure. That work is complete; this file now serves as a living reconciliation log between `docs/FEES.md` and contract source.
