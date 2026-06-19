//! Fee utilities for protocol revenue: basis points calculation and fee types.
//!
//! Fee types supported:
//! - Commitment creation fee
//! - Attestation verification fee
//! - Commitment transformation fee
//! - Marketplace fees (if applicable)
//! - Early exit fee (goes to protocol)

/// Basis points scale: 10000 bps = 100%
pub const BPS_SCALE: u32 = 10000;

/// Maximum allowed basis points (100%)
pub const BPS_MAX: u32 = 10000;

/// Fee calculation using basis points.
///
/// # Arguments
/// * `amount` - The base amount (e.g. commitment amount, transformation value)
/// * `bps` - Fee rate in basis points (0-10000). 100 bps = 1%.
///
/// # Returns
/// Fee amount: `(amount * bps) / 10000`. Rounds down.
///
/// # Panics
/// If `bps > 10000`.
pub fn fee_from_bps(amount: i128, bps: u32) -> i128 {
    if bps > BPS_MAX {
        panic!("Fees: bps must be 0-10000");
    }
    if bps == 0 {
        return 0;
    }
    let scale = BPS_SCALE as i128;
    let bps = bps as i128;
    let whole_units = amount / scale;
    let remainder = amount % scale;

    let whole_fee = whole_units.checked_mul(bps).expect("Fees: overflow");
    let remainder_fee = remainder
        .checked_mul(bps)
        .expect("Fees: overflow")
        .checked_div(scale)
        .expect("Fees: div by zero");

    whole_fee
        .checked_add(remainder_fee)
        .expect("Fees: overflow")
}

/// Net amount after deducting a fee in basis points.
///
/// # Returns
/// `amount - fee_from_bps(amount, bps)`.
pub fn net_after_fee_bps(amount: i128, bps: u32) -> i128 {
    let fee = fee_from_bps(amount, bps);
    amount.checked_sub(fee).expect("Fees: underflow")
}

#[cfg(all(test, not(target_family = "wasm")))]
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
    fn test_fee_from_bps_max_amount_boundaries() {
        assert_eq!(fee_from_bps(i128::MAX, 0), 0);
        assert_eq!(fee_from_bps(i128::MAX, 10_000), i128::MAX);

        let fee = fee_from_bps(i128::MAX, 2);
        assert!(fee >= 0);
        assert!(fee <= i128::MAX);
        assert_eq!(
            i128::MAX.checked_sub(fee).unwrap().checked_add(fee),
            Some(i128::MAX)
        );
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
