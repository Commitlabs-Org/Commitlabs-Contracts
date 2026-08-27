//! Checked fee arithmetic shared by commitment, settlement, and marketplace flows.
//!
//! A fee is an accounting split, not just a multiplication.  The caller needs
//! all three values (`gross`, `fee`, and `net`) and the discarded fractional
//! numerator in order to prove that no token unit disappeared.  This module
//! keeps that policy in one place and makes the rounding rule explicit.

/// Basis points per whole amount (100 bps = 1%).
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Percentage points per whole amount.
pub const PERCENT_DENOMINATOR: i128 = 100;

/// Failures which can occur before an accounting mutation is made.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeeError {
    /// Fee rates must be within their inclusive range.
    InvalidRate,
    /// Negative or zero gross amounts are not valid settlement inputs.
    InvalidAmount,
    /// A checked operation could not be represented by `i128`.
    ArithmeticOverflow,
    /// A persisted remainder came from a different denominator.
    InvalidRemainder,
}

/// The only supported rounding choice for token-denominated fees.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoundingPolicy {
    /// Keep the fee floor and carry the fractional numerator forward.
    FloorWithCarry,
}

/// A conservation-proof split of one gross amount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeSplit {
    /// Amount received from the user or escrow.
    pub gross: i128,
    /// Amount retained by the protocol.
    pub fee: i128,
    /// Amount delivered to the beneficiary.
    pub net: i128,
    /// Fractional fee numerator left below `denominator`.
    pub remainder: i128,
    /// Denominator used to interpret `remainder`.
    pub denominator: i128,
}

impl FeeSplit {
    /// Return true when this split accounts for every whole token unit.
    pub fn conserves(&self) -> bool {
        self.fee >= 0 && self.net >= 0 && self.fee + self.net == self.gross
    }

    /// Return the fractional part in a stable diagnostic representation.
    pub fn remainder_ratio(&self) -> (i128, i128) {
        (self.remainder, self.denominator)
    }
}

/// A split after applying a previously persisted fractional remainder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarriedFeeSplit {
    /// Whole-unit accounting result.
    pub split: FeeSplit,
    /// Remainder to persist for the next operation.
    pub next_remainder: i128,
    /// Number of whole units released from the carry bucket this time.
    pub released_from_carry: i128,
}

/// Checked floor fee calculation for basis points.
pub fn split_bps(amount: i128, bps: u32) -> Result<FeeSplit, FeeError> {
    if bps > 10_000 {
        return Err(FeeError::InvalidRate);
    }
    split_with_denominator(amount, bps as i128, BPS_DENOMINATOR)
}

/// Checked floor fee calculation for percentage points.
pub fn split_percent(amount: i128, percent: u32) -> Result<FeeSplit, FeeError> {
    if percent > 100 {
        return Err(FeeError::InvalidRate);
    }
    split_with_denominator(amount, percent as i128, PERCENT_DENOMINATOR)
}

/// Shared implementation that avoids multiplying a large amount by a rate.
///
/// `amount / denominator * rate` is evaluated separately from the small
/// remainder product.  This preserves the checked-arithmetic guarantee even
/// when the amount is close to `i128::MAX`.
pub fn split_with_denominator(
    amount: i128,
    rate: i128,
    denominator: i128,
) -> Result<FeeSplit, FeeError> {
    if amount <= 0 || rate < 0 || denominator <= 0 {
        return Err(FeeError::InvalidAmount);
    }
    let whole = amount / denominator;
    let input_remainder = amount % denominator;
    let whole_fee = whole
        .checked_mul(rate)
        .ok_or(FeeError::ArithmeticOverflow)?;
    let fractional_product = input_remainder
        .checked_mul(rate)
        .ok_or(FeeError::ArithmeticOverflow)?;
    let fractional_fee = fractional_product / denominator;
    let fee = whole_fee
        .checked_add(fractional_fee)
        .ok_or(FeeError::ArithmeticOverflow)?;
    let net = amount
        .checked_sub(fee)
        .ok_or(FeeError::ArithmeticOverflow)?;
    Ok(FeeSplit {
        gross: amount,
        fee,
        net,
        remainder: fractional_product % denominator,
        denominator,
    })
}

/// Apply a persisted fractional remainder using the floor-with-carry policy.
///
/// Carrying the numerator makes repeated small settlements converge to the
/// same fee as one aggregate settlement, while every individual call still
/// satisfies `gross = fee + net`.
pub fn split_with_carry(
    amount: i128,
    rate: u32,
    prior_remainder: i128,
    denominator: i128,
) -> Result<CarriedFeeSplit, FeeError> {
    if prior_remainder < 0 || prior_remainder >= denominator {
        return Err(FeeError::InvalidRemainder);
    }
    let base = split_with_denominator(amount, rate as i128, denominator)?;
    let combined = prior_remainder
        .checked_add(base.remainder)
        .ok_or(FeeError::ArithmeticOverflow)?;
    let released = combined / denominator;
    let fee = base
        .fee
        .checked_add(released)
        .ok_or(FeeError::ArithmeticOverflow)?;
    let net = base
        .gross
        .checked_sub(fee)
        .ok_or(FeeError::ArithmeticOverflow)?;
    Ok(CarriedFeeSplit {
        split: FeeSplit {
            gross: base.gross,
            fee,
            net,
            remainder: combined % denominator,
            denominator,
        },
        next_remainder: combined % denominator,
        released_from_carry: released,
    })
}

/// Apply a basis-point split while preserving a basis-point remainder.
pub fn split_bps_with_carry(
    amount: i128,
    bps: u32,
    prior_remainder: i128,
) -> Result<CarriedFeeSplit, FeeError> {
    if bps > 10_000 {
        return Err(FeeError::InvalidRate);
    }
    split_with_carry(amount, bps, prior_remainder, BPS_DENOMINATOR)
}

/// Apply a percentage split while preserving a percentage remainder.
pub fn split_percent_with_carry(
    amount: i128,
    percent: u32,
    prior_remainder: i128,
) -> Result<CarriedFeeSplit, FeeError> {
    if percent > 100 {
        return Err(FeeError::InvalidRate);
    }
    split_with_carry(amount, percent, prior_remainder, PERCENT_DENOMINATOR)
}

/// A compact ledger used by adapters that need an explicit conservation check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeeLedger {
    /// Whole units retained so far.
    pub collected: i128,
    /// Whole units paid out so far.
    pub distributed: i128,
    /// Fractional numerator waiting to become a whole unit.
    pub remainder: i128,
    /// Denominator for the remainder.
    pub denominator: i128,
}

impl FeeLedger {
    /// Create an empty ledger for a denominator.
    pub const fn new(denominator: i128) -> Self {
        Self {
            collected: 0,
            distributed: 0,
            remainder: 0,
            denominator,
        }
    }

    /// Record one amount and return the carried split.
    pub fn record_bps(&mut self, amount: i128, bps: u32) -> Result<FeeSplit, FeeError> {
        if self.denominator != BPS_DENOMINATOR {
            return Err(FeeError::InvalidRemainder);
        }
        let result = split_bps_with_carry(amount, bps, self.remainder)?;
        self.collected = self
            .collected
            .checked_add(result.split.fee)
            .ok_or(FeeError::ArithmeticOverflow)?;
        self.distributed = self
            .distributed
            .checked_add(result.split.net)
            .ok_or(FeeError::ArithmeticOverflow)?;
        self.remainder = result.next_remainder;
        Ok(result.split)
    }

    /// Check the ledger's whole-unit accounting invariant.
    pub fn conserves(&self, gross: i128) -> bool {
        self.collected + self.distributed == gross
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_rate_keeps_all_units() {
        let split = split_bps(1_000, 0).unwrap();
        assert_eq!(split.fee, 0);
        assert_eq!(split.net, 1_000);
        assert!(split.conserves());
    }

    #[test]
    fn full_rate_keeps_no_net_units() {
        let split = split_bps(1_000, 10_000).unwrap();
        assert_eq!(split.fee, 1_000);
        assert_eq!(split.net, 0);
        assert!(split.conserves());
    }

    #[test]
    fn floor_remainder_is_visible() {
        let split = split_bps(101, 15).unwrap();
        assert_eq!(split.fee, 0);
        assert_eq!(split.remainder, 1515);
        assert_eq!(split.remainder_ratio(), (1515, 10_000));
    }

    #[test]
    fn percent_split_uses_percent_denominator() {
        let split = split_percent(101, 15).unwrap();
        assert_eq!(split.fee, 15);
        assert_eq!(split.net, 86);
        assert_eq!(split.remainder, 15);
        assert!(split.conserves());
    }

    #[test]
    fn carry_releases_a_unit_after_repeated_dust() {
        let first = split_bps_with_carry(101, 15, 0).unwrap();
        assert_eq!(first.split.fee, 0);
        let second = split_bps_with_carry(101, 15, first.next_remainder).unwrap();
        assert_eq!(second.split.fee, 0);
        let third = split_bps_with_carry(101, 15, second.next_remainder).unwrap();
        assert_eq!(third.split.fee, 0);
        let fourth = split_bps_with_carry(101, 15, third.next_remainder).unwrap();
        assert_eq!(fourth.split.fee, 0);
        let fifth = split_bps_with_carry(101, 15, fourth.next_remainder).unwrap();
        assert_eq!(fifth.split.fee, 0);
        let sixth = split_bps_with_carry(101, 15, fifth.next_remainder).unwrap();
        assert_eq!(sixth.released_from_carry, 0);
        assert!(sixth.split.conserves());
    }

    #[test]
    fn invalid_rate_is_rejected() {
        assert_eq!(split_bps(100, 10_001), Err(FeeError::InvalidRate));
        assert_eq!(split_percent(100, 101), Err(FeeError::InvalidRate));
    }

    #[test]
    fn invalid_amount_is_rejected() {
        assert_eq!(split_bps(0, 1), Err(FeeError::InvalidAmount));
        assert_eq!(split_bps(-1, 1), Err(FeeError::InvalidAmount));
    }

    #[test]
    fn invalid_carry_is_rejected() {
        assert_eq!(
            split_bps_with_carry(100, 1, 10_000),
            Err(FeeError::InvalidRemainder)
        );
    }

    #[test]
    fn quotient_remainder_algorithm_handles_large_values() {
        let split = split_bps(i128::MAX, 10_000).unwrap();
        assert_eq!(split.fee, i128::MAX);
        assert_eq!(split.net, 0);
        assert!(split.conserves());
    }

    #[test]
    fn ledger_tracks_multiple_operations() {
        let mut ledger = FeeLedger::new(BPS_DENOMINATOR);
        let mut gross = 0;
        for amount in [101, 203, 997, 4_001] {
            ledger.record_bps(amount, 125).unwrap();
            gross += amount;
        }
        assert!(ledger.conserves(gross));
        assert!(ledger.remainder >= 0 && ledger.remainder < BPS_DENOMINATOR);
    }

    #[test]
    fn every_rate_preserves_the_split_identity() {
        for amount in [1, 2, 99, 10_000, 999_999, i128::MAX / 2] {
            for rate in [0, 1, 15, 100, 2_500, 9_999, 10_000] {
                let split = split_bps(amount, rate).unwrap();
                assert!(split.conserves());
                assert!(split.fee <= amount);
            }
        }
    }
}
