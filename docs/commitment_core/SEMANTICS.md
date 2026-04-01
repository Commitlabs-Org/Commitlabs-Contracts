# `commitment_core` Semantics Guide

This document explains the technical semantics of Total Value Locked (TVL) accounting, commitment counters, and reentrancy protection in the `commitment_core` contract.

## Total Value Locked (TVL) Accounting

The `TotalValueLocked` (TVL) is a protocol-wide aggregate that tracks the total amount of underlying assets currently managed by the `commitment_core` contract across all active commitments.

### Increments
- **`create_commitment`**: Increments TVL by the `net_amount` (initial amount minus creation fees).

### Decrements
- **`settle`**: Decrements TVL by the `current_value` returned to the owner upon maturity.
- **`early_exit`**: Decrements TVL by the full `current_value` of the commitment (pre-penalty). Retention of the penalty as a fee is recorded separately in `CollectedFees`.
- **`allocate`**: Decrements TVL by the `amount` moved out of the core contract's custody into a target pool.

### Adjustments
- **`update_value`**: Adjusts TVL by the delta (`new_value - old_value`). This ensures that protocol metrics reflect the current estimated value of all locked assets.

## Commitment Counters and IDs

The contract uses a monotonic `u64` counter (`TotalCommitments`) to generate unique, human-readable IDs for each commitment.

### ID Format
Commitment IDs follow the pattern `c_<number>`, where `<number>` is the value of the `TotalCommitments` counter at the time of creation.

### Uniqueness
- The counter is persisted in instance storage and incremented atomically within `create_commitment`.
- Even if a commitment is settled or cleared, its ID is never reused.

## Reentrancy Guard Semantics

To protect against reentrancy attacks—specifically those involving token transfers or cross-contract calls—the contract implements a standard reentrancy guard.

### Scope
The reentrancy guard is applied to all state-changing functions that perform external calls or move assets:
- `create_commitment`
- `settle`
- `early_exit`
- `allocate`
- `withdraw_fees`

### Mechanism
1. **Check**: `require_no_reentrancy` verifies the guard is `false`.
2. **Lock**: `set_reentrancy_guard(true)` is called at the start of the function.
3. **Execute**: The function logic, including external calls, is executed.
4. **Unlock**: `set_reentrancy_guard(false)` is called before the function returns.

### Atomic Rollback
If a downstream call (e.g., to `commitment_nft` or a token contract) fails and panics, the entire Soroban transaction reverts, including the reentrancy guard state. This ensures the contract is never left in a permanently "locked" state due to a failure.

## Arithmetic Safety

All financial calculations (fees, penalties, TVL adjustments) use the `SafeMath` utility from `shared_utils`. This library wraps arithmetic operations to prevent overflows and underflows, ensuring protocol solvency even with extreme asset amounts.
