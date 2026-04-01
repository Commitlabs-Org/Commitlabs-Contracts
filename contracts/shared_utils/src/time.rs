//! # Time Utilities
//!
//! Provides timestamp and duration helper functions for Soroban smart contracts.
//!
//! All time values are in **seconds** (Soroban ledger timestamps are Unix epoch seconds).
//! Arithmetic that could overflow uses checked variants that return `Option<u64>` — callers
//! should prefer the `checked_*` functions in any context where untrusted input drives
//! `duration_days`.
//!
//! ## Trust Model
//! - All functions are **pure utilities**: they read the ledger timestamp from the `Env` but do
//!   not mutate any storage.
//! - `require_not_expired` and `require_valid_duration` **panic** on invalid input — they are
//!   intended as guard clauses in contract entry points.  Only call them on values that have
//!   already been range-checked by the calling contract's business logic.
//! - No `require_auth` is needed because none of these helpers change contract state.

use soroban_sdk::Env;

/// The maximum number of days a duration may span (≈ 200 years, chosen to keep
/// `duration_days as u64 * 86400` comfortably within u64 without overflow).
pub const MAX_DURATION_DAYS: u32 = 73_000;

/// Seconds in one minute.
pub const SECONDS_PER_MINUTE: u64 = 60;
/// Seconds in one hour.
pub const SECONDS_PER_HOUR: u64 = 3_600;
/// Seconds in one day.
pub const SECONDS_PER_DAY: u64 = 86_400;

/// Time utility functions for working with ledger timestamps and durations.
pub struct TimeUtils;

impl TimeUtils {
    // -------------------------------------------------------------------------
    // Ledger time accessors
    // -------------------------------------------------------------------------

    /// Returns the current ledger timestamp in seconds (Unix epoch).
    ///
    /// # Summary
    /// Thin wrapper around `Env::ledger().timestamp()` provided for consistency and
    /// to avoid pepper-ing raw ledger calls across contract code.
    ///
    /// # Arguments
    /// * `e` — The Soroban environment.
    ///
    /// # Returns
    /// Current ledger timestamp as `u64` seconds.
    pub fn now(e: &Env) -> u64 {
        e.ledger().timestamp()
    }

    // -------------------------------------------------------------------------
    // Unit conversions (infallible — inputs are bounded by the u32 range)
    // -------------------------------------------------------------------------

    /// Converts days to seconds.
    ///
    /// # Summary
    /// Multiplies `days` by 86 400.  The result fits in `u64` for any `u32` input
    /// (`u32::MAX * 86400 = 369_316_166_400` which is well within `u64::MAX`).
    ///
    /// # Arguments
    /// * `days` — Number of days.
    ///
    /// # Returns
    /// Equivalent number of seconds as `u64`.
    pub fn days_to_seconds(days: u32) -> u64 {
        days as u64 * SECONDS_PER_DAY
    }

    /// Converts days to seconds with explicit overflow check.
    ///
    /// # Summary
    /// Identical in result to `days_to_seconds` but returns `None` rather than wrapping
    /// on overflow.  In practice, any `u32` value is safe, so this always returns `Some`
    /// today; the helper exists for code paths that receive `u64` day counts.
    ///
    /// # Arguments
    /// * `days` — Number of days (as `u32`).
    ///
    /// # Returns
    /// `Some(seconds)` or `None` on overflow.
    pub fn checked_days_to_seconds(days: u32) -> Option<u64> {
        (days as u64).checked_mul(SECONDS_PER_DAY)
    }

    /// Converts hours to seconds.
    ///
    /// # Summary
    /// Multiplies `hours` by 3 600.
    ///
    /// # Arguments
    /// * `hours` — Number of hours.
    ///
    /// # Returns
    /// Equivalent number of seconds as `u64`.
    pub fn hours_to_seconds(hours: u32) -> u64 {
        hours as u64 * SECONDS_PER_HOUR
    }

    /// Converts minutes to seconds.
    ///
    /// # Summary
    /// Multiplies `minutes` by 60.
    ///
    /// # Arguments
    /// * `minutes` — Number of minutes.
    ///
    /// # Returns
    /// Equivalent number of seconds as `u64`.
    pub fn minutes_to_seconds(minutes: u32) -> u64 {
        minutes as u64 * SECONDS_PER_MINUTE
    }

    /// Converts seconds back to whole days (floors the result).
    ///
    /// # Summary
    /// Integer division; fractional days are discarded.
    ///
    /// # Arguments
    /// * `seconds` — Number of seconds.
    ///
    /// # Returns
    /// Number of complete days as `u32`.  Truncates to `u32::MAX` if the value
    /// would overflow (only possible for astronomically large `seconds`).
    pub fn seconds_to_days(seconds: u64) -> u32 {
        (seconds / SECONDS_PER_DAY).min(u32::MAX as u64) as u32
    }

    // -------------------------------------------------------------------------
    // Expiration calculation
    // -------------------------------------------------------------------------

    /// Calculates an expiration timestamp from the current ledger time.
    ///
    /// # Summary
    /// Adds `duration_days * 86400` to the current ledger timestamp using **wrapping**
    /// arithmetic.  Prefer `checked_calculate_expiration` when `duration_days` comes
    /// from untrusted or user-supplied input.
    ///
    /// # Arguments
    /// * `e`             — The Soroban environment.
    /// * `duration_days` — Duration in days.
    ///
    /// # Returns
    /// Expiration timestamp in seconds.
    ///
    /// # Security Notes
    /// Uses unchecked addition; callers must ensure `now() + duration_days * 86400 <= u64::MAX`.
    /// For untrusted input use `checked_calculate_expiration` instead.
    pub fn calculate_expiration(e: &Env, duration_days: u32) -> u64 {
        let current_time = Self::now(e);
        let duration_seconds = Self::days_to_seconds(duration_days);
        current_time.wrapping_add(duration_seconds)
    }

    /// Calculates an expiration timestamp using checked arithmetic.
    ///
    /// # Summary
    /// Safe alternative to `calculate_expiration`: returns `None` if the resulting
    /// timestamp would overflow `u64`.  Integrators SHOULD use this function when
    /// `duration_days` is user-supplied or otherwise untrusted.
    ///
    /// # Arguments
    /// * `e`             — The Soroban environment.
    /// * `duration_days` — Duration in days.
    ///
    /// # Returns
    /// `Some(expiration_timestamp)` or `None` on overflow.
    ///
    /// # Errors
    /// Returns `None` if `now() + duration_days * 86400` would exceed `u64::MAX`.
    pub fn checked_calculate_expiration(e: &Env, duration_days: u32) -> Option<u64> {
        let current_time = Self::now(e);
        let duration_seconds = Self::checked_days_to_seconds(duration_days)?;
        current_time.checked_add(duration_seconds)
    }

    // -------------------------------------------------------------------------
    // Expiration queries
    // -------------------------------------------------------------------------

    /// Checks whether a timestamp has expired (current time ≥ expiration).
    ///
    /// # Summary
    /// An expiration is considered **reached** when `now >= expiration`.  This is
    /// an **inclusive** boundary: a timestamp equal to the current ledger time is
    /// treated as already expired.
    ///
    /// # Arguments
    /// * `e`          — The Soroban environment.
    /// * `expiration` — The expiration timestamp (seconds).
    ///
    /// # Returns
    /// `true` if the current time is at or past `expiration`; `false` otherwise.
    pub fn is_expired(e: &Env, expiration: u64) -> bool {
        Self::now(e) >= expiration
    }

    /// Checks whether a timestamp is still valid (current time < expiration).
    ///
    /// # Summary
    /// Logical inverse of `is_expired`.  Expiration is an **exclusive** upper bound:
    /// the timestamp is valid while `now < expiration`.
    ///
    /// # Arguments
    /// * `e`          — The Soroban environment.
    /// * `expiration` — The expiration timestamp (seconds).
    ///
    /// # Returns
    /// `true` if not yet expired; `false` if expired.
    pub fn is_valid(e: &Env, expiration: u64) -> bool {
        !Self::is_expired(e, expiration)
    }

    /// Returns the seconds remaining until expiration (0 if already expired).
    ///
    /// # Summary
    /// Uses saturating subtraction so the result is always ≥ 0 and never wraps.
    ///
    /// # Arguments
    /// * `e`          — The Soroban environment.
    /// * `expiration` — The expiration timestamp (seconds).
    ///
    /// # Returns
    /// Seconds until `expiration`, or 0 if `expiration` is in the past.
    pub fn time_remaining(e: &Env, expiration: u64) -> u64 {
        let current_time = Self::now(e);
        expiration.saturating_sub(current_time)
    }

    /// Returns the time elapsed since `start_time` (0 if start_time is in the future).
    ///
    /// # Summary
    /// Uses saturating subtraction so the result is always ≥ 0.
    ///
    /// # Arguments
    /// * `e`          — The Soroban environment.
    /// * `start_time` — The reference timestamp (seconds).
    ///
    /// # Returns
    /// Seconds elapsed since `start_time`, or 0 if `start_time` is in the future.
    pub fn elapsed(e: &Env, start_time: u64) -> u64 {
        let current_time = Self::now(e);
        current_time.saturating_sub(start_time)
    }

    /// Checks whether two timestamps fall within the same UTC day.
    ///
    /// # Summary
    /// Divides both timestamps by `SECONDS_PER_DAY` and compares the quotients.
    /// Useful for once-per-day rate limiting checks.
    ///
    /// # Arguments
    /// * `ts_a` — First timestamp (seconds).
    /// * `ts_b` — Second timestamp (seconds).
    ///
    /// # Returns
    /// `true` if both timestamps are in the same 86 400-second window from epoch;
    /// `false` otherwise.
    pub fn is_same_day(ts_a: u64, ts_b: u64) -> bool {
        ts_a / SECONDS_PER_DAY == ts_b / SECONDS_PER_DAY
    }

    /// Checks whether a timestamp falls within a half-open window `[start, end)`.
    ///
    /// # Summary
    /// Returns `true` if `start <= ts < end`.  If `start >= end` (invalid window)
    /// the function always returns `false` rather than panicking.
    ///
    /// # Arguments
    /// * `ts`    — The timestamp to test (seconds).
    /// * `start` — Window start, inclusive (seconds).
    /// * `end`   — Window end, exclusive (seconds).
    ///
    /// # Returns
    /// `true` if `ts` is within the window; `false` otherwise.
    pub fn is_in_window(ts: u64, start: u64, end: u64) -> bool {
        if start >= end {
            return false;
        }
        ts >= start && ts < end
    }

    // -------------------------------------------------------------------------
    // Guard / validation helpers (panic on violation)
    // -------------------------------------------------------------------------

    /// Panics if the given expiration has already been reached.
    ///
    /// # Summary
    /// Convenience guard for contract entry points.  Call this early in any function
    /// that should only proceed while a deadline is still active.
    ///
    /// # Arguments
    /// * `e`          — The Soroban environment.
    /// * `expiration` — The expiration timestamp (seconds).
    ///
    /// # Errors
    /// Panics with `"expired"` if `is_expired(e, expiration)` is `true`.
    ///
    /// # Security Notes
    /// Panicking aborts the entire transaction; no storage mutation occurs after this
    /// call.  Safe to use as the first guard in a function body.
    pub fn require_not_expired(e: &Env, expiration: u64) {
        if Self::is_expired(e, expiration) {
            panic!("expired");
        }
    }

    /// Panics if `duration_days` exceeds [`MAX_DURATION_DAYS`].
    ///
    /// # Summary
    /// Bounds-checks user-supplied durations before use in expiration arithmetic.
    /// Prevents contracts from accidentally creating expirations so far in the future
    /// that they are, effectively, permanent.
    ///
    /// # Arguments
    /// * `duration_days` — Duration in days to validate.
    ///
    /// # Errors
    /// Panics with `"duration_exceeds_max"` if `duration_days > MAX_DURATION_DAYS`.
    ///
    /// # Security Notes
    /// Pair with `checked_calculate_expiration` for complete overflow safety.
    pub fn require_valid_duration(duration_days: u32) {
        if duration_days > MAX_DURATION_DAYS {
            panic!("duration_exceeds_max");
        }
    }
}

// =============================================================================
// Inline unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Ledger, Env};

    // -------------------------------------------------------------------------
    // helpers
    // -------------------------------------------------------------------------

    fn env_with_timestamp(ts: u64) -> Env {
        let env = Env::default();
        env.ledger().with_mut(|l| l.timestamp = ts);
        env
    }

    // -------------------------------------------------------------------------
    // Unit conversions
    // -------------------------------------------------------------------------

    #[test]
    fn test_days_to_seconds_typical() {
        assert_eq!(TimeUtils::days_to_seconds(0), 0);
        assert_eq!(TimeUtils::days_to_seconds(1), 86_400);
        assert_eq!(TimeUtils::days_to_seconds(7), 604_800);
        assert_eq!(TimeUtils::days_to_seconds(30), 2_592_000);
        assert_eq!(TimeUtils::days_to_seconds(365), 31_536_000);
    }

    #[test]
    fn test_days_to_seconds_max_u32_no_overflow() {
        // u32::MAX * 86400 must fit in u64
        let result = TimeUtils::days_to_seconds(u32::MAX);
        assert_eq!(result, u32::MAX as u64 * 86_400);
        assert!(result < u64::MAX);
    }

    #[test]
    fn test_hours_to_seconds() {
        assert_eq!(TimeUtils::hours_to_seconds(0), 0);
        assert_eq!(TimeUtils::hours_to_seconds(1), 3_600);
        assert_eq!(TimeUtils::hours_to_seconds(24), 86_400);
        assert_eq!(TimeUtils::hours_to_seconds(u32::MAX), u32::MAX as u64 * 3_600);
    }

    #[test]
    fn test_minutes_to_seconds() {
        assert_eq!(TimeUtils::minutes_to_seconds(0), 0);
        assert_eq!(TimeUtils::minutes_to_seconds(1), 60);
        assert_eq!(TimeUtils::minutes_to_seconds(60), 3_600);
    }

    #[test]
    fn test_seconds_to_days_floored() {
        assert_eq!(TimeUtils::seconds_to_days(0), 0);
        assert_eq!(TimeUtils::seconds_to_days(86_399), 0); // just under 1 day
        assert_eq!(TimeUtils::seconds_to_days(86_400), 1);
        assert_eq!(TimeUtils::seconds_to_days(172_800), 2);
        assert_eq!(TimeUtils::seconds_to_days(3_600), 0); // 1 hour < 1 day
    }

    #[test]
    fn test_seconds_to_days_round_trip() {
        for days in [0u32, 1, 7, 30, 365, 1000] {
            let seconds = TimeUtils::days_to_seconds(days);
            assert_eq!(TimeUtils::seconds_to_days(seconds), days,
                "round-trip failed for {} days", days);
        }
    }

    // -------------------------------------------------------------------------
    // Checked conversions
    // -------------------------------------------------------------------------

    #[test]
    fn test_checked_days_to_seconds_some_for_all_u32() {
        // Any u32 input must yield Some because u32::MAX * 86400 < u64::MAX
        assert_eq!(TimeUtils::checked_days_to_seconds(0), Some(0));
        assert_eq!(TimeUtils::checked_days_to_seconds(1), Some(86_400));
        assert_eq!(TimeUtils::checked_days_to_seconds(30), Some(2_592_000));
        assert!(TimeUtils::checked_days_to_seconds(u32::MAX).is_some());
    }

    // -------------------------------------------------------------------------
    // Expiration calculation — typical cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_calculate_expiration_zero_timestamp() {
        // Ledger at genesis / unset (timestamp = 0)
        let env = env_with_timestamp(0);
        let exp = TimeUtils::calculate_expiration(&env, 1);
        assert_eq!(exp, 86_400);
    }

    #[test]
    fn test_calculate_expiration_typical() {
        let env = env_with_timestamp(1_000_000);
        assert_eq!(TimeUtils::calculate_expiration(&env, 1), 1_000_000 + 86_400);
        assert_eq!(TimeUtils::calculate_expiration(&env, 30), 1_000_000 + 2_592_000);
    }

    #[test]
    fn test_calculate_expiration_zero_duration() {
        // Zero-day duration → expiration == now (immediately expired)
        let env = env_with_timestamp(50_000);
        let exp = TimeUtils::calculate_expiration(&env, 0);
        assert_eq!(exp, 50_000);
        assert!(TimeUtils::is_expired(&env, exp));
    }

    // -------------------------------------------------------------------------
    // Checked expiration — boundary and overflow
    // -------------------------------------------------------------------------

    #[test]
    fn test_checked_calculate_expiration_typical() {
        let env = env_with_timestamp(1_000);
        assert_eq!(
            TimeUtils::checked_calculate_expiration(&env, 7),
            Some(1_000 + 7 * 86_400)
        );
    }

    #[test]
    fn test_checked_calculate_expiration_zero_timestamp() {
        let env = env_with_timestamp(0);
        assert_eq!(
            TimeUtils::checked_calculate_expiration(&env, 1),
            Some(86_400)
        );
    }

    #[test]
    fn test_checked_calculate_expiration_overflow_returns_none() {
        // Ledger near u64::MAX — any non-zero duration must overflow
        let env = env_with_timestamp(u64::MAX - 1_000);
        let result = TimeUtils::checked_calculate_expiration(&env, 1);
        assert_eq!(result, None, "expected None on overflow");
    }

    #[test]
    fn test_checked_calculate_expiration_exact_max_allowed() {
        let env = env_with_timestamp(1_000);
        // Largest duration that still fits:  (u64::MAX - 1000) / 86400
        let max_days_u64 = (u64::MAX - 1_000) / 86_400;
        let duration_days = max_days_u64.min(u32::MAX as u64) as u32;
        let result = TimeUtils::checked_calculate_expiration(&env, duration_days);
        assert!(result.is_some(), "expected Some for max-allowed duration");
        let exp = result.unwrap();
        assert_eq!(exp, 1_000 + duration_days as u64 * 86_400);
    }

    #[test]
    fn test_checked_calculate_expiration_zero_duration() {
        let env = env_with_timestamp(999);
        assert_eq!(TimeUtils::checked_calculate_expiration(&env, 0), Some(999));
    }

    // -------------------------------------------------------------------------
    // is_expired / is_valid — boundary conditions
    // -------------------------------------------------------------------------

    #[test]
    fn test_is_expired_past() {
        let env = env_with_timestamp(1_000);
        assert!(TimeUtils::is_expired(&env, 0));
        assert!(TimeUtils::is_expired(&env, 500));
        assert!(TimeUtils::is_expired(&env, 999));
    }

    #[test]
    fn test_is_expired_exactly_at_now() {
        // expiration == now → expired (inclusive boundary)
        let env = env_with_timestamp(1_000);
        assert!(TimeUtils::is_expired(&env, 1_000));
        assert!(!TimeUtils::is_valid(&env, 1_000));
    }

    #[test]
    fn test_is_expired_one_second_ahead() {
        let env = env_with_timestamp(1_000);
        assert!(!TimeUtils::is_expired(&env, 1_001));
        assert!(TimeUtils::is_valid(&env, 1_001));
    }

    #[test]
    fn test_is_expired_future() {
        let env = env_with_timestamp(1_000);
        assert!(!TimeUtils::is_expired(&env, 2_000));
        assert!(TimeUtils::is_valid(&env, 2_000));
    }

    #[test]
    fn test_is_expired_ledger_at_zero() {
        let env = env_with_timestamp(0);
        // expiration = 0 → expired at genesis
        assert!(TimeUtils::is_expired(&env, 0));
        // any future expiration is still valid
        assert!(!TimeUtils::is_expired(&env, 1));
    }

    #[test]
    fn test_is_expired_max_expiration() {
        let env = env_with_timestamp(1_000);
        assert!(!TimeUtils::is_expired(&env, u64::MAX));
    }

    // -------------------------------------------------------------------------
    // time_remaining — edge cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_time_remaining_not_expired() {
        let env = env_with_timestamp(1_000);
        assert_eq!(TimeUtils::time_remaining(&env, 2_000), 1_000);
    }

    #[test]
    fn test_time_remaining_exactly_at_expiration() {
        let env = env_with_timestamp(1_000);
        // At the expiration boundary, remaining = 0 (not negative)
        assert_eq!(TimeUtils::time_remaining(&env, 1_000), 0);
    }

    #[test]
    fn test_time_remaining_past_expiration_saturates_to_zero() {
        let env = env_with_timestamp(2_000);
        assert_eq!(TimeUtils::time_remaining(&env, 500), 0);
        assert_eq!(TimeUtils::time_remaining(&env, 0), 0);
    }

    #[test]
    fn test_time_remaining_ledger_at_zero() {
        let env = env_with_timestamp(0);
        assert_eq!(TimeUtils::time_remaining(&env, 86_400), 86_400);
        assert_eq!(TimeUtils::time_remaining(&env, 0), 0);
    }

    #[test]
    fn test_time_remaining_max_expiration() {
        let env = env_with_timestamp(0);
        assert_eq!(TimeUtils::time_remaining(&env, u64::MAX), u64::MAX);
    }

    // -------------------------------------------------------------------------
    // elapsed — edge cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_elapsed_normal() {
        let env = env_with_timestamp(2_000);
        assert_eq!(TimeUtils::elapsed(&env, 1_000), 1_000);
    }

    #[test]
    fn test_elapsed_same_time() {
        let env = env_with_timestamp(1_000);
        assert_eq!(TimeUtils::elapsed(&env, 1_000), 0);
    }

    #[test]
    fn test_elapsed_future_start_saturates_to_zero() {
        // start_time in the future → saturating_sub → 0
        let env = env_with_timestamp(1_000);
        assert_eq!(TimeUtils::elapsed(&env, 3_000), 0);
    }

    #[test]
    fn test_elapsed_ledger_at_zero() {
        let env = env_with_timestamp(0);
        assert_eq!(TimeUtils::elapsed(&env, 0), 0);
        // start_time in the future
        assert_eq!(TimeUtils::elapsed(&env, 86_400), 0);
    }

    // -------------------------------------------------------------------------
    // is_same_day
    // -------------------------------------------------------------------------

    #[test]
    fn test_is_same_day_same_second() {
        assert!(TimeUtils::is_same_day(86_400, 86_400));
    }

    #[test]
    fn test_is_same_day_same_day_different_seconds() {
        // Both within the range [86400, 172799]
        assert!(TimeUtils::is_same_day(86_400, 172_799));
    }

    #[test]
    fn test_is_same_day_across_day_boundary() {
        assert!(!TimeUtils::is_same_day(86_399, 86_400));
    }

    #[test]
    fn test_is_same_day_zero_timestamps() {
        assert!(TimeUtils::is_same_day(0, 0));
        assert!(TimeUtils::is_same_day(0, 86_399));
        assert!(!TimeUtils::is_same_day(0, 86_400));
    }

    // -------------------------------------------------------------------------
    // is_in_window
    // -------------------------------------------------------------------------

    #[test]
    fn test_is_in_window_typical() {
        assert!(TimeUtils::is_in_window(150, 100, 200));
    }

    #[test]
    fn test_is_in_window_at_start_inclusive() {
        assert!(TimeUtils::is_in_window(100, 100, 200));
    }

    #[test]
    fn test_is_in_window_at_end_exclusive() {
        assert!(!TimeUtils::is_in_window(200, 100, 200));
    }

    #[test]
    fn test_is_in_window_below_start() {
        assert!(!TimeUtils::is_in_window(99, 100, 200));
    }

    #[test]
    fn test_is_in_window_above_end() {
        assert!(!TimeUtils::is_in_window(201, 100, 200));
    }

    #[test]
    fn test_is_in_window_invalid_range_returns_false() {
        // start >= end → always false, even if ts would otherwise match
        assert!(!TimeUtils::is_in_window(150, 200, 100));
        assert!(!TimeUtils::is_in_window(150, 150, 150));
    }

    #[test]
    fn test_is_in_window_zero_bounds() {
        assert!(!TimeUtils::is_in_window(0, 0, 0)); // zero-width window
        assert!(TimeUtils::is_in_window(0, 0, 1));
    }

    // -------------------------------------------------------------------------
    // require_not_expired (panic guard)
    // -------------------------------------------------------------------------

    #[test]
    fn test_require_not_expired_succeeds_when_valid() {
        let env = env_with_timestamp(1_000);
        // Must not panic
        TimeUtils::require_not_expired(&env, 1_001);
        TimeUtils::require_not_expired(&env, u64::MAX);
    }

    #[test]
    #[should_panic(expected = "expired")]
    fn test_require_not_expired_panics_at_exact_expiration() {
        let env = env_with_timestamp(1_000);
        TimeUtils::require_not_expired(&env, 1_000); // == now → expired
    }

    #[test]
    #[should_panic(expected = "expired")]
    fn test_require_not_expired_panics_past_expiration() {
        let env = env_with_timestamp(2_000);
        TimeUtils::require_not_expired(&env, 1_000);
    }

    #[test]
    #[should_panic(expected = "expired")]
    fn test_require_not_expired_panics_at_zero_expiration_nonzero_now() {
        let env = env_with_timestamp(1);
        TimeUtils::require_not_expired(&env, 0);
    }

    #[test]
    #[should_panic(expected = "expired")]
    fn test_require_not_expired_panics_at_zero_zero() {
        // now == expiration == 0
        let env = env_with_timestamp(0);
        TimeUtils::require_not_expired(&env, 0);
    }

    // -------------------------------------------------------------------------
    // require_valid_duration (panic guard)
    // -------------------------------------------------------------------------

    #[test]
    fn test_require_valid_duration_accepts_zero() {
        TimeUtils::require_valid_duration(0); // must not panic
    }

    #[test]
    fn test_require_valid_duration_accepts_max_allowed() {
        TimeUtils::require_valid_duration(MAX_DURATION_DAYS); // exactly at limit
    }

    #[test]
    #[should_panic(expected = "duration_exceeds_max")]
    fn test_require_valid_duration_panics_above_max() {
        TimeUtils::require_valid_duration(MAX_DURATION_DAYS + 1);
    }

    #[test]
    #[should_panic(expected = "duration_exceeds_max")]
    fn test_require_valid_duration_panics_at_u32_max() {
        TimeUtils::require_valid_duration(u32::MAX);
    }

    // -------------------------------------------------------------------------
    // Ledger progression integration
    // -------------------------------------------------------------------------

    #[test]
    fn test_expiration_transitions_with_ledger_advance() {
        let env = Env::default();

        env.ledger().with_mut(|l| l.timestamp = 10_000);
        let expiration = TimeUtils::calculate_expiration(&env, 2); // 2 days forward

        // Before expiration
        assert!(TimeUtils::is_valid(&env, expiration));
        assert_eq!(TimeUtils::time_remaining(&env, expiration), TimeUtils::days_to_seconds(2));

        // Advance to one second before expiration
        env.ledger().with_mut(|l| l.timestamp = expiration - 1);
        assert!(TimeUtils::is_valid(&env, expiration));
        assert_eq!(TimeUtils::time_remaining(&env, expiration), 1);

        // Advance to exact expiration (inclusive boundary → expired)
        env.ledger().with_mut(|l| l.timestamp = expiration);
        assert!(TimeUtils::is_expired(&env, expiration));
        assert_eq!(TimeUtils::time_remaining(&env, expiration), 0);

        // Advance past expiration
        env.ledger().with_mut(|l| l.timestamp = expiration + 1_000);
        assert!(TimeUtils::is_expired(&env, expiration));
        assert_eq!(TimeUtils::elapsed(&env, expiration), 1_000);
    }

    #[test]
    fn test_zero_ledger_timestamp_all_helpers_coherent() {
        // Regression: ensure no helpers panic or behave unexpectedly when ledger
        // is at genesis (timestamp == 0).
        let env = env_with_timestamp(0);

        assert_eq!(TimeUtils::now(&env), 0);

        let exp = TimeUtils::calculate_expiration(&env, 1);
        assert_eq!(exp, 86_400);
        assert!(!TimeUtils::is_expired(&env, exp));
        assert!(TimeUtils::is_valid(&env, exp));
        assert_eq!(TimeUtils::time_remaining(&env, exp), 86_400);
        assert_eq!(TimeUtils::elapsed(&env, 0), 0);

        // Zero-duration from zero timestamp
        let exp0 = TimeUtils::calculate_expiration(&env, 0);
        assert_eq!(exp0, 0);
        assert!(TimeUtils::is_expired(&env, exp0)); // now == expiration → expired
    }
}
