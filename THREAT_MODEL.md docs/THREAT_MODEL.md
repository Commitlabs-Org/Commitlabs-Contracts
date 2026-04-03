# CommitmentCore Security Review — Functions Without `require_auth`

> **Issue:** #203  
> **Contract:** `contracts/commitment_core/src/lib.rs`  
> **Scope:** Soroban / Rust workspace only — no frontend, backend, or off-chain services.

---

## 1. Trust Boundaries

| Actor | Storage keys writable | Entry points |
|---|---|---|
| **Admin** | `Admin`, `NftContract`, `AllocationContract`, `AuthorizedUpdaters`, `AuthorizedAllocator(*)`, `CreationFeeBps`, `FeeRecipient`, `CollectedFees(*)` (via withdraw) | `pause`, `unpause`, `add_authorized_contract`, `remove_authorized_contract`, `add_updater`, `remove_updater`, `set_allocation_contract`, `set_rate_limit`, `set_rate_limit_exempt`, `set_emergency_mode`, `emergency_withdraw`, `set_creation_fee_bps`, `set_fee_recipient`, `withdraw_fees` |
| **Commitment owner** | `Commitment(id)` (status only, via `early_exit`) | `create_commitment`, `early_exit` |
| **Authorized updater** | `Commitment(id)` (`current_value` + `status`), `TotalValueLocked` | `update_value` |
| **Authorized allocator** | `Commitment(id)` (`current_value`), token transfer | `allocate` |
| **Anyone (keeper / indexer / attestation_engine)** | None | All read-only getters, `settle` (permissionless — see §3) |

---

## 2. Functions WITHOUT `require_auth` — Enumerated

### 2.1 Read-Only Functions (no threat — no state mutation)

All functions below touch no mutable storage keys. Their absence of `require_auth` is intentional and safe.

| Function | Rationale |
|---|---|
| `get_commitment` | Read-only. Consumed by `attestation_engine` without admin context. |
| `get_owner_commitments` | Read-only. Off-chain indexers and UIs enumerate owner portfolios. |
| `list_commitments_by_owner` | Alias of the above. |
| `get_total_commitments` | Read-only counter. |
| `get_total_value_locked` | Read-only aggregate. |
| `get_admin` | Read-only. Admin address is not a secret. |
| `get_nft_contract` | Read-only. NFT contract address is not a secret. |
| `get_authorized_updaters` | Read-only. |
| `is_paused` | Read-only. |
| `is_emergency_mode` | Read-only. |
| `is_authorized` | Read-only predicate used internally by `allocate`. |
| `check_violations` | Read-only; emits an event but **mutates no storage keys**. The event emission is not a security concern. |
| `get_violation_details` | Read-only. |
| `get_creation_fee_bps` | Read-only. |
| `get_fee_recipient` | Read-only. |
| `get_collected_fees` | Read-only. |
| `get_commitments_created_between` | Read-only O(n) scan. Caller pays gas cost. |

**Mitigation:** None required. All storage keys accessed are read under immutable borrows; Soroban's execution model prevents any side-channel write from a read-only call.

---

### 2.2 `settle` — Permissionless by Design

```
pub fn settle(e: Env, commitment_id: String)
```

**Why no `require_auth`:**  
Settlement is the natural conclusion of an expired commitment. Requiring the owner to be online at expiry would lock funds in contracts where owners lose their keys or are simply offline. Keeper bots, liquidators, and any other on-chain actor should be able to trigger settlement without special privileges.

**Threat model:**

| Threat | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Keeper settles early, stealing funds | Low | High | `NotExpired` check (`current_time < expires_at`) fires before any state change. |
| Keeper redirects settlement to themselves | None | N/A | Token transfer destination is hardcoded to `commitment.owner` — the caller's identity is irrelevant. |
| Double-settle drains contract balance | Low | Medium | `AlreadySettled` guard: status must be `"active"` at entry; set to `"settled"` before transfer. |
| Settlement of violated commitment | Low | Low | `NotActive` guard: violated/early-exit commitments have non-`"active"` status. |
| Reentrancy via NFT `settle` callback | Low | High | Reentrancy guard set to `true` before any state change; NFT call is last. Soroban atomicity means panic in NFT call rolls back the entire transaction. |

**Conclusion:** Permissionless settlement is correct. The token destination is owner-locked; expiry and status guards prevent premature or duplicate settlement.

---

## 3. Functions WITH `require_auth` — Summary

These all call `owner.require_auth()`, `caller.require_auth()` (via `require_admin`), or the explicit auth path in `update_value`.

| Function | Auth mechanism |
|---|---|
| `create_commitment` | `owner.require_auth()` |
| `early_exit` | `caller.require_auth()` + ownership equality check |
| `update_value` | `caller.require_auth()` + `is_authorized_updater` check |
| `allocate` | `caller.require_auth()` + `is_authorized` check |
| `pause` / `unpause` | `require_admin` (admin `require_auth` + equality) |
| `add_authorized_contract` / `remove_authorized_contract` | `require_admin` |
| `add_updater` / `remove_updater` | `require_admin` |
| `set_allocation_contract` | `require_admin` |
| `set_rate_limit` / `set_rate_limit_exempt` | `require_admin` |
| `set_emergency_mode` | `require_admin` |
| `emergency_withdraw` | `require_admin` + emergency mode check |
| `set_creation_fee_bps` | `require_admin` |
| `set_fee_recipient` | `require_admin` |
| `withdraw_fees` | `require_admin` + reentrancy guard |

---

## 4. `update_value` Auth Fix (Issue #203 Core Fix)

**Problem (pre-fix):** `update_value` accepted a `commitment_id` and `new_value` without verifying the caller's identity. Any address could manipulate `current_value`, which drives loss-percent computation and settlement amounts.

**Fix applied:**

```rust
pub fn update_value(e: Env, caller: Address, commitment_id: String, new_value: i128) {
    // 1. Soroban framework validates caller's cryptographic signature.
    caller.require_auth();

    // 2. Caller must be admin or in AuthorizedUpdaters.
    if !is_authorized_updater(&e, &caller) {
        fail(&e, CommitmentError::NotAuthorizedUpdater, "update_value");
    }
    // ...
}
```

**Trust model for authorized updaters:**

Authorized updaters are trusted to report accurate off-chain price or value data. A compromised updater can:
- Drive a commitment to `"violated"` status (stopping further value updates).
- Reduce `current_value` before early exit, lowering the returned amount.

A compromised updater **cannot**:
- Withdraw tokens directly — `emergency_withdraw` requires admin + emergency mode.
- Settle before expiry — expiration check is independent of `current_value`.
- Change ownership, NFT contract, or fee configuration.

**Recommendation:** Use a time-weighted oracle or multi-sig updater for production deployments.

---

## 5. Reentrancy Model

The reentrancy guard at `DataKey::ReentrancyGuard` (a boolean flag) is set to `true` at the entry of every function that performs external calls and cleared before returning. Soroban's single-threaded execution model means no true concurrent reentrancy is possible, but the guard defends against re-entrant calls routed through `commitment_nft` cross-contract callbacks.

Functions with reentrancy guard active: `create_commitment`, `settle`, `early_exit`, `allocate`, `withdraw_fees`.

---

## 6. Arithmetic Safety

All financial arithmetic is delegated to `shared_utils::SafeMath`:

- `SafeMath::sub` — panics on underflow.
- `SafeMath::add` — panics on overflow.
- `SafeMath::penalty_amount` — computes `value * percent / 100`; integer division truncates toward zero (protocol benefits).
- `SafeMath::loss_percent` — computes `(amount - current_value) * 100 / amount`; guarded by `amount > 0` check to prevent division by zero.

**i128 range:** Stellar asset supply is capped at 100 billion × 10^7 stroops ≈ 10^18, well within i128 max (≈ 1.7 × 10^38). No overflow risk under normal protocol operation.

**Rounding:** Integer division is floor (truncation). For penalty calculations this means the protocol retains the fractional stroop; for loss-percent calculations the reported loss is floored, meaning the exact boundary case (`loss_percent == max_loss_percent`) does **not** trigger a violation (strict `>` comparison).

---

## 7. Cross-Contract Call Graph

```
create_commitment  →  commitment_nft::mint
settle             →  commitment_nft::settle
early_exit         →  commitment_nft::mark_inactive
allocate           →  token::transfer (SAC)
```

Full call-graph threat review: [`docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md`](./CORE_NFT_ATTESTATION_THREAT_REVIEW.md)

---

## 8. Deployment Note — `initialize` Front-Running Risk

`initialize` has no `require_auth` by design (the admin address does not exist yet at deploy time). Deployers **must** call `initialize` in the same transaction as the `upload_contract` + `create_contract` operations to prevent a front-runner from claiming admin before the legitimate deployer. Once initialized, a second `initialize` call panics with `AlreadyInitialized`.