//! # Input Validation Utilities
//!
//! Provides reusable assertion helpers for sanitizing contract inputs.
//! Standardizes panic messages for UI/off-chain consistency.
//!
//! ### Best Practices
//! * Use these at the entry point of public contract functions.
//! * Always provide a descriptive `field_name` for contextual errors.


use soroban_sdk::{Address, Env, String};

/// Helper for enforcing business logic constraints on primitive types.
pub struct Validation;

impl Validation {
    /// Asserts that a value is strictly positive (> 0).
    ///
    /// ### Errors
    /// * Panics with "Invalid amount" if `<= 0`.
    pub fn require_positive(amount: i128) {
        if amount <= 0 {
            panic!("Invalid amount: must be greater than zero");
        }
    }

    /// Asserts that a value is zero or greater (>= 0).
    ///
    /// ### Errors
    /// * Panics with "Invalid amount" if `< 0`.
    pub fn require_non_negative(amount: i128) {
        if amount < 0 {
            panic!("Invalid amount: must be non-negative");
        }
    }

    /// Asserts that a day duration is non-zero.
    ///
    /// ### Errors
    /// * Panics with "Invalid duration" if `days == 0`.
    pub fn require_valid_duration(duration_days: u32) {
        if duration_days == 0 {
            panic!("Invalid duration: must be greater than zero");
        }
    }

    /// Asserts that a percentage value is within [0, 100].
    ///
    /// ### Errors
    /// * Panics with "Invalid percent" if `> 100`.
    pub fn require_valid_percent(percent: u32) {
        if percent > 100 {
            panic!("Invalid percent: must be between 0 and 100");
        }
    }

    /// Asserts that a `SorobanString` contains characters.
    ///
    /// ### Parameters
    /// * `value` - The string to check.
    /// * `field_name` - Contextual label for the error message.
    ///
    /// ### Errors
    /// * Panics if empty.
    pub fn require_non_empty_string(value: &String, field_name: &str) {
        if value.is_empty() {
            panic!("Invalid {}: must not be empty", field_name);
        }
    }

    /// Placeholder for future address-specific validation needs.
    ///
    /// ### Note
    /// In current Soroban versions, any `Address` received via call is valid.
    pub fn require_non_zero_address(_address: &Address) {
        // In Soroban, addresses are always valid
    }

    /// Verifies that a string value is part of an allowlist.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `commitment_type` - The string to validate.
    /// * `allowed_types` - Array of valid string literals.
    ///
    /// ### Errors
    /// * Panics if no match is found.
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

    /// Asserts that a value lies within an inclusive range `[min, max]`.
    ///
    /// ### Errors
    /// * Panics if the value is out of bounds.
    pub fn require_in_range(value: i128, min: i128, max: i128, field_name: &str) {
        if value < min || value > max {
            panic!(
                "Invalid {}: must be between {} and {}",
                field_name, min, max
            );
        }
    }

    /// Asserts a lower bound for a value (inclusive).
    pub fn require_min(value: i128, min: i128, field_name: &str) {
        if value < min {
            panic!("Invalid {}: must be at least {}", field_name, min);
        }
    }

    /// Asserts an upper bound for a value (inclusive).
    pub fn require_max(value: i128, max: i128, field_name: &str) {
        if value > max {
            panic!("Invalid {}: must be at most {}", field_name, max);
        }
    }
}

#[cfg(test)]
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
}
