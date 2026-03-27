//! # Error Handling Utilities
//!
//! Provides consistent logging, panicking, and validation helpers for
//! CommitLabs development and production environments.
//!
//! ### Patterns
//! * **Logging**: Non-fatal diagnostic output for the host environment.
//! * **Requirement Checks**: Combined boolean checks and panics with logging.


use soroban_sdk::{log, Env};

/// Helper functions for standardized error logging and assertion.
pub struct ErrorHelper;

impl ErrorHelper {
    /// Logs a basic error message to the Soroban host.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `message` - The text to log.
    pub fn log_error(e: &Env, message: &str) {
        log!(e, "Error: {}", message);
    }

    /// Logs an error message prefixed with a specific context (e.g., contract name).
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `context` - The logic area or function where the error occurred.
    /// * `message` - The detailed error text.
    pub fn log_error_with_context(e: &Env, context: &str, message: &str) {
        log!(e, "Error [{}]: {}", context, message);
    }

    /// Combined logging and termination call.
    ///
    /// ### Errors
    /// * Always panics with the provided message.
    pub fn panic_with_log(e: &Env, message: &str) -> ! {
        Self::log_error(e, message);
        panic!("{}", message);
    }

    /// Combined logging and termination call with contextual prefix.
    ///
    /// ### Errors
    /// * Always panics with "[context] message".
    pub fn panic_with_context(e: &Env, context: &str, message: &str) -> ! {
        Self::log_error_with_context(e, context, message);
        panic!("[{}] {}", context, message);
    }

    /// Asserts that a condition is true; logs and panics otherwise.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `condition` - The boolean state to verify.
    /// * `message` - The error message to use if `condition` is `false`.
    ///
    /// ### Errors
    /// * Panics if `condition` is `false`.
    pub fn require(e: &Env, condition: bool, message: &str) {
        if !condition {
            Self::panic_with_log(e, message);
        }
    }

    /// Asserts that a condition is true; logs and panics with context otherwise.
    ///
    /// ### Parameters
    /// * `condition` - The boolean state to verify.
    /// * `context` - The logic area (e.g., "storage").
    /// * `message` - Detailed failure description.
    pub fn require_with_context(e: &Env, condition: bool, context: &str, message: &str) {
        if !condition {
            Self::panic_with_context(e, context, message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require() {
        let env = Env::default();
        ErrorHelper::require(&env, true, "This should not panic");
    }

    #[test]
    #[should_panic]
    fn test_require_fails() {
        let env = Env::default();
        ErrorHelper::require(&env, false, "This should panic");
    }
}
