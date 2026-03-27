//! # Time and Duration Utilities
//!
//! Standardizes time-based logic across CommitLabs contracts, providing
//! safe conversion between days, hours, and minutes to ledger seconds.
//!
//! ### Arithmetic Safety
//! * Uses `checked_*` arithmetic for expiration and duration calculations.
//! * Includes saturating operations for "time remaining" to avoid underflow panics.


use soroban_sdk::Env;

/// Helper for working with Soroban ledger timestamps and unit conversions.
pub struct TimeUtils;

impl TimeUtils {
    /// Retrieves the current Unix timestamp from the Soroban ledger.
    pub fn now(e: &Env) -> u64 {
        e.ledger().timestamp()
    }

    /// Converts a number of days into equivalent seconds.
    ///
    /// ### Parameters
    /// * `days` - The count of days to convert.
    ///
    /// ### Returns
    /// * `days * 86,400`.
    pub fn days_to_seconds(days: u32) -> u64 {
        days as u64 * 24 * 60 * 60
    }

    /// Safely converts days to seconds, checking for `u64` overflow.
    pub fn checked_days_to_seconds(days: u32) -> Option<u64> {
        (days as u64).checked_mul(24 * 60 * 60)
    }

    /// Calculates a future timestamp based on the current time and a day duration.
    ///
    /// ### Returns
    /// * `Some(now + duration)` or `None` if the addition overflows.
    pub fn checked_calculate_expiration(e: &Env, duration_days: u32) -> Option<u64> {
        let current_time = Self::now(e);
        let duration_seconds = Self::checked_days_to_seconds(duration_days)?;
        current_time.checked_add(duration_seconds)
    }

    /// Converts a number of hours into equivalent seconds.
    pub fn hours_to_seconds(hours: u32) -> u64 {
        hours as u64 * 60 * 60
    }

    /// Converts a number of minutes into equivalent seconds.
    pub fn minutes_to_seconds(minutes: u32) -> u64 {
        minutes as u64 * 60
    }

    /// Calculates a future timestamp. WARNING: Potential for overflow.
    /// Prefer `checked_calculate_expiration` in production.
    pub fn calculate_expiration(e: &Env, duration_days: u32) -> u64 {
        let current_time = Self::now(e);
        let duration_seconds = Self::days_to_seconds(duration_days);
        current_time + duration_seconds
    }

    /// Checks if a timestamp has been reached or passed by the current ledger time.
    ///
    /// ### Returns
    /// * `true` if `now >= expiration`.
    pub fn is_expired(e: &Env, expiration: u64) -> bool {
        Self::now(e) >= expiration
    }

    /// Checks if a timestamp is still in the future.
    ///
    /// ### Returns
    /// * `true` if `now < expiration`.
    pub fn is_valid(e: &Env, expiration: u64) -> bool {
        !Self::is_expired(e, expiration)
    }

    /// Returns the number of seconds until a future timestamp is reached.
    ///
    /// ### Returns
    /// * `expiration - now` or `0` if already expired.
    pub fn time_remaining(e: &Env, expiration: u64) -> u64 {
        let current_time = Self::now(e);
        expiration.saturating_sub(current_time)
    }

    /// Calculates the seconds passed since a given start timestamp.
    ///
    /// ### Returns
    /// * `now - start_time` or `0` if `start_time` is in the future.
    pub fn elapsed(e: &Env, start_time: u64) -> u64 {
        let current_time = Self::now(e);
        current_time.saturating_sub(start_time)
    }

    /// Converts a second count back into days (divides by 86,400).
    pub fn seconds_to_days(seconds: u64) -> u32 {
        (seconds / (24 * 60 * 60)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Ledger;

    #[test]
    fn test_days_to_seconds() {
        assert_eq!(TimeUtils::days_to_seconds(1), 86400);
        assert_eq!(TimeUtils::days_to_seconds(7), 604800);
        assert_eq!(TimeUtils::days_to_seconds(30), 2592000);
    }

    #[test]
    fn test_hours_to_seconds() {
        assert_eq!(TimeUtils::hours_to_seconds(1), 3600);
        assert_eq!(TimeUtils::hours_to_seconds(24), 86400);
    }

    #[test]
    fn test_minutes_to_seconds() {
        assert_eq!(TimeUtils::minutes_to_seconds(1), 60);
        assert_eq!(TimeUtils::minutes_to_seconds(60), 3600);
    }

    #[test]
    fn test_calculate_expiration() {
        let env = Env::default();
        env.ledger().with_mut(|l| {
            l.timestamp = 1000;
        });

        let expiration = TimeUtils::calculate_expiration(&env, 1);
        assert_eq!(expiration, 1000 + 86400);
    }

    #[test]
    fn test_is_expired() {
        let env = Env::default();
        env.ledger().with_mut(|l| {
            l.timestamp = 1000;
        });

        assert!(TimeUtils::is_expired(&env, 500));
        assert!(!TimeUtils::is_expired(&env, 2000));
    }

    #[test]
    fn test_time_remaining() {
        let env = Env::default();
        env.ledger().with_mut(|l| {
            l.timestamp = 1000;
        });

        assert_eq!(TimeUtils::time_remaining(&env, 500), 0);
        assert_eq!(TimeUtils::time_remaining(&env, 2000), 1000);
    }

    #[test]
    fn test_elapsed() {
        let env = Env::default();
        env.ledger().with_mut(|l| {
            l.timestamp = 2000;
        });

        assert_eq!(TimeUtils::elapsed(&env, 1000), 1000);
        assert_eq!(TimeUtils::elapsed(&env, 3000), 0);
    }

    #[test]
    fn test_seconds_to_days() {
        assert_eq!(TimeUtils::seconds_to_days(86400), 1);
        assert_eq!(TimeUtils::seconds_to_days(172800), 2);
        assert_eq!(TimeUtils::seconds_to_days(3600), 0); // Less than a day
    }

    #[test]
    fn test_checked_days_to_seconds() {
        assert_eq!(TimeUtils::checked_days_to_seconds(1), Some(86400));
        assert_eq!(TimeUtils::checked_days_to_seconds(30), Some(2592000));
        // u32::MAX days still fits in u64
        assert!(TimeUtils::checked_days_to_seconds(u32::MAX).is_some());
    }

    #[test]
    fn test_checked_calculate_expiration_overflow() {
        let env = Env::default();
        // Set ledger timestamp near u64::MAX so that adding any meaningful duration overflows
        env.ledger().with_mut(|l| {
            l.timestamp = u64::MAX - 1000;
        });
        // duration_days that would push expires_at past u64::MAX
        let expiration = TimeUtils::checked_calculate_expiration(&env, 1);
        assert_eq!(expiration, None);
    }

    #[test]
    fn test_checked_calculate_expiration_max_allowed() {
        let env = Env::default();
        env.ledger().with_mut(|l| {
            l.timestamp = 1000;
        });
        // Max duration that fits: (u64::MAX - 1000) / 86400
        let max_days = (u64::MAX - 1000) / 86400;
        let duration_days = max_days.min(u32::MAX as u64) as u32;
        let expiration = TimeUtils::checked_calculate_expiration(&env, duration_days);
        assert!(expiration.is_some());
        let exp = expiration.unwrap();
        assert_eq!(exp, 1000 + (duration_days as u64 * 86400));
    }
}
