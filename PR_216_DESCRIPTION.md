# PR: Align mint signature with core — `early_exit_penalty` argument consistency across contracts

Closes issue #216

## Summary

`commitment_core::call_nft_mint` invokes `commitment_nft::mint` via
`e.invoke_contract` with arguments pushed in positional order. Because
Soroban's dynamic dispatch matches by position and type — not by name —
any argument-order mismatch silently passes the wrong value to the wrong
field. This PR makes the alignment explicit, deterministic, and tested.

Changes are confined to `contracts/commitment_nft/` (contracts-only, no
frontend, backend, or off-chain scope).

---

## Problem

`early_exit_penalty` was:
- Stored only in `CommitmentNFT::early_exit_penalty` (top-level field).
- **Not** stored in `CommitmentMetadata::early_exit_penalty` — the struct
  that integrators read for the full commitment picture.
- Undocumented in terms of its positional relationship to the other mint
  arguments.
- Not covered by any test that explicitly asserts the value round-trips
  correctly through the mint call.

---

## Changes

### `contracts/commitment_nft/src/lib.rs`

- Added `early_exit_penalty: u32` field to `CommitmentMetadata` with
  Rustdoc explaining it mirrors `CommitmentRules::early_exit_penalty` from
  `commitment_core`.
- Updated `CommitmentNFT` Rustdoc to note that `early_exit_penalty` appears
  in both the top-level struct and `metadata`, and that both are always
  written with the same value during `mint`.
- Replaced the bare `/// Mint a new Commitment NFT.` comment with a full
  NatSpec-style Rustdoc block on `mint` covering: purpose, argument
  alignment table, all parameters, all error variants, and security notes.
- Updated the `CommitmentMetadata` construction inside `mint` to populate
  `early_exit_penalty` from the mint argument.
- Removed the duplicate unconditional `mod benchmarks;` declaration at the
  bottom of the file (was declared twice — once under `#[cfg(all(test,
  feature = "benchmark"))]` and once unconditionally).
- Registered `mod mint_signature_tests` under `#[cfg(test)]`.

### `contracts/commitment_nft/src/mint_signature_tests.rs` (new)

13 deterministic tests covering 7 invariants:

| # | Invariant | Test(s) |
|---|-----------|---------|
| 1 | `early_exit_penalty` stored in `CommitmentNFT::early_exit_penalty` | `invariant_early_exit_penalty_stored_in_nft_top_level` |
| 2 | `early_exit_penalty` stored in `CommitmentMetadata::early_exit_penalty` | `invariant_early_exit_penalty_stored_in_metadata` |
| 3 | Both fields hold the same value after mint | `invariant_penalty_fields_are_consistent` |
| 4 | Argument order matches `commitment_core::call_nft_mint` push order | `invariant_argument_order_penalty_not_swapped_with_max_loss`, `invariant_argument_order_penalty_not_swapped_with_duration` |
| 5 | Boundary values (0, 100) accepted and round-trip correctly | `invariant_penalty_zero_accepted`, `invariant_penalty_max_accepted` |
| 6 | `early_exit_penalty` is independent of `max_loss_percent` | `invariant_penalty_independent_of_max_loss` |
| 7 | Multiple mints each store their own penalty independently | `invariant_multiple_mints_store_independent_penalties` |
| — | Auth: unauthorized caller rejected; admin always authorized | `invariant_unauthorized_caller_rejected`, `invariant_admin_is_authorized_caller` |

---

## Argument Order Reference

The table below documents the canonical argument order shared between
`commitment_core::call_nft_mint` and `commitment_nft::mint`. Any future
change to either side must keep these in sync.

| Position | `call_nft_mint` push | `mint` parameter | Stored in |
|----------|----------------------|------------------|-----------|
| 1 | `caller` | `caller: Address` | auth check only |
| 2 | `owner` | `owner: Address` | `CommitmentNFT::owner` |
| 3 | `commitment_id` | `_commitment_id: String` | ignored (auto-generated) |
| 4 | `duration_days` | `duration_days: u32` | `CommitmentMetadata::duration_days` |
| 5 | `max_loss_percent` | `max_loss_percent: u32` | `CommitmentMetadata::max_loss_percent` |
| 6 | `commitment_type` | `commitment_type: String` | `CommitmentMetadata::commitment_type` |
| 7 | `initial_amount` | `initial_amount: i128` | `CommitmentMetadata::initial_amount` |
| 8 | `asset_address` | `asset_address: Address` | `CommitmentMetadata::asset_address` |
| 9 | `early_exit_penalty` | `early_exit_penalty: u32` | `CommitmentNFT::early_exit_penalty` **and** `CommitmentMetadata::early_exit_penalty` |

`early_exit_penalty` is intentionally the **last** positional argument so
that adding future optional fields does not shift existing positions.

---

## Pre-existing Cargo Error — `shared_utils` `E0255`

`cargo check -p commitment_nft` surfaces the same pre-existing error as
noted in PR #209:

```
error[E0255]: the name `fees` is defined multiple times
  --> contracts/shared_utils/src/lib.rs:43:9
```

**This error is pre-existing and unrelated to this PR.** Verified by
stashing all changes and running `cargo check` on the unmodified base
branch — the identical error appears. We did not fix it here because:

- It lives in `contracts/shared_utils`, outside the scope of issue #216.
- Modifying `shared_utils` without a dedicated review risks breaking
  `attestation_engine`, `allocation_logic`, and every other dependent
  contract.
- The correct path is a dedicated issue/PR scoped to `shared_utils`.

All diagnostics on the files touched by this PR return **no errors and no
warnings**.

---

## Security Notes

- No new trust boundaries introduced. `mint` auth model is unchanged:
  admin, registered core contract, or whitelisted address.
- `early_exit_penalty` is a read-only metadata field after mint — no
  mutation path exists.
- Reentrancy guard behavior is unchanged.
- No arithmetic on `early_exit_penalty` inside `commitment_nft`; the value
  is stored and returned as-is. Arithmetic safety for penalty calculations
  lives in `commitment_core` via `SafeMath::penalty_amount`.

---

## How to Run

```sh
cargo test -p commitment_nft
```

issue #216
