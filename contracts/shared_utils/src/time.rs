//! Time utilities for timestamp and duration calculations

use soroban_sdk::Env;

/// Time utility functions for working with timestamps and durations
pub struct TimeUtils;

impl TimeUtils {
    /// Get the current ledger timestamp
    pub fn now(e: &Env) -> u64 {
        e.ledger().timestamp()
    }

    /// Convert days to seconds
    ///
    /// # Arguments
    /// * `days` - Number of days
    ///
    /// # Returns
    /// Number of seconds
    pub fn days_to_seconds(days: u32) -> u64 {
        days as u64 * 24 * 60 * 60
    }

    /// Convert hours to seconds
    ///
    /// # Arguments
    /// * `hours` - Number of hours
    ///
    /// # Returns
    /// Number of seconds
    pub fn hours_to_seconds(hours: u32) -> u64 {
        hours as u64 * 60 * 60
    }

    /// Convert minutes to seconds
    ///
    /// # Arguments
    /// * `minutes` - Number of minutes
    ///
    /// # Returns
    /// Number of seconds
    pub fn minutes_to_seconds(minutes: u32) -> u64 {
        minutes as u64 * 60
    }

    /// Calculate expiration timestamp from current time and duration in days
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `duration_days` - Duration in days
    ///
    /// # Returns
    /// Expiration timestamp
    pub fn calculate_expiration(e: &Env, duration_days: u32) -> u64 {
        let current_time = Self::now(e);
        let duration_seconds = Self::days_to_seconds(duration_days);
        current_time + duration_seconds
    }

    /// Check if a timestamp has expired (current time >= expiration)
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `expiration` - The expiration timestamp
    ///
    /// # Returns
    /// `true` if expired, `false` otherwise
    pub fn is_expired(e: &Env, expiration: u64) -> bool {
        Self::now(e) >= expiration
    }

    /// Check if a timestamp is still valid (current time < expiration)
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `expiration` - The expiration timestamp
    ///
    /// # Returns
    /// `true` if still valid, `false` if expired
    pub fn is_valid(e: &Env, expiration: u64) -> bool {
        !Self::is_expired(e, expiration)
    }

    /// Calculate time remaining until expiration
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `expiration` - The expiration timestamp
    ///
    /// # Returns
    /// Time remaining in seconds (0 if expired)
    pub fn time_remaining(e: &Env, expiration: u64) -> u64 {
        let current_time = Self::now(e);
        expiration.saturating_sub(current_time)
    }

    /// Calculate elapsed time since a timestamp
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `start_time` - The start timestamp
    ///
    /// # Returns
    /// Elapsed time in seconds
    pub fn elapsed(e: &Env, start_time: u64) -> u64 {
        let current_time = Self::now(e);
        current_time.saturating_sub(start_time)
    }

    /// Convert seconds to days (rounded down)
    ///
    /// # Arguments
    /// * `seconds` - Number of seconds
    ///
    /// # Returns
    /// Number of days
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
}
