//! Fee calculation and management utilities
//!
//! Provides a `FeeManager` struct with helpers for computing protocol fees,
//! applying fee rates, splitting amounts between a recipient and the treasury,
//! and validating fee configuration.  All arithmetic uses `SafeMath` so
//! overflows always panic before producing silent bad values.

use crate::math::SafeMath;
use crate::validation::Validation;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum allowed fee rate in basis points (100 % = 10 000 bps).
pub const MAX_FEE_BPS: u32 = 10_000;

/// One hundred percent expressed in basis points.
pub const BPS_DENOMINATOR: u32 = 10_000;

/// Maximum allowed fee expressed as a plain percentage (100 %).
pub const MAX_FEE_PERCENT: u32 = 100;

// ---------------------------------------------------------------------------
// FeeManager
// ---------------------------------------------------------------------------

/// Fee calculation and splitting utilities.
pub struct FeeManager;

impl FeeManager {
    // ── Core calculations ────────────────────────────────────────────────

    /// Calculate the fee for a given amount and fee rate in basis points.
    ///
    /// `fee = amount × fee_bps / 10_000`
    ///
    /// # Arguments
    /// * `amount`  – Gross amount the fee is applied to (must be > 0).
    /// * `fee_bps` – Fee rate in basis points (0 – 10 000 inclusive).
    ///
    /// # Returns
    /// The fee amount, truncated (floor division).
    ///
    /// # Panics
    /// - If `amount <= 0` (via `require_positive`).
    /// - If `fee_bps > 10_000` (via `require_valid_fee_bps`).
    /// - On arithmetic overflow (via `SafeMath`).
    pub fn calculate_fee_bps(amount: i128, fee_bps: u32) -> i128 {
        Validation::require_positive(amount);
        Self::require_valid_fee_bps(fee_bps);

        SafeMath::div(
            SafeMath::mul(amount, fee_bps as i128),
            BPS_DENOMINATOR as i128,
        )
    }

    /// Calculate the fee for a given amount and fee rate as a plain percentage.
    ///
    /// `fee = amount × percent / 100`
    ///
    /// # Arguments
    /// * `amount`  – Gross amount (must be > 0).
    /// * `percent` – Fee percentage (0 – 100 inclusive).
    ///
    /// # Returns
    /// The fee amount, truncated (floor division).
    ///
    /// # Panics
    /// - If `amount <= 0`.
    /// - If `percent > 100`.
    /// - On arithmetic overflow.
    pub fn calculate_fee_percent(amount: i128, percent: u32) -> i128 {
        Validation::require_positive(amount);
        Validation::require_valid_percent(percent);

        SafeMath::percent(amount, percent)
    }

    /// Return the net amount after deducting a basis-point fee.
    ///
    /// `net = amount - fee_bps(amount, fee_bps)`
    ///
    /// # Arguments
    /// * `amount`  – Gross amount (must be > 0).
    /// * `fee_bps` – Fee rate in basis points (0 – 10 000 inclusive).
    ///
    /// # Returns
    /// The net amount after the fee is removed.
    ///
    /// # Panics
    /// Same conditions as [`calculate_fee_bps`].
    pub fn net_amount_bps(amount: i128, fee_bps: u32) -> i128 {
        let fee = Self::calculate_fee_bps(amount, fee_bps);
        SafeMath::sub(amount, fee)
    }

    /// Return the net amount after deducting a percentage fee.
    ///
    /// # Arguments
    /// * `amount`  – Gross amount (must be > 0).
    /// * `percent` – Fee percentage (0 – 100 inclusive).
    ///
    /// # Returns
    /// The net amount after the fee is removed.
    ///
    /// # Panics
    /// Same conditions as [`calculate_fee_percent`].
    pub fn net_amount_percent(amount: i128, percent: u32) -> i128 {
        let fee = Self::calculate_fee_percent(amount, percent);
        SafeMath::sub(amount, fee)
    }

    // ── Splitting ────────────────────────────────────────────────────────

    /// Split an amount into `(net, fee)` using a basis-point rate.
    ///
    /// # Returns
    /// `(net_amount, fee_amount)` where `net + fee == amount` (no dust lost).
    ///
    /// # Panics
    /// Same as [`calculate_fee_bps`].
    pub fn split_bps(amount: i128, fee_bps: u32) -> (i128, i128) {
        let fee = Self::calculate_fee_bps(amount, fee_bps);
        let net = SafeMath::sub(amount, fee);
        (net, fee)
    }

    /// Split an amount into `(net, fee)` using a plain percentage rate.
    ///
    /// # Returns
    /// `(net_amount, fee_amount)` where `net + fee == amount` (no dust lost).
    ///
    /// # Panics
    /// Same as [`calculate_fee_percent`].
    pub fn split_percent(amount: i128, percent: u32) -> (i128, i128) {
        let fee = Self::calculate_fee_percent(amount, percent);
        let net = SafeMath::sub(amount, fee);
        (net, fee)
    }

    // ── Tiered fees ──────────────────────────────────────────────────────

    /// Apply a tiered fee schedule and return the blended fee.
    ///
    /// Each tier is `(threshold, fee_bps)`.  The first tier whose `threshold`
    /// is **≥ amount** wins.  If no tier matches (amount exceeds every threshold)
    /// the last tier's rate is used.
    ///
    /// # Arguments
    /// * `amount` – Gross amount (must be > 0).
    /// * `tiers`  – Ordered slice of `(threshold, fee_bps)` pairs, ascending
    ///              by threshold.
    ///
    /// # Returns
    /// The fee amount for the matched tier.
    ///
    /// # Panics
    /// - If `amount <= 0`.
    /// - If `tiers` is empty.
    /// - If any `fee_bps` in a tier exceeds 10 000.
    pub fn calculate_tiered_fee(amount: i128, tiers: &[(i128, u32)]) -> i128 {
        Validation::require_positive(amount);

        if tiers.is_empty() {
            panic!("Fee tiers must not be empty");
        }

        let mut selected_bps = tiers.last().unwrap().1;

        for (threshold, fee_bps) in tiers.iter() {
            if amount <= *threshold {
                selected_bps = *fee_bps;
                break;
            }
        }

        Self::calculate_fee_bps(amount, selected_bps)
    }

    // ── Compound / multi-hop fees ─────────────────────────────────────────

    /// Calculate the total fee when multiple fee rates are applied in sequence.
    ///
    /// Each rate is applied to the *running net* of the previous step.
    /// Returns the total fee deducted across all steps.
    ///
    /// # Arguments
    /// * `amount`  – Starting gross amount (must be > 0).
    /// * `rates`   – Ordered slice of basis-point rates (each 0 – 10 000).
    ///
    /// # Returns
    /// Total fee removed across all steps.
    ///
    /// # Panics
    /// - If `amount <= 0`.
    /// - If any rate exceeds 10 000.
    pub fn calculate_compound_fee(amount: i128, rates: &[u32]) -> i128 {
        Validation::require_positive(amount);

        let mut running = amount;
        for &rate in rates {
            Self::require_valid_fee_bps(rate);
            let fee = Self::calculate_fee_bps(running, rate);
            running = SafeMath::sub(running, fee);
        }

        SafeMath::sub(amount, running)
    }

    // ── Conversion helpers ────────────────────────────────────────────────

    /// Convert a plain percentage (0 – 100) to basis points (0 – 10 000).
    ///
    /// # Panics
    /// If `percent > 100`.
    pub fn percent_to_bps(percent: u32) -> u32 {
        Validation::require_valid_percent(percent);
        percent * 100
    }

    /// Convert basis points (0 – 10 000) to a plain percentage (0 – 100).
    ///
    /// # Panics
    /// If `bps > 10 000`.
    pub fn bps_to_percent(bps: u32) -> u32 {
        Self::require_valid_fee_bps(bps);
        bps / 100
    }

    // ── Validation helpers ────────────────────────────────────────────────

    /// Panic if `fee_bps > MAX_FEE_BPS`.
    ///
    /// # Panics
    /// `"Invalid fee: basis points must be between 0 and 10000"` if out of range.
    pub fn require_valid_fee_bps(fee_bps: u32) {
        if fee_bps > MAX_FEE_BPS {
            panic!("Invalid fee: basis points must be between 0 and 10000");
        }
    }

    /// Return `true` if `fee_bps` is within the valid range.
    pub fn is_valid_fee_bps(fee_bps: u32) -> bool {
        fee_bps <= MAX_FEE_BPS
    }

    /// Panic if `fee_bps` is zero (fee is mandatory).
    ///
    /// # Panics
    /// `"Invalid fee: must be greater than zero"` when `fee_bps == 0`.
    pub fn require_non_zero_fee(fee_bps: u32) {
        if fee_bps == 0 {
            panic!("Invalid fee: must be greater than zero");
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── require_valid_fee_bps ─────────────────────────────────────────────

    #[test]
    fn test_valid_fee_bps_zero() {
        FeeManager::require_valid_fee_bps(0);
    }

    #[test]
    fn test_valid_fee_bps_max() {
        FeeManager::require_valid_fee_bps(10_000);
    }

    #[test]
    fn test_valid_fee_bps_mid() {
        FeeManager::require_valid_fee_bps(500);
    }

    #[test]
    #[should_panic(expected = "Invalid fee: basis points must be between 0 and 10000")]
    fn test_invalid_fee_bps_over_max() {
        FeeManager::require_valid_fee_bps(10_001);
    }

    // ── is_valid_fee_bps ──────────────────────────────────────────────────

    #[test]
    fn test_is_valid_fee_bps_true() {
        assert!(FeeManager::is_valid_fee_bps(0));
        assert!(FeeManager::is_valid_fee_bps(500));
        assert!(FeeManager::is_valid_fee_bps(10_000));
    }

    #[test]
    fn test_is_valid_fee_bps_false() {
        assert!(!FeeManager::is_valid_fee_bps(10_001));
        assert!(!FeeManager::is_valid_fee_bps(u32::MAX));
    }

    // ── require_non_zero_fee ──────────────────────────────────────────────

    #[test]
    fn test_require_non_zero_fee_passes() {
        FeeManager::require_non_zero_fee(1);
        FeeManager::require_non_zero_fee(500);
        FeeManager::require_non_zero_fee(10_000);
    }

    #[test]
    #[should_panic(expected = "Invalid fee: must be greater than zero")]
    fn test_require_non_zero_fee_fails() {
        FeeManager::require_non_zero_fee(0);
    }

    // ── calculate_fee_bps ─────────────────────────────────────────────────

    #[test]
    fn test_calculate_fee_bps_100_at_10_percent() {
        // 100 × 1000 bps (10 %) = 10
        assert_eq!(FeeManager::calculate_fee_bps(100, 1_000), 10);
    }

    #[test]
    fn test_calculate_fee_bps_1000_at_5_percent() {
        // 1000 × 500 bps (5 %) = 50
        assert_eq!(FeeManager::calculate_fee_bps(1_000, 500), 50);
    }

    #[test]
    fn test_calculate_fee_bps_zero_rate() {
        // zero fee rate → fee is zero
        assert_eq!(FeeManager::calculate_fee_bps(1_000, 0), 0);
    }

    #[test]
    fn test_calculate_fee_bps_full_rate() {
        // 100 % fee rate (10 000 bps) → fee equals amount
        assert_eq!(FeeManager::calculate_fee_bps(1_000, 10_000), 1_000);
    }

    #[test]
    fn test_calculate_fee_bps_truncates() {
        // 1 × 1 bps = 0.0001 → truncated to 0
        assert_eq!(FeeManager::calculate_fee_bps(1, 1), 0);
    }

    #[test]
    fn test_calculate_fee_bps_large_amount() {
        // 1_000_000 × 250 bps (2.5 %) = 25_000
        assert_eq!(FeeManager::calculate_fee_bps(1_000_000, 250), 25_000);
    }

    #[test]
    #[should_panic(expected = "Invalid amount")]
    fn test_calculate_fee_bps_rejects_zero_amount() {
        FeeManager::calculate_fee_bps(0, 500);
    }

    #[test]
    #[should_panic(expected = "Invalid amount")]
    fn test_calculate_fee_bps_rejects_negative_amount() {
        FeeManager::calculate_fee_bps(-1, 500);
    }

    #[test]
    #[should_panic(expected = "Invalid fee")]
    fn test_calculate_fee_bps_rejects_invalid_rate() {
        FeeManager::calculate_fee_bps(1_000, 10_001);
    }

    // ── calculate_fee_percent ─────────────────────────────────────────────

    #[test]
    fn test_calculate_fee_percent_10() {
        assert_eq!(FeeManager::calculate_fee_percent(1_000, 10), 100);
    }

    #[test]
    fn test_calculate_fee_percent_50() {
        assert_eq!(FeeManager::calculate_fee_percent(200, 50), 100);
    }

    #[test]
    fn test_calculate_fee_percent_zero() {
        assert_eq!(FeeManager::calculate_fee_percent(1_000, 0), 0);
    }

    #[test]
    fn test_calculate_fee_percent_100() {
        assert_eq!(FeeManager::calculate_fee_percent(1_000, 100), 1_000);
    }

    #[test]
    #[should_panic(expected = "Invalid amount")]
    fn test_calculate_fee_percent_rejects_zero_amount() {
        FeeManager::calculate_fee_percent(0, 10);
    }

    #[test]
    #[should_panic(expected = "Invalid percent")]
    fn test_calculate_fee_percent_rejects_over_100() {
        FeeManager::calculate_fee_percent(1_000, 101);
    }

    // ── net_amount_bps ────────────────────────────────────────────────────

    #[test]
    fn test_net_amount_bps_10_percent() {
        // 1000 − 10 % (100) = 900
        assert_eq!(FeeManager::net_amount_bps(1_000, 1_000), 900);
    }

    #[test]
    fn test_net_amount_bps_zero_fee() {
        assert_eq!(FeeManager::net_amount_bps(1_000, 0), 1_000);
    }

    #[test]
    fn test_net_amount_bps_full_fee() {
        // 100 % fee → net is 0
        assert_eq!(FeeManager::net_amount_bps(1_000, 10_000), 0);
    }

    // ── net_amount_percent ────────────────────────────────────────────────

    #[test]
    fn test_net_amount_percent_25() {
        assert_eq!(FeeManager::net_amount_percent(1_000, 25), 750);
    }

    #[test]
    fn test_net_amount_percent_zero() {
        assert_eq!(FeeManager::net_amount_percent(500, 0), 500);
    }

    // ── split_bps ─────────────────────────────────────────────────────────

    #[test]
    fn test_split_bps_basic() {
        let (net, fee) = FeeManager::split_bps(1_000, 1_000); // 10 %
        assert_eq!(net, 900);
        assert_eq!(fee, 100);
        assert_eq!(net + fee, 1_000); // no dust
    }

    #[test]
    fn test_split_bps_zero_fee() {
        let (net, fee) = FeeManager::split_bps(500, 0);
        assert_eq!(net, 500);
        assert_eq!(fee, 0);
    }

    #[test]
    fn test_split_bps_full_fee() {
        let (net, fee) = FeeManager::split_bps(300, 10_000);
        assert_eq!(net, 0);
        assert_eq!(fee, 300);
    }

    #[test]
    fn test_split_bps_no_dust() {
        // 10 001 × 333 bps — verify net + fee == amount regardless of rounding
        let amount = 10_001i128;
        let bps = 333u32;
        let (net, fee) = FeeManager::split_bps(amount, bps);
        assert_eq!(net + fee, amount);
    }

    // ── split_percent ─────────────────────────────────────────────────────

    #[test]
    fn test_split_percent_basic() {
        let (net, fee) = FeeManager::split_percent(1_000, 20);
        assert_eq!(net, 800);
        assert_eq!(fee, 200);
        assert_eq!(net + fee, 1_000);
    }

    #[test]
    fn test_split_percent_zero() {
        let (net, fee) = FeeManager::split_percent(1_000, 0);
        assert_eq!(net, 1_000);
        assert_eq!(fee, 0);
    }

    #[test]
    fn test_split_percent_100() {
        let (net, fee) = FeeManager::split_percent(400, 100);
        assert_eq!(net, 0);
        assert_eq!(fee, 400);
    }

    // ── tiered fees ───────────────────────────────────────────────────────

    #[test]
    fn test_tiered_fee_first_tier() {
        let tiers: &[(i128, u32)] = &[(100, 500), (1_000, 300), (10_000, 100)];
        // amount=50 hits first tier → 500 bps (5 %) of 50 = 2
        assert_eq!(FeeManager::calculate_tiered_fee(50, tiers), 2);
    }

    #[test]
    fn test_tiered_fee_second_tier() {
        let tiers: &[(i128, u32)] = &[(100, 500), (1_000, 300), (10_000, 100)];
        // amount=500 hits second tier → 300 bps (3 %) of 500 = 15
        assert_eq!(FeeManager::calculate_tiered_fee(500, tiers), 15);
    }

    #[test]
    fn test_tiered_fee_third_tier() {
        let tiers: &[(i128, u32)] = &[(100, 500), (1_000, 300), (10_000, 100)];
        // amount=5_000 hits third tier → 100 bps (1 %) of 5_000 = 50
        assert_eq!(FeeManager::calculate_tiered_fee(5_000, tiers), 50);
    }

    #[test]
    fn test_tiered_fee_falls_through_to_last() {
        let tiers: &[(i128, u32)] = &[(100, 500), (1_000, 300), (10_000, 100)];
        // amount=50_000 exceeds all thresholds → last tier 100 bps = 500
        assert_eq!(FeeManager::calculate_tiered_fee(50_000, tiers), 500);
    }

    #[test]
    fn test_tiered_fee_exact_threshold_boundary() {
        let tiers: &[(i128, u32)] = &[(1_000, 200), (5_000, 100)];
        // amount == threshold should match that tier
        assert_eq!(FeeManager::calculate_tiered_fee(1_000, tiers), 20);
    }

    #[test]
    fn test_tiered_fee_single_tier() {
        let tiers: &[(i128, u32)] = &[(i128::MAX, 1_000)];
        // Any positive amount → 1000 bps (10 %)
        assert_eq!(FeeManager::calculate_tiered_fee(200, tiers), 20);
    }

    #[test]
    #[should_panic(expected = "Fee tiers must not be empty")]
    fn test_tiered_fee_empty_tiers_panics() {
        FeeManager::calculate_tiered_fee(100, &[]);
    }

    #[test]
    #[should_panic(expected = "Invalid amount")]
    fn test_tiered_fee_zero_amount_panics() {
        let tiers: &[(i128, u32)] = &[(1_000, 100)];
        FeeManager::calculate_tiered_fee(0, tiers);
    }

    // ── compound fees ─────────────────────────────────────────────────────

    #[test]
    fn test_compound_fee_single_rate() {
        // 1000 at 10 % → fee 100, same as simple
        assert_eq!(FeeManager::calculate_compound_fee(1_000, &[1_000]), 100);
    }

    #[test]
    fn test_compound_fee_two_rates() {
        // Step 1: 1000 − 10 % = 900 (fee 100)
        // Step 2: 900  − 10 % = 810 (fee  90)
        // Total fee = 190
        assert_eq!(
            FeeManager::calculate_compound_fee(1_000, &[1_000, 1_000]),
            190
        );
    }

    #[test]
    fn test_compound_fee_empty_rates() {
        // No rates → no fee
        assert_eq!(FeeManager::calculate_compound_fee(1_000, &[]), 0);
    }

    #[test]
    fn test_compound_fee_zero_rates() {
        // Rates of 0 bps → no fee
        assert_eq!(FeeManager::calculate_compound_fee(1_000, &[0, 0, 0]), 0);
    }

    #[test]
    #[should_panic(expected = "Invalid fee")]
    fn test_compound_fee_invalid_rate_panics() {
        FeeManager::calculate_compound_fee(1_000, &[500, 10_001]);
    }

    // ── conversion helpers ────────────────────────────────────────────────

    #[test]
    fn test_percent_to_bps() {
        assert_eq!(FeeManager::percent_to_bps(0), 0);
        assert_eq!(FeeManager::percent_to_bps(1), 100);
        assert_eq!(FeeManager::percent_to_bps(10), 1_000);
        assert_eq!(FeeManager::percent_to_bps(50), 5_000);
        assert_eq!(FeeManager::percent_to_bps(100), 10_000);
    }

    #[test]
    #[should_panic(expected = "Invalid percent")]
    fn test_percent_to_bps_over_100_panics() {
        FeeManager::percent_to_bps(101);
    }

    #[test]
    fn test_bps_to_percent() {
        assert_eq!(FeeManager::bps_to_percent(0), 0);
        assert_eq!(FeeManager::bps_to_percent(100), 1);
        assert_eq!(FeeManager::bps_to_percent(1_000), 10);
        assert_eq!(FeeManager::bps_to_percent(5_000), 50);
        assert_eq!(FeeManager::bps_to_percent(10_000), 100);
    }

    #[test]
    #[should_panic(expected = "Invalid fee")]
    fn test_bps_to_percent_over_max_panics() {
        FeeManager::bps_to_percent(10_001);
    }

    // ── round-trip conversion ─────────────────────────────────────────────

    #[test]
    fn test_percent_bps_round_trip() {
        for p in [0u32, 1, 5, 10, 25, 50, 75, 100] {
            let bps = FeeManager::percent_to_bps(p);
            let back = FeeManager::bps_to_percent(bps);
            assert_eq!(back, p, "round-trip failed for percent={p}");
        }
    }
}