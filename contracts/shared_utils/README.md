# Shared Utilities Library

A shared utility library providing common functions, helpers, and patterns used across all CommitLabs Soroban smart contracts.

## Overview

This library centralizes reusable code to ensure consistency, reduce duplication, and make maintenance easier across all contracts in the CommitLabs protocol.

## Modules

### Math Utilities (`math`)

Safe arithmetic operations and percentage calculations:

- `SafeMath::add/sub/mul/div` - Safe arithmetic with overflow/underflow protection
- `SafeMath::percent` - Calculate percentage of a value
- `SafeMath::percent_from` - Calculate what percentage one value is of another
- `SafeMath::loss_percent` - Calculate loss percentage
- `SafeMath::gain_percent` - Calculate gain percentage
- `SafeMath::apply_penalty` - Apply a percentage penalty
- `SafeMath::penalty_amount` - Calculate penalty amount

### Time Utilities (`time`)

Timestamp and duration calculations:

- `TimeUtils::now` - Get current ledger timestamp
- `TimeUtils::days_to_seconds` - Convert days to seconds
- `TimeUtils::calculate_expiration` - Calculate expiration timestamp
- `TimeUtils::is_expired` - Check if timestamp has expired
- `TimeUtils::time_remaining` - Calculate time remaining until expiration
- `TimeUtils::elapsed` - Calculate elapsed time since a timestamp

### Validation Utilities (`validation`)

Common input validation patterns:

- `Validation::require_positive` - Require amount > 0
- `Validation::require_non_negative` - Require amount >= 0
- `Validation::require_valid_duration` - Require duration > 0
- `Validation::require_valid_percent` - Require percent 0-100
- `Validation::require_non_empty_string` - Require non-empty string
- `Validation::require_valid_commitment_type` - Validate commitment type
- `Validation::require_in_range` - Require value in range
- `Validation::require_min/max` - Require value >= min or <= max

### Storage Helpers (`storage`)

Common storage patterns:

- `Storage::is_initialized` - Check if contract initialized
- `Storage::require_initialized` - Require contract initialized
- `Storage::set_initialized` - Mark contract as initialized
- `Storage::get_admin/set_admin` - Admin address management
- `Storage::get/set/has` - Generic storage operations
- `Storage::get_or_default` - Get with default value

### Error Helpers (`errors`)

Error handling utilities:

- `ErrorHelper::log_error` - Log error message
- `ErrorHelper::panic_with_log` - Panic with logged error
- `ErrorHelper::require` - Require condition with error message

### Access Control (`access_control`)

Access control patterns:

- `AccessControl::require_admin` - Require caller is admin
- `AccessControl::require_owner` - Require caller is owner
- `AccessControl::require_owner_or_admin` - Require caller is owner or admin
- `AccessControl::is_admin` - Check if address is admin

### Event Emission (`events`)

Event emission patterns:

- `Events::emit` - Emit simple event
- `Events::emit_with_topics` - Emit event with multiple topics
- `Events::emit_created` - Emit creation event
- `Events::emit_updated` - Emit update event
- `Events::emit_deleted` - Emit deletion event
- `Events::emit_transfer` - Emit transfer event
- `Events::emit_violation` - Emit violation event

## Usage

Add to your contract's `Cargo.toml`:

```toml
[dependencies]
shared_utils = { path = "../shared_utils" }
```

Import in your contract:

```rust
use shared_utils::{SafeMath, TimeUtils, Validation, Storage, AccessControl, Events};
```

Example usage:

```rust
// Validate input
Validation::require_positive(amount);
Validation::require_valid_duration(duration_days);

// Calculate expiration
let expiration = TimeUtils::calculate_expiration(&e, duration_days);

// Safe math operations
let penalty = SafeMath::penalty_amount(value, penalty_percent);
let returned = SafeMath::sub(value, penalty);

// Access control
AccessControl::require_admin(&e, &caller);

// Emit events
Events::emit_created(&e, &id, &creator, (amount,));
```

## Testing

Run tests:

```bash
cargo test --package shared_utils
```

## Design Principles

1. **Simplicity**: Utilities are focused and do one thing well
2. **Safety**: All operations include overflow/underflow protection
3. **Consistency**: Common patterns are standardized across contracts
4. **Documentation**: All functions are well-documented
5. **Testability**: All utilities have comprehensive tests

## Contributing

When adding new utilities:

1. Keep functions simple and focused
2. Add comprehensive tests
3. Document all functions
4. Follow existing patterns
5. Ensure no_std compatibility
