# Health Metrics Aggregation Rules

This document defines the aggregation rules for health metrics in the Attestation Engine contract, specifically addressing the consistency requirements from Issue #150.

## Overview

The `get_health_metrics` function aggregates data from multiple attestations to provide a comprehensive view of a commitment's health status. Different metrics follow different aggregation semantics based on their business logic.

## Aggregation Rules

### 1. Fees Generated (`fees_generated`)

**Rule**: Cumulative Sum

**Behavior**: 
- All `fee_generation` attestations are summed together
- Each call to `record_fees` adds to the total
- The total persists across the lifetime of the commitment

**Implementation**:
```rust
// In get_health_metrics (lines 827-843)
for att in attestations.iter() {
    if att.attestation_type == fee_type {
        if let Some(fee_str) = att.data.get(fee_key.clone()) {
            if let Some(v) = Self::parse_i128_from_string(&e, &fee_str) {
                fees_generated = fees_generated.checked_add(v).unwrap_or(fees_generated);
            }
        }
    }
}
```

**Example**:
```rust
record_fees(cid, 10);   // fees_generated = 10
record_fees(cid, 20);   // fees_generated = 30
record_fees(cid, 5);    // fees_generated = 35
```

**Edge Cases**:
- Zero fees are included in the sum
- Overflow protection uses `checked_add` with fallback
- Negative fees are not allowed (validation in `record_fees`)

### 2. Drawdown Percent (`drawdown_percent`)

**Rule**: Latest Value Wins

**Behavior**:
- Each `record_drawdown` call overwrites the previous value
- Only the most recent drawdown percentage is stored
- Historical drawdown values are not preserved

**Implementation**:
```rust
// In update_health_metrics (lines 540-547)
if attestation.attestation_type == drawdown_type {
    let drawdown_percent_key = String::from_str(e, "drawdown_percent");
    if let Some(drawdown_str) = attestation.data.get(drawdown_percent_key) {
        if let Some(drawdown_val) = Self::parse_i128_from_string(e, &drawdown_str) {
            metrics.drawdown_percent = drawdown_val; // Overwrites previous value
        }
    }
}
```

**Example**:
```rust
record_drawdown(cid, 5);   // drawdown_percent = 5
record_drawdown(cid, 10);  // drawdown_percent = 10
record_drawdown(cid, 3);   // drawdown_percent = 3 (latest value)
```

**Compliance Check**:
- Drawdown is compared against `commitment.rules.max_loss_percent`
- If `drawdown_percent <= max_loss_percent`, the attestation is marked compliant
- Otherwise, it's marked non-compliant and affects the compliance score

### 3. Compliance Score (`compliance_score`)

**Rule**: Incremental Updates with Bounds

**Behavior**:
- Starts at 100 for new commitments
- Decreases for violations and non-compliant attestations
- Increases for compliant attestations (capped at 100)
- Persists in stored health metrics

**Implementation**:
```rust
// In update_health_metrics (lines 548-566)
if attestation.attestation_type == violation {
    let penalty = match severity {
        "high" => 30u32,
        "medium" => 20u32,
        _ => 10u32,
    };
    metrics.compliance_score = metrics.compliance_score.saturating_sub(penalty);
}

// Compliance bonus for compliant attestations (lines 568-573)
if attestation.is_compliant && attestation.attestation_type != violation {
    metrics.compliance_score = core::cmp::min(100, metrics.compliance_score.saturating_add(1));
}
```

**Penalty Structure**:
- **High severity violations**: -30 points
- **Medium severity violations**: -20 points  
- **Low severity violations**: -10 points
- **Non-compliant attestations**: Treated as violations
- **Compliant attestations**: +1 point (capped at 100)

**Example**:
```rust
// Initial: compliance_score = 100
record_fees(cid, 10);           // compliance_score = 100 (compliant, +1 but capped)
record_drawdown(cid, 5);        // compliance_score = 100 (compliant, +1 but capped)
record_drawdown(cid, 15);       // compliance_score = 100 (non-compliant, -20 for violation)
```

### 4. Last Attestation (`last_attestation`)

**Rule**: Maximum Timestamp

**Behavior**:
- Tracks the most recent attestation timestamp
- Updated on every attestation regardless of type
- Used for freshness checks and analytics

## Storage vs Calculation

### Stored Metrics
- `compliance_score` is stored and incrementally updated
- `drawdown_percent` is stored (latest value)
- `fees_generated` is calculated dynamically from attestations

### Calculated Metrics
- `fees_generated` is recalculated each time `get_health_metrics` is called
- `drawdown_percent` can also be calculated from commitment value changes
- `compliance_score` uses stored value but can be recalculated if missing

## Test Coverage

The aggregation rules are tested in `health_metrics_consistency_tests.rs`:

1. **Fee Aggregation Tests**:
   - `test_multiple_record_fees_cumulative_sum`
   - `test_record_fees_zero_amount`
   - `test_record_fees_large_amounts`

2. **Drawdown Aggregation Tests**:
   - `test_multiple_record_drawdown_latest_value`
   - `test_record_drawdown_compliance_check`
   - `test_record_drawdown_non_compliant`

3. **Compliance Score Tests**:
   - `test_compliance_score_updates_after_fees`
   - `test_compliance_score_updates_after_drawdown`
   - `test_compliance_score_with_violation_attestation`

4. **Mixed Operations Tests**:
   - `test_mixed_fees_and_drawdown_operations`
   - `test_health_metrics_persistence`

## Performance Considerations

1. **Fee Calculation**: Iterates through all attestations each time
   - Consider optimization for high-volume commitments
   - Current implementation prioritizes correctness over performance

2. **Storage Efficiency**: 
   - Compliance score stored to avoid recalculation
   - Drawdown percent stored as latest value
   - Fees calculated to ensure accuracy

3. **Overflow Protection**:
   - Uses `checked_add` and `checked_mul` for arithmetic safety
   - Falls back to current value on overflow

## Future Enhancements

1. **Historical Tracking**: Consider storing historical drawdown values
2. **Fee Optimization**: Cache cumulative fees for performance
3. **Compliance Algorithms**: More sophisticated compliance scoring
4. **Analytics**: Additional metrics for monitoring and reporting

## Conclusion

These aggregation rules ensure consistent and predictable behavior for health metrics across multiple attestations. The design balances accuracy, performance, and storage efficiency while maintaining clear business logic for each metric type.
