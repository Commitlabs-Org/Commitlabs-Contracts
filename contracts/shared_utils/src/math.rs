//! # Safe Math Utilities
//!
//! Provides a collection of arithmetic helpers designed to prevent
//! silent overflow and underflow in financial and state-critical logic.
//!
//! ### Security
//! * All operations use `checked_*` variants and panic on failure.
//! * Percentage calculations are guarded against out-of-bounds inputs.


/// Helper for enforcing safe arithmetic and percentage logic.
pub struct SafeMath;

impl SafeMath {
    /// Safely calculates `a + b`.
    ///
    /// ### Errors
    /// * Panics on addition overflow.
    pub fn add(a: i128, b: i128) -> i128 {
        a.checked_add(b).expect("Math: addition overflow")
    }

    /// Safely calculates `a - b`.
    ///
    /// ### Errors
    /// * Panics on subtraction underflow.
    pub fn sub(a: i128, b: i128) -> i128 {
        a.checked_sub(b).expect("Math: subtraction underflow")
    }

    /// Safely calculates `a * b`.
    ///
    /// ### Errors
    /// * Panics on multiplication overflow.
    pub fn mul(a: i128, b: i128) -> i128 {
        a.checked_mul(b).expect("Math: multiplication overflow")
    }

    /// Safely calculates `a / b`.
    ///
    /// ### Errors
    /// * Panics if `b` is zero.
    /// * Panics on division overflow (e.g., `i128::MIN / -1`).
    pub fn div(a: i128, b: i128) -> i128 {
        if b == 0 {
            panic!("Math: division by zero");
        }
        a.checked_div(b).expect("Math: division overflow")
    }

    /// Calculates a percentage of a value: `(value * percent) / 100`.
    ///
    /// ### Parameters
    /// * `value` - The principal amount.
    /// * `percent` - The rate (0-100).
    ///
    /// ### Returns
    /// * The calculated percentage (integer).
    ///
    /// ### Errors
    /// * Panics if `percent > 100`.
    /// * Panics on multiplication overflow.
    pub fn percent(value: i128, percent: u32) -> i128 {
        if percent > 100 {
            panic!("Math: percent must be <= 100");
        }
        Self::div(Self::mul(value, percent as i128), 100)
    }

    /// Alias for `percent`.
    pub fn percent_of(value: i128, percent: u32) -> i128 {
        Self::percent(value, percent)
    }

    /// Calculates what percentage `part` is of `whole`: `(part * 100) / whole`.
    ///
    /// ### Parameters
    /// * `part` - The portion value.
    /// * `whole` - The total value.
    ///
    /// ### Returns
    /// * The percentage as an integer (0-100, though can exceed 100 if part > whole).
    ///
    /// ### Errors
    /// * Panics if `whole` is zero.
    pub fn percent_from(part: i128, whole: i128) -> i128 {
        if whole == 0 {
            panic!("Math: cannot calculate percent from zero");
        }
        Self::div(Self::mul(part, 100), whole)
    }

    /// Calculates the percentage loss from `initial` to `current`.
    ///
    /// ### Errors
    /// * Panics if `initial` is zero.
    pub fn loss_percent(initial: i128, current: i128) -> i128 {
        if initial == 0 {
            panic!("Math: cannot calculate loss percent from zero initial value");
        }
        let loss = Self::sub(initial, current);
        Self::percent_from(loss, initial)
    }

    /// Calculates the percentage gain from `initial` to `current`.
    ///
    /// ### Errors
    /// * Panics if `initial` is zero.
    pub fn gain_percent(initial: i128, current: i128) -> i128 {
        if initial == 0 {
            panic!("Math: cannot calculate gain percent from zero initial value");
        }
        let gain = Self::sub(current, initial);
        Self::percent_from(gain, initial)
    }

    /// Deducts a percentage penalty from a value.
    ///
    /// ### Parameters
    /// * `value` - Principal amount.
    /// * `penalty_percent` - Rate to deduct (0-100).
    ///
    /// ### Returns
    /// * `value - (value * penalty_percent / 100)`.
    pub fn apply_penalty(value: i128, penalty_percent: u32) -> i128 {
        let penalty_amount = Self::percent(value, penalty_percent);
        Self::sub(value, penalty_amount)
    }

    /// Calculates the penalty amount for a given value and rate.
    pub fn penalty_amount(value: i128, penalty_percent: u32) -> i128 {
        Self::percent(value, penalty_percent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_add() {
        assert_eq!(SafeMath::add(100, 50), 150);
        assert_eq!(SafeMath::add(-100, 50), -50);
    }

    #[test]
    fn test_safe_sub() {
        assert_eq!(SafeMath::sub(100, 50), 50);
        assert_eq!(SafeMath::sub(50, 100), -50);
    }

    #[test]
    fn test_safe_mul() {
        assert_eq!(SafeMath::mul(10, 5), 50);
        assert_eq!(SafeMath::mul(-10, 5), -50);
    }

    #[test]
    fn test_safe_div() {
        assert_eq!(SafeMath::div(100, 5), 20);
        assert_eq!(SafeMath::div(100, -5), -20);
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn test_safe_div_by_zero() {
        SafeMath::div(100, 0);
    }

    #[test]
    fn test_percent() {
        assert_eq!(SafeMath::percent(1000, 10), 100);
        assert_eq!(SafeMath::percent(1000, 50), 500);
        assert_eq!(SafeMath::percent(1000, 100), 1000);
    }

    #[test]
    fn test_percent_from() {
        assert_eq!(SafeMath::percent_from(50, 100), 50);
        assert_eq!(SafeMath::percent_from(25, 100), 25);
        assert_eq!(SafeMath::percent_from(150, 100), 150);
    }

    #[test]
    fn test_loss_percent() {
        assert_eq!(SafeMath::loss_percent(1000, 900), 10);
        assert_eq!(SafeMath::loss_percent(1000, 800), 20);
        assert_eq!(SafeMath::loss_percent(1000, 1000), 0);
    }

    #[test]
    fn test_gain_percent() {
        assert_eq!(SafeMath::gain_percent(1000, 1100), 10);
        assert_eq!(SafeMath::gain_percent(1000, 1200), 20);
        assert_eq!(SafeMath::gain_percent(1000, 1000), 0);
    }

    #[test]
    fn test_apply_penalty() {
        assert_eq!(SafeMath::apply_penalty(1000, 10), 900);
        assert_eq!(SafeMath::apply_penalty(1000, 5), 950);
        assert_eq!(SafeMath::apply_penalty(1000, 0), 1000);
    }

    #[test]
    fn test_penalty_amount() {
        assert_eq!(SafeMath::penalty_amount(1000, 10), 100);
        assert_eq!(SafeMath::penalty_amount(1000, 5), 50);
        assert_eq!(SafeMath::penalty_amount(1000, 0), 0);
    }
}
