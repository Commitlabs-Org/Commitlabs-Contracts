# Commitment Interface Error Codes

This document provides a comprehensive reference for all error codes exposed by the `commitment_interface` contract. All errors are aligned with `shared_utils::error_codes` for consistency across CommitLabs contracts.

## Error Code Categories

Errors are organized into categories following the standardized error code system:

| Category | Code Range | Description |
|----------|------------|-------------|
| **Validation** | 1-99 | Invalid inputs, out-of-range values |
| **Authorization** | 100-199 | Unauthorized access, insufficient permissions |
| **State** | 200-299 | Wrong state, already processed |
| **Resource** | 300-399 | Insufficient balance, not found |
| **System** | 400-499 | Storage failures, contract failures |

## Error Code Reference Table

### Validation Errors (1-99)

| Error Name | Code | Description | When It Occurs | Usage Notes |
|------------|------|-------------|----------------|-------------|
| `InvalidAmount` | 1 | Invalid amount: must be greater than zero | `create_commitment()` called with amount ≤ 0 | Ensure amount is positive before calling |
| `InvalidDuration` | 2 | Invalid duration: must be greater than zero | `create_commitment()` with duration_days = 0 | Duration must be at least 1 day |
| `InvalidPercent` | 3 | Invalid percent: must be between 0 and 100 | Rules contain percentage outside 0-100 range | Validate percentages in UI before submission |
| `InvalidType` | 4 | Invalid type: value not allowed | `create_commitment()` with invalid commitment_type | Valid types: "safe", "balanced", "aggressive" |
| `OutOfRange` | 5 | Value out of allowed range | Parameter exceeds maximum or minimum bounds | Check parameter constraints |
| `EmptyString` | 6 | Required field must not be empty | String parameter is empty | Ensure all required strings are populated |

### Authorization Errors (100-199)

| Error Name | Code | Description | When It Occurs | Usage Notes |
|------------|------|-------------|----------------|-------------|
| `Unauthorized` | 100 | Unauthorized: caller not allowed | Caller lacks permission for operation | Verify caller has required role/ownership |
| `NotOwner` | 101 | Caller is not the owner | Non-owner attempts owner-only action | Check `commitment.owner == caller` |
| `NotAdmin` | 102 | Caller is not the admin | Non-admin attempts admin-only action | Admin address from `get_admin()` |
| `NotAuthorizedContract` | 103 | Caller contract not authorized | Unauthorized contract attempts privileged call | Verify contract authorization |

### State Errors (200-299)

| Error Name | Code | Description | When It Occurs | Usage Notes |
|------------|------|-------------|----------------|-------------|
| `AlreadyInitialized` | 200 | Contract already initialized | `initialize()` called on initialized contract | Initialization is single-use only |
| `NotInitialized` | 201 | Contract not initialized | Operation called before initialization | Call `initialize()` first |
| `WrongState` | 202 | Invalid state for this operation | Commitment state doesn't allow operation | Check commitment status before calling |
| `AlreadyProcessed` | 203 | Item already processed | Duplicate processing attempted | Idempotency check failed |
| `ReentrancyDetected` | 204 | Reentrancy detected | Recursive call during execution | Use checks-effects-interactions pattern |
| `NotActive` | 205 | Commitment or item not active | Operation requires active status | Status must be "active" |

### Resource Errors (300-399)

| Error Name | Code | Description | When It Occurs | Usage Notes |
|------------|------|-------------|----------------|-------------|
| `NotFound` | 300 | Resource not found | Requested commitment doesn't exist | Verify commitment_id exists |
| `InsufficientBalance` | 301 | Insufficient balance | Token balance < required amount | Check balance before calling |
| `InsufficientValue` | 302 | Insufficient commitment value | Commitment value too low for operation | Verify commitment.current_value |
| `TransferFailed` | 303 | Token transfer failed | Token transfer returned error | Check token contract compatibility |

### System Errors (400-499)

| Error Name | Code | Description | When It Occurs | Usage Notes |
|------------|------|-------------|----------------|-------------|
| `StorageError` | 400 | Storage operation failed | Storage read/write failed | Internal error; retry may help |
| `ContractCallFailed` | 401 | Cross-contract call failed | External contract call failed | Check downstream contract state |

## Function-Specific Error Mappings

### initialize(admin, nft_contract)

| Possible Errors | Codes |
|-----------------|-------|
| AlreadyInitialized | 200 |
| NotAuthorizedContract | 103 |

### create_commitment(owner, amount, asset_address, rules)

| Possible Errors | Codes |
|-----------------|-------|
| InvalidAmount | 1 |
| InvalidDuration | 2 |
| InvalidPercent | 3 |
| InvalidType | 4 |
| InsufficientBalance | 301 |
| TransferFailed | 303 |
| ContractCallFailed | 401 |

### get_commitment(commitment_id)

| Possible Errors | Codes |
|-----------------|-------|
| NotFound | 300 |

### settle(commitment_id)

| Possible Errors | Codes |
|-----------------|-------|
| NotFound | 300 |
| NotActive | 205 |
| WrongState | 202 |
| ContractCallFailed | 401 |
| TransferFailed | 303 |

### early_exit(commitment_id, caller)

| Possible Errors | Codes |
|-----------------|-------|
| NotFound | 300 |
| Unauthorized | 100 |
| NotActive | 205 |
| ContractCallFailed | 401 |
| TransferFailed | 303 |

## Error Handling Best Practices

### For Integrators

1. **Check Error Codes Programmatically**
   ```typescript
   try {
     await contract.create_commitment(owner, amount, asset, rules);
   } catch (error) {
     if (error.code === 1) {
       // Handle InvalidAmount
       console.error("Amount must be positive");
     } else if (error.code === 301) {
       // Handle InsufficientBalance
       console.error("Insufficient token balance");
     }
   }
   ```

2. **Validate Before Submission**
   - Validate amounts > 0
   - Validate durations > 0
   - Validate percentages in 0-100 range
   - Check token balances before calling

3. **Handle Transient Errors**
   - `StorageError` (400) and `ContractCallFailed` (401) may be transient
   - Implement retry logic with exponential backoff
   - Log error codes for debugging

4. **Use Error Messages for UX**
   - Display human-readable messages to users
   - Include error codes in logs for support
   - Provide actionable guidance based on error type

### Expected Handling Patterns

| Error Category | Handling Pattern |
|----------------|------------------|
| **Validation (1-99)** | Fix input and retry |
| **Authorization (100-199)** | Verify permissions/ownership |
| **State (200-299)** | Check current state before retry |
| **Resource (300-399)** | Ensure sufficient resources |
| **System (400-499)** | Retry with backoff; may need admin intervention |

## Integration Examples

### TypeScript/JavaScript Example

```typescript
import { Contract } from '@stellar/freighter-api';

async function createCommitmentSafely(contract, owner, amount, rules) {
  try {
    const commitmentId = await contract.create_commitment(
      owner,
      amount,
      rules.asset_address,
      rules
    );
    return { success: true, commitmentId };
  } catch (error) {
    const errorCode = error.code;
    
    switch (errorCode) {
      case 1: // InvalidAmount
        throw new Error('Amount must be greater than zero');
      case 2: // InvalidDuration
        throw new Error('Duration must be greater than zero');
      case 3: // InvalidPercent
        throw new Error('Percentage must be between 0 and 100');
      case 301: // InsufficientBalance
        throw new Error('Insufficient token balance');
      case 303: // TransferFailed
        throw new Error('Token transfer failed');
      default:
        throw error;
    }
  }
}
```

### Rust Example

```rust
use commitment_interface::{CommitmentInterfaceClient, Error};

fn handle_commitment_creation(
    client: &CommitmentInterfaceClient,
    owner: Address,
    amount: i128,
    rules: CommitmentRules,
) -> Result<String, String> {
    match client.try_create_commitment(&owner, &amount, &rules.asset_address, &rules) {
        Ok(result) => Ok(result?),
        Err(err) => {
            match err {
                Error::InvalidAmount => Err("Amount must be positive".to_string()),
                Error::InvalidDuration => Err("Duration must be positive".to_string()),
                Error::InsufficientBalance => Err("Insufficient balance".to_string()),
                _ => Err(format!("Unexpected error: {:?}", err)),
            }
        }
    }
}
```

## Monitoring and Debugging

### Error Event Logging

All errors emit events via `shared_utils::emit_error_event`. Monitor these events:

```rust
e.events().publish(
    (symbol_short!("Error"), error_code),
    (context_str, msg_str, e.ledger().timestamp()),
);
```

### Querying Error Events

Indexers can query error events by code:

```typescript
// Query all Unauthorized errors (code 100)
const authErrors = await ledger.getEvents({
  type: "contract",
  contractIds: [CONTRACT_ID],
  topics: [["error", 100]]
});
```

## Version Compatibility

This error code system is stable across interface versions. Changes to error codes require a new interface version bump.

- **Current Interface Version**: 2
- **Error Code Schema**: Stable since v1
- **Breaking Changes**: None planned

## Related Documentation

- [Contract Functions](./CONTRACT_FUNCTIONS.md) - Complete function reference
- [Security Considerations](./SECURITY_CONSIDERATIONS.md) - Security model overview
- [Threat Model](./THREAT_MODEL.md) - Threat analysis and mitigations
