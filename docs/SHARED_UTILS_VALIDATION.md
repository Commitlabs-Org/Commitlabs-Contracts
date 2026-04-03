# Shared Utils: Validation Helpers

This document describes the validation helpers provided by the `shared_utils`
crate and how integrators should use them.

## Purpose

`shared_utils::Validation` contains deterministic, panic-based checks used by
CommitLabs contracts to validate user inputs such as amounts, percentages,
commitment types, and durations. They are intentionally simple and designed to
be used as precondition checks inside contract entrypoints.

## Key helpers

- `require_positive(amount: i128)` — panics if `amount <= 0`.
- `require_non_negative(amount: i128)` — panics if `amount < 0`.
- `require_valid_percent(percent: u32)` — panics if `percent > 100`.
- `require_percent_sum(percents: &[u32])` — panics if any individual percent
  is invalid or if the sum of the slice is not exactly `100`.
- `require_valid_commitment_type(e: &Env, commitment_type: &String, allowed_types: &[&str])`
  — panics if `commitment_type` is not equal to one of `allowed_types`.

## Integration guidance

1. Use these helpers as early as possible in an entrypoint to fail fast and
   produce deterministic errors that are easy to assert in unit tests.
2. These helpers do NOT perform authorization checks. Always call `require_auth`
   or other access-control functions prior to mutating storage or performing
   privileged actions.
3. For financial code, always perform validation first, then use `math` helpers
   from the same crate for arithmetic operations to ensure safety.

## Security notes

- The helpers are pure and have no side effects; they only `panic!` on invalid
  inputs.
- `require_percent_sum` uses widened integer accumulation and `checked_add`
  to avoid overflow in malicious scenarios.

## Examples

Validate a fee break-down passed in as percent parts:

```rust
// validate a two-way split
Validation::require_percent_sum(&[fee_part_1, fee_part_2]);

// validate a commitment type passed from an integrator
Validation::require_valid_commitment_type(&env, &commitment_type, &["VEST", "LOCK"]);
```

If you rely on these checks in production flows, include the same checks in any
off-chain code that constructs inputs for your contracts to reduce rejected
transactions and provide a better developer experience.
