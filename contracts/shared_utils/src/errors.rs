//! Error handling utilities and common error patterns

use soroban_sdk::{Env, log};

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
