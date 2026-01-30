//! Validation utilities for common input validation patterns

use soroban_sdk::{Address, Env, String};

/// Validation utility functions
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

    /// Validate that a percentage is between 0 and 100
    ///
    /// # Arguments
    /// * `percent` - The percentage value
    ///
    /// # Panics
    /// Panics with "Invalid percent" if percent > 100
    pub fn require_valid_percent(percent: u32) {
        if percent > 100 {
            panic!("Invalid percent: must be between 0 and 100");
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

    /// Validate commitment type is one of the allowed values
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `commitment_type` - The commitment type string
    /// * `allowed_types` - Slice of allowed type strings
    ///
    /// # Panics
    /// Panics if commitment_type is not in allowed_types
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
