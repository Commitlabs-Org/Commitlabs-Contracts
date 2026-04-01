#![cfg(feature = "fuzzing")]

use crate::math::SafeMath;
use crate::validation::Validation;
use proptest::prelude::*;

proptest! {
    /// SafeMath::add property tests
    #[test]
    fn test_proptest_add(a: i128, b: i128) {
        if let Some(expected) = a.checked_add(b) {
            prop_assert_eq!(SafeMath::add(a, b), expected);
        } else {
            // Should panic on overflow
            // We can't easily catch panics in proptest without external helpers
            // but we can test the range that SHOULDN'T panic.
        }
    }

    /// SafeMath::sub property tests
    #[test]
    fn test_proptest_sub(a: i128, b: i128) {
        if let Some(expected) = a.checked_sub(b) {
            prop_assert_eq!(SafeMath::sub(a, b), expected);
        }
    }

    /// SafeMath::mul property tests
    #[test]
    fn test_proptest_mul(a: i128, b: i128) {
        if let Some(expected) = a.checked_mul(b) {
            prop_assert_eq!(SafeMath::mul(a, b), expected);
        }
    }

    /// SafeMath::div property tests
    #[test]
    fn test_proptest_div(a: i128, b: i128) {
        if b != 0 && !(a == i128::MIN && b == -1) {
            if let Some(expected) = a.checked_div(b) {
                prop_assert_eq!(SafeMath::div(a, b), expected);
            }
        }
    }

    /// SafeMath::percent property tests
    #[test]
    fn test_proptest_percent(value: i128, percent in 0u32..=100u32) {
        // value * percent could overflow before division by 100
        if let Some(multiplied) = value.checked_mul(percent as i128) {
            let expected = multiplied / 100;
            prop_assert_eq!(SafeMath::percent(value, percent), expected);
        }
    }

    /// Validation::require_in_range property tests
    #[test]
    fn test_proptest_require_in_range(v: i128, min: i128, max: i128) {
        if min <= max {
            if v >= min && v <= max {
                // Should not panic
                Validation::require_in_range(v, min, max, "test_field");
            }
        }
    }

    /// Validation::require_positive property tests
    #[test]
    fn test_proptest_require_positive(v in 1i128..i128::MAX) {
        Validation::require_positive(v);
    }

    /// Validation::require_non_negative property tests
    #[test]
    fn test_proptest_require_non_negative(v in 0i128..i128::MAX) {
        Validation::require_non_negative(v);
    }
}
