# PR: Add record_fees Fee Amount Validation

## Description

Add validation to the `record_fees` function to reject negative or invalid fee amounts.

## Changes

### 1. `contracts/attestation_engine/src/lib.rs`

Added fee amount validation in `record_fees` function:

```rust
if fee_amount < 0 {
    return Err(AttestationError::InvalidFeeAmount);
}
```

**Details:**

- Rejects: `fee_amount < 0`
- Allows: `fee_amount >= 0`
- Error type: `InvalidFeeAmount` (existing)

### 2. `tests/integration/cross_contract_tests.rs`

Added comprehensive test: `test_record_fees_validation`

**Test Cases:**

- ❌ `-1` → `InvalidFeeAmount`
- ✅ `0` → Success
- ✅ `50_000_000` → Success
- ✅ `1_000_000_000_000` → Success
- ❌ `i128::MIN` → `InvalidFeeAmount`

## Test Results

✅ `test_record_fees_validation` - PASS
✅ `test_record_fees_record_drawdown_access_control` - PASS (no regression)
✅ All 11 attestation tests - PASS

## Impact

- **Files Changed:** 2
- **Lines Added:** 95 (5 implementation + 90 tests)
- **Breaking Changes:** None
- **Regressions:** None

## Checklist

- [x] Validation rejects negative amounts
- [x] Zero fees allowed (non-negative)
- [x] Positive amounts processed
- [x] All test scenarios pass
- [x] No regressions
- [x] Release build successful
