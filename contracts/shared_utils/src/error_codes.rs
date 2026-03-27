//! # Standardized Error Codes and Messages
//!
//! Defines a unified error registry for all CommitLabs contracts.
//! Standardizing codes ensures that off-chain indexers and user interfaces
//! can consistently interpret failures across the ecosystem.
//!
//! ### Error Code Ranges
//! * **Validation (1-99)**: Inputs are malformed or out of logical bounds.
//! * **Authorization (100-199)**: The caller lack necessary permissions.
//! * **State (200-299)**: The contract is in an incompatible state for the request.
//! * **Resource (300-399)**: External resources (e.g., balances, IDs) are missing or insufficient.
//! * **System (400-499)**: Internal failures or unexpected cross-contract errors.

use soroban_sdk::{symbol_short, Env, String as SorobanString};

/// Categorical range boundaries for error classification.
pub mod category {
    /// Start of validation error range.
    pub const VALIDATION_START: u32 = 1;
    /// End of validation error range.
    pub const VALIDATION_END: u32 = 99;
    /// Start of authorization error range.
    pub const AUTH_START: u32 = 100;
    /// End of authorization error range.
    pub const AUTH_END: u32 = 199;
    /// Start of contract state error range.
    pub const STATE_START: u32 = 200;
    /// End of contract state error range.
    pub const STATE_END: u32 = 299;
    /// Start of resource availability error range.
    pub const RESOURCE_START: u32 = 300;
    /// End of resource availability error range.
    pub const RESOURCE_END: u32 = 399;
    /// Start of system-level error range.
    pub const SYSTEM_START: u32 = 400;
    /// End of system-level error range.
    pub const SYSTEM_END: u32 = 499;
}

/// Numeric error code constants.
pub mod code {
    // Validation (1-99)
    /// Provided numerical amount is zero or negative when a positive value is required.
    pub const INVALID_AMOUNT: u32 = 1;
    /// Provided timeframe or duration is logically invalid (e.g., zero).
    pub const INVALID_DURATION: u32 = 2;
    /// Provided percentage is outside the permitted [0, 100] range.
    pub const INVALID_PERCENT: u32 = 3;
    /// Provided enum or type discriminator is unrecognized.
    pub const INVALID_TYPE: u32 = 4;
    /// Parameter value is outside the contextually allowed range.
    pub const OUT_OF_RANGE: u32 = 5;
    /// Required string parameter is empty or purely whitespace.
    pub const EMPTY_STRING: u32 = 6;

    // Authorization (100-199)
    /// General unauthorized access attempt.
    pub const UNAUTHORIZED: u32 = 100;
    /// Action restricted specifically to the resource owner.
    pub const NOT_OWNER: u32 = 101;
    /// Action restricted specifically to the system administrator.
    pub const NOT_ADMIN: u32 = 102;
    /// The calling contract address is not on the allowlist.
    pub const NOT_AUTHORIZED_CONTRACT: u32 = 103;

    // State (200-299)
    /// Attempted to initialize a contract that is already configured.
    pub const ALREADY_INITIALIZED: u32 = 200;
    /// Attempted an action on a contract that has not been initialized.
    pub const NOT_INITIALIZED: u32 = 201;
    /// The contract is in a state where the requested action is logically invalid.
    pub const WRONG_STATE: u32 = 202;
    /// The specific item or ID has already been finalized or processed.
    pub const ALREADY_PROCESSED: u32 = 203;
    /// Prevention of reentrant calls in sensitive functions.
    pub const REENTRANCY: u32 = 204;
    /// The requested entity (e.g., a commitment) is not in an active status.
    pub const NOT_ACTIVE: u32 = 205;

    // Resource (300-399)
    /// The requested entity ID or record does not exist in storage.
    pub const NOT_FOUND: u32 = 300;
    /// The caller lacks sufficient token balance for the operation.
    pub const INSUFFICIENT_BALANCE: u32 = 301;
    /// The specific commitment or escrow lacks sufficient value.
    pub const INSUFFICIENT_VALUE: u32 = 302;
    /// An attempt to transfer Stellar assets failed.
    pub const TRANSFER_FAILED: u32 = 303;

    // System (400-499)
    /// Underlying storage or ledger access error.
    pub const STORAGE_ERROR: u32 = 400;
    /// A synchronous call to another contract returned a failure.
    pub const CONTRACT_CALL_FAILED: u32 = 401;
}

/// Maps a numeric error code to a static human-readable description.
///
/// Designed for use in diagnostic events and off-chain logging.
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

/// Publishes a standardized "Error" event to the ledger.
///
/// Highly recommended to call this immediately before panicking, allowing
/// block explorers and indexers to capture the failure context.
///
/// ### Parameters
/// * `e` - The Soroban environment.
/// * `error_code` - The numeric error code from `code`.
/// * `context` - A string literal or variable identifying the contract and function (e.g., "core::withdraw").
pub fn emit_error_event(e: &Env, error_code: u32, context: &str) {
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
        assert_eq!(
            message_for_code(code::INVALID_AMOUNT),
            "Invalid amount: must be greater than zero"
        );
        assert_eq!(
            message_for_code(code::UNAUTHORIZED),
            "Unauthorized: caller not allowed"
        );
        assert_eq!(message_for_code(code::NOT_FOUND), "Resource not found");
        assert_eq!(message_for_code(999), "Unknown error");
    }

    #[test]
    fn test_emit_error_event() {
        let e = Env::default();
        emit_error_event(&e, code::UNAUTHORIZED, "commitment_core::settle");
    }
}
