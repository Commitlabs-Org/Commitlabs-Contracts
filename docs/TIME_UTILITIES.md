# Time Utilities Guide

The `TimeUtils` module in `shared_utils` provides a collection of pure, storage-free helper functions for timestamp and duration arithmetic on the Soroban ledger.

## Trust Boundaries and Security Assumptions

- **Pure logic**: `TimeUtils` methods do not perform storage checks, emit events, or assert user authorization. They only interact with the `Env::ledger().timestamp()`.
- **Validation**: Contract entry points MUST validate the business rules of any incoming timestamps or durations before calling expiration logic.
- **Panic guards**: `TimeUtils` provides helper methods (`require_not_expired`, `require_valid_duration`) designed to be used as guard clauses in contract entry points. They will panic if conditions are not met, safely reverting the transaction.
- **Arithmetic Safety**: Expiration bounds might overflow for durations far into the future near `u64::MAX`. `TimeUtils` provides `checked_calculate_expiration` internally, returning an `Option<u64>` to permit graceful failures when managing user inputs. `require_valid_duration` exists to stop excessively long durations up-front.

## Key Concepts

### Expiration Boundaries
Expirations are evaluated using **inclusive boundaries**. An object is considered strictly expired the exact moment `now() == expiration`.
- **`is_expired(e, exp)`**: Evaluates to `true` when `now >= window`.
- **`is_valid(e, exp)`**: Evaluates to `true` when `now < exp`.

### Durations and Time Conversion
All duration arguments (`days`, `hours`, `minutes`) map exactly to the standard SI defined intervals mapping cleanly into `now()` seconds:
- 1 Minute = 60 Seconds
- 1 Hour = 3600 Seconds
- 1 Day = 86400 Seconds

Maximum durations explicitly default to 73,000 Days (approx 200 years). This ceiling effectively maps safely below the u64 limits against typical runtime timestamps bounds. 

## Common Patterns

### Expiration Guards

Use `require_not_expired` cleanly on business entry points to reject calls if past the required boundary:

```rust
use shared_utils::time::TimeUtils;

pub fn claim(e: Env, deadline: u64) {
    TimeUtils::require_not_expired(&e, deadline);
    // ... logic ...
}
```

### Establishing Bounds
When saving a duration-based expiration state to storage, securely compute its future timestamp:
```rust
pub fn create_order(e: Env, duration_days: u32) {
    TimeUtils::require_valid_duration(duration_days);
    
    // checked equivalent returning Option<u64> instead of wrapping or panicking!
    let expiration = TimeUtils::checked_calculate_expiration(&e, duration_days).unwrap();
    
    // ... save expiration ...
}
```

### Checking Remaining Time
```rust
let remaining_seconds = TimeUtils::time_remaining(&e, expiration);
if remaining_seconds == 0 {
    // Handling completed timers gracefully...
}
```

## API Reference

### Accessing Time
* `now(&e)` -> u64: Returns real-time timestamp.

### Computations
* `calculate_expiration(&e, days)` -> u64: Wrapping logic for exact timer future targeting.
* `checked_calculate_expiration(&e, days)` -> Option<u64>: Overflow-safe equivalent targeting inputs loosely managed by external origins.
* `days_to_seconds(days)` -> u64
* `checked_days_to_seconds(days)` -> Option<u64>

### Logic
* `is_expired(&e, expiration)` -> bool
* `is_valid(&e, expiration)` -> bool
* `time_remaining(&e, expiration)` -> u64 (always >= 0 bounds checked)
* `elapsed(&e, start_time)` -> u64 (always >= 0 bounds checked)
* `is_same_day(ts_a, ts_b)` -> bool
* `is_in_window(ts, start, end)` -> bool

### Guard Condition Wrappers
* `require_not_expired(&e, expiration)`
* `require_valid_duration(duration)`
