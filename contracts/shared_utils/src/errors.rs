//! Error handling utilities and common error patterns

use soroban_sdk::{log, Env};

/// Error helper functions
pub struct ErrorHelper;

impl ErrorHelper {
    /// Log an error message
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `message` - The error message
    pub fn log_error(e: &Env, message: &str) {
        log!(e, "Error: {}", message);
    }

    /// Log an error with context
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `context` - The error context
    /// * `message` - The error message
    pub fn log_error_with_context(e: &Env, context: &str, message: &str) {
        log!(e, "Error [{}]: {}", context, message);
    }

    /// Panic with a formatted error message
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `message` - The error message
    ///
    /// # Panics
    /// Always panics with the error message
    pub fn panic_with_log(e: &Env, message: &str) -> ! {
        Self::log_error(e, message);
        panic!("{}", message);
    }

    /// Panic with context and formatted error message
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `context` - The error context
    /// * `message` - The error message
    ///
    /// # Panics
    /// Always panics with the formatted error message
    pub fn panic_with_context(e: &Env, context: &str, message: &str) -> ! {
        Self::log_error_with_context(e, context, message);
        panic!("[{}] {}", context, message);
    }

    /// Require a condition to be true, panic otherwise
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `condition` - The condition to check
    /// * `message` - The error message if condition is false
    ///
    /// # Panics
    /// Panics with the error message if condition is false
    pub fn require(e: &Env, condition: bool, message: &str) {
        if !condition {
            Self::panic_with_log(e, message);
        }
    }

    /// Require a condition with context
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `condition` - The condition to check
    /// * `context` - The error context
    /// * `message` - The error message if condition is false
    ///
    /// # Panics
    /// Panics with the formatted error message if condition is false
    pub fn require_with_context(e: &Env, condition: bool, context: &str, message: &str) {
        if !condition {
            Self::panic_with_context(e, context, message);
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::error_codes::{code, message_for_code};

    #[test]
    fn test_require() {
        let env = Env::default();
        ErrorHelper::require(&env, true, "This should not panic");
    }

    #[test]
    #[should_panic(expected = "This should panic")]
    fn test_require_fails() {
        let env = Env::default();
        ErrorHelper::require(&env, false, "This should panic");
    }

    #[test]
    #[should_panic(expected = "Invalid amount: must be greater than zero")]
    fn test_require_fails_with_error_code_message_invalid_amount() {
        let env = Env::default();
        ErrorHelper::require(&env, false, message_for_code(code::INVALID_AMOUNT));
    }

    #[test]
    #[should_panic(expected = "Unauthorized: caller not allowed")]
    fn test_require_fails_with_error_code_message_unauthorized() {
        let env = Env::default();
        ErrorHelper::require(&env, false, message_for_code(code::UNAUTHORIZED));
    }

    #[test]
    #[should_panic(expected = "Value out of allowed range")]
    fn test_panic_with_log_uses_exact_message_for_code() {
        let env = Env::default();
        ErrorHelper::panic_with_log(&env, message_for_code(code::OUT_OF_RANGE));
    }

    #[test]
    #[should_panic(expected = "[validation] Invalid percent: must be between 0 and 100")]
    fn test_require_with_context_formats_expected_panic() {
        let env = Env::default();
        ErrorHelper::require_with_context(
            &env,
            false,
            "validation",
            message_for_code(code::INVALID_PERCENT),
        );
    }
}
