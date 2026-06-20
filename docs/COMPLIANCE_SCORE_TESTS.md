# Compliance Score Algorithm Tests

## Overview

Comprehensive test suite for the compliance score algorithm in the attestation engine. The compliance score is calculated based on multiple factors including violations, drawdowns, fee generation, and duration adherence.

## Algorithm Summary

The compliance score algorithm:

1. **Base Score**: Starts at 100
2. **Violation Penalties**: 
   - High severity: -30 points
   - Medium severity: -20 points
   - Low severity: -10 points
3. **Drawdown Penalty**: -1 point per percentage point over the max_loss_percent threshold
4. **Compliant Attestation Bonus**: +1 point for each compliant attestation (health_check, fee_generation, drawdown)
5. **Duration Adherence Bonus**: +10 points if commitment is on track (only in calculate_compliance_score)
6. **Clamping**: Final score is clamped between 0 and 100

## Test Cases Implemented

### 1. `test_compliance_score_no_attestations_default`
- **Scenario**: No attestations recorded
- **Expected**: Score = 100 (base score with duration bonus, clamped)
- **Status**: ✅ PASS

### 2. `test_compliance_score_only_positive_attestations`
- **Scenario**: Only compliant health_check attestations
- **Expected**: Score = 100 (base + bonuses, clamped)
- **Status**: ✅ PASS

### 3. `test_compliance_score_with_single_violation`
- **Scenario**: One medium severity violation
- **Expected**: Score = 80 (100 - 20)
- **Status**: ✅ PASS

### 4. `test_compliance_score_with_multiple_violations`
- **Scenario**: One low + one medium severity violation
- **Expected**: Score = 70 (100 - 10 - 20)
- **Status**: ✅ PASS

### 5. `test_compliance_score_with_drawdown_penalty`
- **Scenario**: 30% drawdown with 10% threshold (20% over)
- **Expected**: Score = 90 (100 + 10 duration - 20 drawdown)
- **Status**: ✅ PASS

### 6. `test_compliance_score_with_fees_and_drawdown`
- **Scenario**: 15% drawdown (5% over threshold) + fee_generation attestation
- **Expected**: Score = 100 (100 + 10 duration - 5 drawdown, clamped)
- **Status**: ✅ PASS

### 7. `test_compliance_score_clamped_at_zero`
- **Scenario**: 5 high severity violations (150 points penalty)
- **Expected**: Score = 0 (100 - 150, clamped at 0)
- **Status**: ✅ PASS

### 8. `test_compliance_score_clamped_at_100`
- **Scenario**: Perfect conditions, value gained
- **Expected**: Score = 100 (110 clamped to 100)
- **Status**: ✅ PASS

### 9. `test_compliance_score_mixed_attestations`
- **Scenario**: health_check + violation + drawdown attestations
- **Expected**: Score = 91 (100 + 1 health - 10 violation + 1 drawdown, clamped at 100 after first)
- **Status**: ✅ PASS

### 10. `test_compliance_score_decreases_on_violation` (existing)
- **Scenario**: High severity violation
- **Expected**: Score = 70 (100 - 30)
- **Status**: ✅ PASS

## Key Findings

### Two Score Calculation Methods

1. **Stored Metrics** (`get_stored_health_metrics`):
   - Updated incrementally by the `attest()` function
   - Applies violation penalties based on severity
   - Adds +1 bonus for compliant attestations
   - Does NOT apply drawdown penalties from commitment value
   - Does NOT add duration adherence bonus

2. **Calculated Score** (`calculate_compliance_score`):
   - Recalculates from scratch using all attestations
   - Counts violations at -20 points each (ignores severity)
   - Applies drawdown penalty from commitment value
   - Adds +10 duration adherence bonus
   - Used when no stored metrics exist

### Attestation Data Requirements

- **health_check**: No required fields (always valid)
- **violation**: Requires `violation_type` and `severity` fields
- **fee_generation**: Requires `fee_amount` field
- **drawdown**: Requires `drawdown_percent` field

## Issue #482 edge-case coverage

The `test_calculate_compliance_score_*` regression group pins the raw `calculate_compliance_score` path without pre-seeded stored `HealthMetrics` so future refactors cannot silently drift from the documented formula.

| Test | Scenario pinned | Expected invariant |
| --- | --- | --- |
| `test_calculate_compliance_score_no_attestations_defaults_to_full_score` | Active commitment with no attestations | Score defaults to `100`; `get_health_metrics` mirrors the score; `last_attestation`/fees stay zero; `verify_compliance` remains true. |
| `test_calculate_compliance_score_all_violations_clamps_and_marks_noncompliant` | Four non-compliant violation attestations at deterministic timestamps | Score is `30` (`100 - 4*20 + 10`), last timestamp is preserved, and `verify_compliance` returns false because score `< 50`. |
| `test_calculate_compliance_score_mixed_attestations_and_health_metrics_consistency` | Health check + violation + drawdown + fee-generation attestations | Score, latest drawdown, fee total, and max timestamp are identical through direct score calculation and `get_health_metrics`. |
| `test_calculate_compliance_score_single_drawdown_attestation_drives_verification` | One drawdown attestation above the max-loss rule | Score is clamped within `0..=100`; health metrics expose the attested drawdown; `verify_compliance` fails because drawdown exceeds the rule. |

## Test Coverage

✅ No attestations → default score  
✅ Only positive attestations → score at 100  
✅ With violations → score decreased per severity/stored metrics and flat raw-score penalty coverage
✅ With fees and drawdowns → score reflects formula and `HealthMetrics` aggregation
✅ Score clamped between 0 and 100  
✅ Multiple attestation types combined  
✅ `verify_compliance` and `get_health_metrics` consistency for score edge cases

## Running the Tests

```bash
# Run all compliance score tests
cd contracts/attestation_engine
cargo test test_compliance_score

# Run all attestation engine tests
cargo test
```

## Notes

- All tests use the stored metrics approach (via `attest()` function)
- The `calculate_compliance_score()` function is used when no stored metrics exist
- Severity-based penalties (10/20/30) are only applied in the `attest()` function
- The `calculate_compliance_score()` function uses a flat 20-point penalty per violation
