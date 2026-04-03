//! Validation utilities for common input validation patterns

use soroban_sdk::{Address, Env, String};

/// Validation utility functions
///
/// # Summary
/// `Validation` provides deterministic, panic-based input checks used across
/// CommitLabs contracts. These helpers are pure (no storage mutation) and are
/// intended for validating user-supplied inputs before performing state
/// transitions in other contracts.
///
/// # Contract usage
/// - Use these helpers inside contract entrypoints to validate parameters.
/// - They intentionally `panic!` with clear messages so callers (and unit
///   tests) can assert expected failure modes using `#[should_panic]`.
///
/// # Security notes
/// - These helpers do not perform authorization. Any code that mutates
///   storage or performs cross-contract calls must still call `require_auth`
///   or other access-control checks as appropriate.
/// - Arithmetic-sensitive helpers use widened, checked accumulation where
///   appropriate to avoid overflows in edge cases.
pub struct Validation;

impl Validation {
    /// Validate that an amount is greater than zero
    ///
    /// # Arguments
    /// * `amount` - The amount to validate
    ///
    /// # Panics
    /// Panics with "Invalid amount" if amount <= 0
    pub fn require_positive(amount: i128) {
        if amount <= 0 {
            panic!("Invalid amount: must be greater than zero");
        }
    }

    /// Validate that an amount is greater than or equal to zero
    ///
    /// # Arguments
    /// * `amount` - The amount to validate
    ///
    /// # Panics
    /// Panics with "Invalid amount" if amount < 0
    pub fn require_non_negative(amount: i128) {
        if amount < 0 {
            panic!("Invalid amount: must be non-negative");
        }
    }

    /// Validate that a duration is greater than zero
    ///
    /// # Arguments
    /// * `duration_days` - The duration in days
    ///
    /// # Panics
    /// Panics with "Invalid duration" if duration_days == 0
    pub fn require_valid_duration(duration_days: u32) {
        if duration_days == 0 {
            panic!("Invalid duration: must be greater than zero");
        }
    }

    /// Validate that a percentage is between 0 and 100 (inclusive)
    ///
    /// # Params
    /// * `percent` - The percentage value to validate (0..=100)
    ///
    /// # Errors
    /// Panics with `"Invalid percent: must be between 0 and 100"` when
    /// `percent > 100`.
    ///
    /// # Security
    /// This is a pure check and does not mutate state or perform auth.
    pub fn require_valid_percent(percent: u32) {
        if percent > 100 {
            panic!("Invalid percent: must be between 0 and 100");
        }
    }

    /// Validate that a list of percentages sums to exactly 100
    ///
    /// # Params
    /// * `percents` - Slice of individual percentage values (each 0..=100)
    ///
    /// # Errors
    /// - Panics with `"Invalid percent: must be between 0 and 100"` if any
    ///   individual percent is out of range.
    /// - Panics with `"Invalid percent sum: must sum to exactly 100"` if the
    ///   sum of the provided percentages is not exactly 100.
    ///
    /// # Rationale
    /// Percentages are accumulated into a widened `u128` and added using
    /// `checked_add` to guard against pathological overflows. For typical
    /// use-cases (small arrays of percentages) this is conservative and
    /// performant.
    ///
    /// # Security
    /// This helper is pure and deterministic. Use it to validate split
    /// allocations (fees, penalties, tranche weights) before persisting
    /// them to storage or performing financial calculations.
    pub fn require_percent_sum(percents: &[u32]) {
        // Ensure each percent is individually valid and accumulate in a larger type
        let mut sum: u128 = 0;
        for p in percents.iter() {
            if *p > 100 {
                panic!("Invalid percent: must be between 0 and 100");
            }
            sum = sum
                .checked_add(*p as u128)
                .expect("Percent sum overflowed u128 (unexpected)");
        }
        if sum != 100u128 {
            panic!("Invalid percent sum: must sum to exactly 100");
        }
    }

    /// Validate that a string is not empty
    ///
    /// # Arguments
    /// * `value` - The string to validate
    /// * `field_name` - The name of the field (for error message)
    ///
    /// # Panics
    /// Panics if the string is empty
    pub fn require_non_empty_string(value: &String, field_name: &str) {
        if value.is_empty() {
            panic!("Invalid {}: must not be empty", field_name);
        }
    }

    /// Validate that an address is not the zero address
    ///
    /// # Arguments
    /// * `address` - The address to validate
    ///
    /// # Panics
    /// Panics if address is zero
    ///
    /// Note: In Soroban, addresses are always valid, so this is a placeholder
    /// for future validation needs
    pub fn require_non_zero_address(_address: &Address) {
        // In Soroban, addresses are always valid
        // This function is a placeholder for future validation needs
    }

    /// Validate that the provided commitment type is one of the allowed values
    ///
    /// # Params
    /// * `e` - Soroban `Env` used to construct `String` values for comparison
    /// * `commitment_type` - The commitment type as a Soroban `String`
    /// * `allowed_types` - Slice of allowed type string literals (e.g. `"VEST"`)
    ///
    /// # Errors
    /// Panics with `"Invalid commitment type: must be one of the allowed types"`
    /// when `commitment_type` is not equal to any value in `allowed_types`.
    ///
    /// # Notes
    /// This helper does string equality checks using Soroban `String` values.
    /// It is intentionally permissive about the representation (case-sensitive)
    /// and leaves canonicalization (if desired) to callers.
    ///
    /// # Security
    /// This function is a pure validation helper only. Any code that allows
    /// callers to set commitment types in on-chain storage should additionally
    /// validate caller authorization before mutating storage.
    pub fn require_valid_commitment_type(
        e: &Env,
        commitment_type: &String,
        allowed_types: &[&str],
    ) {
        let mut is_valid = false;
        for allowed_type in allowed_types.iter() {
            if *commitment_type == String::from_str(e, allowed_type) {
                is_valid = true;
                break;
            }
        }
        if !is_valid {
            panic!("Invalid commitment type: must be one of the allowed types");
        }
    }

    /// Validate that a value is within a range (inclusive)
    ///
    /// # Arguments
    /// * `value` - The value to validate
    /// * `min` - Minimum allowed value (inclusive)
    /// * `max` - Maximum allowed value (inclusive)
    /// * `field_name` - The name of the field (for error message)
    ///
    /// # Panics
    /// Panics if value is outside the range
    pub fn require_in_range(value: i128, min: i128, max: i128, field_name: &str) {
        if value < min || value > max {
            panic!(
                "Invalid {}: must be between {} and {}",
                field_name, min, max
            );
        }
    }

    /// Validate that a value is greater than or equal to a minimum
    ///
    /// # Arguments
    /// * `value` - The value to validate
    /// * `min` - Minimum allowed value (inclusive)
    /// * `field_name` - The name of the field (for error message)
    ///
    /// # Panics
    /// Panics if value < min
    pub fn require_min(value: i128, min: i128, field_name: &str) {
        if value < min {
            panic!("Invalid {}: must be at least {}", field_name, min);
        }
    }

    /// Validate that a value is less than or equal to a maximum
    ///
    /// # Arguments
    /// * `value` - The value to validate
    /// * `max` - Maximum allowed value (inclusive)
    /// * `field_name` - The name of the field (for error message)
    ///
    /// # Panics
    /// Panics if value > max
    pub fn require_max(value: i128, max: i128, field_name: &str) {
        if value > max {
            panic!("Invalid {}: must be at most {}", field_name, max);
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    #[test]
    fn test_require_positive() {
        Validation::require_positive(1);
        Validation::require_positive(100);
    }

    #[test]
    #[should_panic(expected = "Invalid amount")]
    fn test_require_positive_fails_zero() {
        Validation::require_positive(0);
    }

    #[test]
    #[should_panic(expected = "Invalid amount")]
    fn test_require_positive_fails_negative() {
        Validation::require_positive(-1);
    }

    #[test]
    fn test_require_non_negative() {
        Validation::require_non_negative(0);
        Validation::require_non_negative(1);
        Validation::require_non_negative(100);
    }

    #[test]
    #[should_panic(expected = "Invalid amount")]
    fn test_require_non_negative_fails() {
        Validation::require_non_negative(-1);
    }

    #[test]
    fn test_require_valid_duration() {
        Validation::require_valid_duration(1);
        Validation::require_valid_duration(365);
    }

    #[test]
    #[should_panic(expected = "Invalid duration")]
    fn test_require_valid_duration_fails() {
        Validation::require_valid_duration(0);
    }

    #[test]
    fn test_require_valid_percent() {
        Validation::require_valid_percent(0);
        Validation::require_valid_percent(50);
        Validation::require_valid_percent(100);
    }

    #[test]
    #[should_panic(expected = "Invalid percent")]
    fn test_require_valid_percent_fails() {
        Validation::require_valid_percent(101);
    }

    #[test]
    fn test_require_in_range() {
        Validation::require_in_range(50, 0, 100, "value");
        Validation::require_in_range(0, 0, 100, "value");
        Validation::require_in_range(100, 0, 100, "value");
    }

    #[test]
    #[should_panic(expected = "Invalid value")]
    fn test_require_in_range_fails_below() {
        Validation::require_in_range(-1, 0, 100, "value");
    }

    #[test]
    #[should_panic(expected = "Invalid value")]
    fn test_require_in_range_fails_above() {
        Validation::require_in_range(101, 0, 100, "value");
    }

    #[test]
    fn test_require_valid_commitment_type_ok() {
        let env = Env::default();
        let ct = String::from_str(&env, "TYPE_A");
        Validation::require_valid_commitment_type(&env, &ct, &["TYPE_A", "TYPE_B"]);
    }

    #[test]
    #[should_panic(expected = "Invalid commitment type")]
    fn test_require_valid_commitment_type_fails() {
        let env = Env::default();
        let ct = String::from_str(&env, "TYPE_C");
        Validation::require_valid_commitment_type(&env, &ct, &["TYPE_A", "TYPE_B"]);
    }

    #[test]
    fn test_require_percent_sum_ok() {
        Validation::require_percent_sum(&[100]);
        Validation::require_percent_sum(&[50, 50]);
        Validation::require_percent_sum(&[25, 25, 25, 25]);
    }

    #[test]
    #[should_panic(expected = "Invalid percent sum")]
    fn test_require_percent_sum_fails_sum_mismatch() {
        Validation::require_percent_sum(&[30, 30]);
    }

    #[test]
    #[should_panic(expected = "Invalid percent")]
    fn test_require_percent_sum_fails_individual_invalid() {
        Validation::require_percent_sum(&[50, 200]);
    }
}
