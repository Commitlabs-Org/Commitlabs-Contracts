//! # Fee and Revenue Utilities
//!
//! Provides standardized basis-points (BPS) calculation for protocol fees.
//! Supports multiple fee types including creation, transformation, and early exit fees.
//!
//! ### Arithmetic Safety
//! * Uses `checked_mul` and `checked_div` to prevent overflows during calculation.
//! * Logic enforces a maximum fee of 100% (10,000 BPS).

/// The scale used for fee calculations. 10,000 units represents 100.00%.
pub const BPS_SCALE: u32 = 10000;

/// The functional maximum for basis points.
pub const BPS_MAX: u32 = 10000;

/// Calculates a fee amount based on a base value and rate in basis points.
///
/// ### Parameters
/// * `amount` - The principal value (e.g., transaction volume).
/// * `bps` - The fee rate (0 to 10,000). 1 BPS = 0.01%.
///
/// ### Returns
/// * The calculated fee: `(amount * bps) / 10,000`.
///
/// ### Errors
/// * Panics if `bps` exceeds `BPS_MAX` (10,000).
/// * Panics on arithmetic overflow if `amount * bps` exceeds `i128` limits.
pub fn fee_from_bps(amount: i128, bps: u32) -> i128 {
    if bps > BPS_MAX {
        panic!("Fees: bps must be 0-10000");
    }
    if bps == 0 {
        return 0;
    }
    amount
        .checked_mul(bps as i128)
        .expect("Fees: overflow")
        .checked_div(BPS_SCALE as i128)
        .expect("Fees: div by zero")
}

/// Calculates the remaining value after a fee is deducted.
///
/// ### Parameters
/// * `amount` - The principal value.
/// * `bps` - The fee rate to deduct.
///
/// ### Returns
/// * `amount - fee_amount`.
pub fn net_after_fee_bps(amount: i128, bps: u32) -> i128 {
    let fee = fee_from_bps(amount, bps);
    amount.checked_sub(fee).expect("Fees: underflow")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_from_bps_zero() {
        assert_eq!(fee_from_bps(1000, 0), 0);
    }

    #[test]
    fn test_fee_from_bps_one_percent() {
        assert_eq!(fee_from_bps(10000, 100), 100); // 1%
    }

    #[test]
    fn test_fee_from_bps_ten_percent() {
        assert_eq!(fee_from_bps(1000, 1000), 100); // 10%
    }

    #[test]
    fn test_fee_from_bps_hundred_percent() {
        assert_eq!(fee_from_bps(1000, 10000), 1000);
    }

    #[test]
    fn test_fee_from_bps_rounds_down() {
        assert_eq!(fee_from_bps(100, 15), 0); // 1.5% of 100 = 1.5 -> 1
        assert_eq!(fee_from_bps(1000, 33), 3); // 3.3% rounds down
    }

    #[test]
    fn test_net_after_fee_bps() {
        assert_eq!(net_after_fee_bps(1000, 100), 990); // 1% fee: 1000 - 10 = 990
        assert_eq!(net_after_fee_bps(10000, 50), 9950); // 0.5% fee: 10000 - 50 = 9950
    }

    #[test]
    #[should_panic(expected = "bps must be 0-10000")]
    fn test_fee_from_bps_invalid() {
        fee_from_bps(1000, 10001);
    }
}
