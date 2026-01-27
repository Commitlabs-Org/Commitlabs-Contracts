//! Math utilities for safe arithmetic operations and percentage calculations

use soroban_sdk::Env;

/// Safe math operations to prevent overflow/underflow
pub struct SafeMath;

impl SafeMath {
    /// Safely add two i128 values, panicking on overflow
    pub fn add(a: i128, b: i128) -> i128 {
        a.checked_add(b).expect("Math: addition overflow")
    }

    /// Safely subtract two i128 values, panicking on underflow
    pub fn sub(a: i128, b: i128) -> i128 {
        a.checked_sub(b).expect("Math: subtraction underflow")
    }

    /// Safely multiply two i128 values, panicking on overflow
    pub fn mul(a: i128, b: i128) -> i128 {
        a.checked_mul(b).expect("Math: multiplication overflow")
    }

    /// Safely divide two i128 values, panicking on division by zero
    pub fn div(a: i128, b: i128) -> i128 {
        if b == 0 {
            panic!("Math: division by zero");
        }
        a.checked_div(b).expect("Math: division overflow")
    }

    /// Calculate percentage: (value * percent) / 100
    /// 
    /// # Arguments
    /// * `value` - The base value
    /// * `percent` - The percentage (0-100)
    /// 
    /// # Returns
    /// The calculated percentage value
    pub fn percent(value: i128, percent: u32) -> i128 {
        if percent > 100 {
            panic!("Math: percent must be <= 100");
        }
        Self::div(Self::mul(value, percent as i128), 100)
    }

    /// Calculate percentage of a value: (value * percent) / 100
    /// Returns the percentage amount
    pub fn percent_of(value: i128, percent: u32) -> i128 {
        Self::percent(value, percent)
    }

    /// Calculate what percentage `part` is of `whole`: (part * 100) / whole
    /// 
    /// # Arguments
    /// * `part` - The part value
    /// * `whole` - The whole value
    /// 
    /// # Returns
    /// The percentage (0-100) as i128
    pub fn percent_from(part: i128, whole: i128) -> i128 {
        if whole == 0 {
            panic!("Math: cannot calculate percent from zero");
        }
        Self::div(Self::mul(part, 100), whole)
    }

    /// Calculate loss percentage: ((initial - current) * 100) / initial
    /// 
    /// # Arguments
    /// * `initial` - The initial value
    /// * `current` - The current value
    /// 
    /// # Returns
    /// The loss percentage as i128 (can be negative if current > initial)
    pub fn loss_percent(initial: i128, current: i128) -> i128 {
        if initial == 0 {
            panic!("Math: cannot calculate loss percent from zero initial value");
        }
        let loss = Self::sub(initial, current);
        Self::percent_from(loss, initial)
    }

    /// Calculate gain percentage: ((current - initial) * 100) / initial
    /// 
    /// # Arguments
    /// * `initial` - The initial value
    /// * `current` - The current value
    /// 
    /// # Returns
    /// The gain percentage as i128 (can be negative if current < initial)
    pub fn gain_percent(initial: i128, current: i128) -> i128 {
        if initial == 0 {
            panic!("Math: cannot calculate gain percent from zero initial value");
        }
        let gain = Self::sub(current, initial);
        Self::percent_from(gain, initial)
    }

    /// Apply a percentage penalty: value - (value * penalty_percent / 100)
    /// 
    /// # Arguments
    /// * `value` - The base value
    /// * `penalty_percent` - The penalty percentage (0-100)
    /// 
    /// # Returns
    /// The value after applying the penalty
    pub fn apply_penalty(value: i128, penalty_percent: u32) -> i128 {
        let penalty_amount = Self::percent(value, penalty_percent);
        Self::sub(value, penalty_amount)
    }

    /// Calculate the penalty amount: (value * penalty_percent / 100)
    /// 
    /// # Arguments
    /// * `value` - The base value
    /// * `penalty_percent` - The penalty percentage (0-100)
    /// 
    /// # Returns
    /// The penalty amount
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
