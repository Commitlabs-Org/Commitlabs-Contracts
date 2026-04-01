# Risk Tranche Management

## Overview

The Commitment Transformation contract supports transforming commitments into **risk tranches** - sliced portions of a commitment with different risk profiles (senior, mezzanine, equity). This document describes the risk tranche data model, lifecycle management, and integration patterns.

## Data Model

### RiskTranche Struct

```rust
pub struct RiskTranche {
    pub tranche_id: String,              // Unique identifier
    pub transformation_id: String,       // Parent tranche set reference
    pub commitment_id: String,           // Original commitment reference
    pub risk_level: String,              // "senior", "mezzanine", "equity"
    pub amount: i128,                    // Current allocation
    pub share_bps: u32,                  // Share in basis points (0-10000)
    pub created_at: u64,                 // Creation timestamp
    pub status: TrancheStatus,           // Active or Closed
    pub updated_at: u64,                 // Last update timestamp
}
```

### TrancheStatus Enum

```rust
pub enum TrancheStatus {
    Active,    // Tranche can be modified
    Closed,    // Tranche is frozen (one-way transition)
}
```

### TrancheSet Structure

A `TrancheSet` groups multiple tranches created from a single transformation:

```rust
pub struct TrancheSet {
    pub transformation_id: String,
    pub commitment_id: String,
    pub owner: Address,
    pub total_value: i128,
    pub tranches: Vec<RiskTranche>,
    pub fee_paid: i128,
    pub created_at: u64,
}
```

## Lifecycle

### Creation

Tranches are created via `create_tranches()`:

1. Caller must be authorized (owner or authorized transformer)
2. Tranche shares must sum to 10000 bps (100%)
3. Each tranche is initialized with:
   - `status: TrancheStatus::Active`
   - `created_at: ledger timestamp`
   - `updated_at: ledger timestamp`
4. Individual tranches are stored for direct access
5. `TrancheCreated` event is emitted

### Updates

Active tranches can be modified:

- **`update_tranche()`**: Change risk level metadata
- **`allocate_to_tranche()`**: Adjust allocation amount (positive or negative)
- **`close_tranche()`**: Transition to closed state (irreversible)

All modifications require:
- Caller authorization (tranche owner)
- Tranche must be in `Active` status
- Reentrancy guard protection

### Closure

Closed tranches cannot be modified. Any attempt to update/allocate on a closed tranche fails with `InvalidState` error.

## Functions

### Read Operations

| Function | Description | Auth Required |
|----------|-------------|---------------|
| `get_tranche(tranche_id)` | Get individual tranche by ID | No |
| `get_tranche_set(transformation_id)` | Get full tranche set | No |
| `get_commitment_tranche_sets(commitment_id)` | List all tranche sets for a commitment | No |

### Write Operations

| Function | Description | Auth Required |
|----------|-------------|---------------|
| `create_tranches(...)` | Create new tranche set from commitment | Yes (owner/authorized) |
| `update_tranche(caller, tranche_id, risk_level)` | Update tranche risk level | Yes (owner only) |
| `allocate_to_tranche(caller, tranche_id, amount)` | Adjust tranche allocation | Yes (owner only) |
| `close_tranche(caller, tranche_id)` | Close a tranche | Yes (owner only) |

## Events

### TrancheCreated
```rust
// Topic: (symbol_short!("TrCreated"), transformation_id, owner)
// Data: (total_value, fee_paid, timestamp)
```
Emitted when a new tranche set is created.

### TrancheUpdated
```rust
// Topic: (symbol_short!("TrUpdated"), tranche_id, caller)
// Data: (new_risk_level, timestamp)
```
Emitted when tranche metadata is updated.

### TrancheAllocated
```rust
// Topic: (symbol_short!("TrAlloc"), tranche_id, caller)
// Data: (amount_change, new_amount, timestamp)
```
Emitted when tranche allocation is adjusted.

### TrancheClosed
```rust
// Topic: (symbol_short!("TrClosed"), tranche_id, caller)
// Data: (timestamp,)
```
Emitted when a tranche is closed.

## Security Considerations

### Authorization

- **Creation**: Requires caller to be authorized transformer or commitment owner
- **Modification**: Requires caller to be the tranche set owner
- **Read operations**: No authorization required

### State Transitions

```
Active ──────────────> Closed
  ↑  ↖                   (terminal)
  │    ↖
  │      ↖ (update/allocate)
  └───────┘
```

- `Active → Active`: Allowed via update/allocate
- `Active → Closed`: Allowed via close_tranche
- `Closed → *`: Not allowed (terminal state)

### Arithmetic Safety

- All allocation calculations use **checked arithmetic**
- Overflow/underflow results in `InvalidAmount` error
- Negative allocations allowed (withdrawals) but cannot result in negative balance

### Reentrancy Protection

All state-changing functions use a reentrancy guard:
- Guard is set before operations
- Guard is cleared after operations
- Nested calls detected and rejected

## Integration Guide

### Creating Tranches

```rust
// Example: Split 1M commitment into 3 tranches
let tranche_share_bps = vec![&env, 6000u32, 3000u32, 1000u32]; // 60%, 30%, 10%
let risk_levels = vec![
    &env,
    String::from_str(&env, "senior"),
    String::from_str(&env, "mezzanine"),
    String::from_str(&env, "equity"),
];

let transformation_id = client.create_tranches(
    &owner,
    &commitment_id,
    &1_000_000i128,
    &tranche_share_bps,
    &risk_levels,
    &fee_asset,
);
```

### Accessing Individual Tranches

```rust
// Get tranche set
let set = client.get_tranche_set(&transformation_id);

// Get first tranche ID
let tranche_id = &set.tranches.get(0).unwrap().tranche_id;

// Get individual tranche
let tranche = client.get_tranche(tranche_id);
```

### Modifying Tranches

```rust
// Update risk level
client.update_tranche(&owner, &tranche_id, &String::from_str(&env, "equity"));

// Increase allocation
client.allocate_to_tranche(&owner, &tranche_id, &100_000i128);

// Decrease allocation (withdrawal)
client.allocate_to_tranche(&owner, &tranche_id, &-50_000i128);

// Close tranche
client.close_tranche(&owner, &tranche_id);
```

## Error Codes

| Error | Code | Description |
|-------|------|-------------|
| `InvalidAmount` | 1 | Amount must be positive or operation would cause negative balance |
| `InvalidTrancheRatios` | 2 | Tranche shares don't sum to 10000 bps |
| `Unauthorized` | 4 | Caller is not owner or authorized |
| `TransformationNotFound` | 8 | Tranche or tranche set does not exist |
| `InvalidState` | 9 | Operation not allowed in current state (e.g., modifying closed tranche) |
| `ReentrancyDetected` | 10 | Reentrant call detected |

## Testing

The implementation includes comprehensive tests covering:

- Tranche creation with various configurations
- Individual tranche retrieval
- Metadata updates
- Allocation adjustments (increase, decrease, zero)
- Tranche closure
- Authorization enforcement
- State transition validation
- Edge cases (overflow, underflow, closed tranche operations)
- Large value handling

Run tests with:
```bash
cargo test -p commitment_transformation
```

## Known Limitations

1. **No pagination**: `get_commitment_tranche_sets` returns all sets for a commitment
2. **No bulk operations**: Each tranche must be updated individually
3. **No reopening**: Closed tranches cannot be reopened (by design)
4. **Owner verification**: Requires lookup of parent tranche set for ownership check

## Future Enhancements

Potential improvements for future iterations:

1. Paginated listing of tranche sets
2. Bulk tranche operations
3. Tranched fee distribution
4. Secondary market support for tranche tokens
5. Automated rebalancing mechanisms
