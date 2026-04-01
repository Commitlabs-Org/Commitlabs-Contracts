# Attestation Types and Compliance Scoring

This document provides comprehensive guidance for integrators on using attestation types and compliance scoring in the Commitlabs protocol.

## Overview

The attestation engine provides a flexible framework for recording various types of on-chain attestations and computing compliance scores for commitments. This enables transparent monitoring and evaluation of commitment performance.

## Attestation Types

### Supported Types

The attestation engine supports four core attestation types:

#### 1. Health Check (`health_check`)
- **Purpose**: General health monitoring and status updates
- **Required Data**: None (optional fields allowed)
- **Use Cases**: Periodic health verifications, status confirmations
- **Compliance Impact**: Neutral (no score change)

#### 2. Violation (`violation`)
- **Purpose**: Record rule violations or non-compliant behavior
- **Required Data**: 
  - `violation_type`: String describing violation category
  - `severity`: String ("high", "medium", "low")
- **Compliance Impact**: Negative (-30 for high, -20 for medium, -10 for low)

#### 3. Fee Generation (`fee_generation`)
- **Purpose**: Record fee income and revenue generation
- **Required Data**: 
  - `fee_amount`: String representing fee amount in base units
- **Compliance Impact**: Positive (based on fee performance vs threshold)

#### 4. Drawdown (`drawdown`)
- **Purpose**: Record value drawdown and loss events
- **Required Data**: 
  - `drawdown_percent`: String representing percentage loss (basis points)
- **Compliance Impact**: Negative if exceeds commitment threshold

### Recording Attestations

#### Direct Attestation
```rust
// Basic attestation recording
AttestationEngineContract::attest(
    env,
    verifier_address,
    "commitment_123".into(),
    "health_check".into(),
    Map::new(&env), // Empty data for health_check
    true, // is_compliant
)?;
```

#### Convenience Functions
```rust
// Record fee generation
AttestationEngineContract::record_fees(
    env,
    verifier_address,
    "commitment_123".into(),
    1000000 // 1 unit in base units
)?;

// Record drawdown (auto-determines compliance)
AttestationEngineContract::record_drawdown(
    env,
    verifier_address,
    "commitment_123".into(),
    1500 // 15% drawdown
)?;
```

## Compliance Scoring

### Score Calculation

Compliance scores range from 0-100 and are calculated based on:

1. **Base Score**: 100 points
2. **Violation Penalties**: -20 points per violation
3. **Drawdown Penalties**: -1 point per percentage point over threshold
4. **Fee Performance Bonus**: +1 point per percentage of expected fees met
5. **Duration Adherence Bonus**: +10 points if on track

### Score Factors

#### Violation Impact
- Each violation attestation reduces score by 20 points
- Non-compliant attestations also count as violations
- Score never goes below 0

#### Drawdown Analysis
```rust
// Example: 25% drawdown on 20% max loss commitment
max_loss_percent = 20;
drawdown_percent = 25;
over_threshold = 5; // 25 - 20
score_reduction = 5; // -5 points
```

#### Fee Performance
```rust
// Example: 80% of expected fees generated
min_fee_threshold = 1000000;
total_fees = 800000;
fee_percent = 80;
score_bonus = 80; // +80 points (capped at 100)
```

### Score Interpretation

| Score Range | Interpretation | Recommended Action |
|-------------|----------------|-------------------|
| 90-100 | Excellent | Maintain current strategy |
| 70-89 | Good | Monitor for improvements |
| 50-69 | Fair | Review performance metrics |
| 30-49 | Poor | Consider strategy adjustment |
| 0-29 | Critical | Immediate intervention needed |

## Integration Examples

### Verifier Integration

```rust
// Setup verifier authorization
AttestationEngineContract::add_verifier(
    env,
    admin_address,
    verifier_address
)?;

// Record periodic health check
let mut health_data = Map::new(&env);
health_data.set("status".into(), "healthy".into());
health_data.set("last_check".into(), "2024-01-15".into());

AttestationEngineContract::attest(
    env,
    verifier_address,
    "commitment_123".into(),
    "health_check".into(),
    health_data,
    true
)?;
```

### Compliance Monitoring

```rust
// Check current compliance status
let is_compliant = AttestationEngineContract::verify_compliance(
    env,
    "commitment_123".into()
);

// Get detailed compliance score
let score = AttestationEngineContract::calculate_compliance_score(
    env,
    "commitment_123".into()
);

// Get health metrics
let metrics = AttestationEngineContract::get_health_metrics(
    env,
    "commitment_123".into()
);
```

### Batch Operations

```rust
// Record multiple attestations efficiently
let mut attestations = Vec::new(&env);

// Add health check
attestations.push_back(AttestParams {
    commitment_id: "commitment_123".into(),
    attestation_type: "health_check".into(),
    data: Map::new(&env),
    is_compliant: true
});

// Add fee generation
let mut fee_data = Map::new(&env);
fee_data.set("fee_amount".into(), "500000".into());
attestations.push_back(AttestParams {
    commitment_id: "commitment_123".into(),
    attestation_type: "fee_generation".into(),
    data: fee_data,
    is_compliant: true
});

// Execute batch
let result = AttestationEngineContract::batch_attest(
    env,
    verifier_address,
    attestations,
    BatchMode::Atomic
)?;
```

## Security Considerations

### Access Control
- Only authorized verifiers can record attestations
- Admin controls verifier whitelist
- Rate limiting prevents spam attestations

### Data Validation
- Attestation types are strictly validated
- Required data fields are enforced
- Commitment existence is verified

### Reentrancy Protection
- All attestation functions use reentrancy guards
- State changes follow checks-effects-interactions pattern

## Best Practices

### For Verifiers
1. **Validate Data**: Ensure all required fields are present
2. **Use Convenience Functions**: Prefer `record_fees` and `record_drawdown` when applicable
3. **Batch Operations**: Use batch attestation for multiple records
4. **Monitor Limits**: Respect rate limits and batch size constraints

### For Integrators
1. **Monitor Scores**: Regularly check compliance scores
2. **Handle Events**: Listen for attestation and score update events
3. **Validate Commitments**: Verify commitment existence before attestation
4. **Error Handling**: Implement proper error handling for all operations

## Event Monitoring

### Key Events

#### AttestationRecorded
```rust
// Emitted when any attestation is recorded
// Topics: ("AttestationRecorded", commitment_id, verifier)
// Data: (attestation_type, is_compliant, timestamp)
```

#### ScoreUpd
```rust
// Emitted when compliance score is updated
// Topics: ("ScoreUpd", commitment_id)
// Data: (new_score, timestamp)
```

#### ViolationRecorded
```rust
// Emitted when violation is recorded via drawdown
// Topics: ("ViolationRecorded", commitment_id)
// Data: (drawdown_percent, max_loss_percent, timestamp)
```

### Event Integration
```rust
// Example: Listen for compliance score updates
let events = env.events().all();
for event in events.iter() {
    if event.topics[0] == Symbol::new(&env, "ScoreUpd") {
        let commitment_id = event.topics[1];
        let new_score = event.data[0];
        // Handle score update
    }
}
```

## Troubleshooting

### Common Issues

#### Invalid Attestation Data
- **Error**: `InvalidAttestationData`
- **Solution**: Ensure all required fields are present for the attestation type

#### Unauthorized Access
- **Error**: `Unauthorized`
- **Solution**: Verify caller is in verifier whitelist

#### Commitment Not Found
- **Error**: `CommitmentNotFound`
- **Solution**: Verify commitment exists in core contract

#### Rate Limiting
- **Error**: Rate limit exceeded
- **Solution**: Implement backoff strategy or request exemption

### Debugging Tips

1. **Use View Functions**: Query current state before operations
2. **Check Events**: Monitor emitted events for operation details
3. **Verify Authorization**: Confirm verifier status
4. **Validate Data**: Double-check attestation data format

## Migration and Upgrades

### Version Compatibility
- Storage schema versioning ensures safe upgrades
- Migration functions handle data format changes
- Backward compatibility maintained where possible

### Data Persistence
- Attestations stored in persistent storage
- Health metrics cached for performance
- Analytics counters maintained across upgrades

This documentation provides the foundation for integrating with the attestation engine's attestation types and compliance scoring features. For additional technical details, refer to the contract source code and test files.
