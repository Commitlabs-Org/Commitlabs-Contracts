//! Standardized error codes and messages for CommitLabs contracts.
//!
//! Error code ranges (for documentation and off-chain indexing):
//! - Validation: 1-99 (invalid input, out of range)
//! - Authorization: 100-199 (unauthorized access, insufficient permissions)
//! - State: 200-299 (wrong state, already processed)
//! - Resource: 300-399 (insufficient balance, not found)
//! - System: 400-499 (storage failures, contract failures)

use soroban_sdk::{Env, symbol_short, String as SorobanString};

/// Error category boundaries for documentation and indexing.
pub mod category {
    pub const VALIDATION_START: u32 = 1;
    pub const VALIDATION_END: u32 = 99;
    pub const AUTH_START: u32 = 100;
    pub const AUTH_END: u32 = 199;
    pub const STATE_START: u32 = 200;
    pub const STATE_END: u32 = 299;
    pub const RESOURCE_START: u32 = 300;
    pub const RESOURCE_END: u32 = 399;
    pub const SYSTEM_START: u32 = 400;
    pub const SYSTEM_END: u32 = 499;
}

/// Standard error code constants (numeric only; contracts use their own contracterror enums).
pub mod code {
    // Validation (1-99)
    pub const INVALID_AMOUNT: u32 = 1;
    pub const INVALID_DURATION: u32 = 2;
    pub const INVALID_PERCENT: u32 = 3;
    pub const INVALID_TYPE: u32 = 4;
    pub const OUT_OF_RANGE: u32 = 5;
    pub const EMPTY_STRING: u32 = 6;

    // Authorization (100-199)
    pub const UNAUTHORIZED: u32 = 100;
    pub const NOT_OWNER: u32 = 101;
    pub const NOT_ADMIN: u32 = 102;
    pub const NOT_AUTHORIZED_CONTRACT: u32 = 103;

    // State (200-299)
    pub const ALREADY_INITIALIZED: u32 = 200;
    pub const NOT_INITIALIZED: u32 = 201;
    pub const WRONG_STATE: u32 = 202;
    pub const ALREADY_PROCESSED: u32 = 203;
    pub const REENTRANCY: u32 = 204;
    pub const NOT_ACTIVE: u32 = 205;

    // Resource (300-399)
    pub const NOT_FOUND: u32 = 300;
    pub const INSUFFICIENT_BALANCE: u32 = 301;
    pub const INSUFFICIENT_VALUE: u32 = 302;
    pub const TRANSFER_FAILED: u32 = 303;

    // System (400-499)
    pub const STORAGE_ERROR: u32 = 400;
    pub const CONTRACT_CALL_FAILED: u32 = 401;
}

/// Returns a human-readable message for a given error code (for events/logging).
pub fn message_for_code(code: u32) -> &'static str {
    match code {
        1 => "Invalid amount: must be greater than zero",
        2 => "Invalid duration: must be greater than zero",
        3 => "Invalid percent: must be between 0 and 100",
        4 => "Invalid type: value not allowed",
        5 => "Value out of allowed range",
        6 => "Required field must not be empty",
        100 => "Unauthorized: caller not allowed",
        101 => "Caller is not the owner",
        102 => "Caller is not the admin",
        103 => "Caller contract not authorized",
        200 => "Contract already initialized",
        201 => "Contract not initialized",
        202 => "Invalid state for this operation",
        203 => "Item already processed",
        204 => "Reentrancy detected",
        205 => "Commitment or item not active",
        300 => "Resource not found",
        301 => "Insufficient balance",
        302 => "Insufficient commitment value",
        303 => "Token transfer failed",
        400 => "Storage operation failed",
        401 => "Cross-contract call failed",
        _ => "Unknown error",
    }
}

/// Emit an error event for off-chain indexing and debugging.
/// Call this before panicking or returning an error so indexers can record it.
pub fn emit_error_event(
    e: &Env,
    error_code: u32,
    context: &str,
) {
    let msg = message_for_code(error_code);
    let context_str = SorobanString::from_str(e, context);
    let msg_str = SorobanString::from_str(e, msg);
    e.events().publish(
        (symbol_short!("Error"), error_code),
        (context_str, msg_str, e.ledger().timestamp()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_for_code() {
        assert_eq!(message_for_code(code::INVALID_AMOUNT), "Invalid amount: must be greater than zero");
        assert_eq!(message_for_code(code::UNAUTHORIZED), "Unauthorized: caller not allowed");
        assert_eq!(message_for_code(code::NOT_FOUND), "Resource not found");
        assert_eq!(message_for_code(999), "Unknown error");
    }

    #[test]
    fn test_emit_error_event() {
        let e = Env::default();
        emit_error_event(&e, code::UNAUTHORIZED, "commitment_core::settle");
    }
}
