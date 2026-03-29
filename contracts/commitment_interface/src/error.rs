//! Standardized error codes for the commitment interface.
//!
//! # Error Code Mapping to shared_utils::error_codes
//!
//! This module aligns with `shared_utils::error_codes` categories for consistency across
//! CommitLabs contracts. Each error variant maps to a standardized code from the shared utilities.
//!
//! ## Mapping Reference:
//!
//! | Interface Error | Code | Shared Utils Code | Category |
//! |----------------|------|-------------------|----------|
//! | InvalidAmount | 1 | code::INVALID_AMOUNT (1) | Validation |
//! | InvalidDuration | 2 | code::INVALID_DURATION (2) | Validation |
//! | InvalidPercent | 3 | code::INVALID_PERCENT (3) | Validation |
//! | InvalidType | 4 | code::INVALID_TYPE (4) | Validation |
//! | OutOfRange | 5 | code::OUT_OF_RANGE (5) | Validation |
//! | EmptyString | 6 | code::EMPTY_STRING (6) | Validation |
//! | Unauthorized | 7 | code::UNAUTHORIZED (100) | Authorization |
//! | NotOwner | 8 | code::NOT_OWNER (101) | Authorization |
//! | NotAdmin | 9 | code::NOT_ADMIN (102) | Authorization |
//! | NotAuthorizedContract | 10 | code::NOT_AUTHORIZED_CONTRACT (103) | Authorization |
//! | AlreadyInitialized | 11 | code::ALREADY_INITIALIZED (200) | State |
//! | NotInitialized | 12 | code::NOT_INITIALIZED (201) | State |
//! | WrongState | 13 | code::WRONG_STATE (202) | State |
//! | AlreadyProcessed | 14 | code::ALREADY_PROCESSED (203) | State |
//! | ReentrancyDetected | 15 | code::REENTRANCY (204) | State |
//! | NotActive | 16 | code::NOT_ACTIVE (205) | State |
//! | NotFound | 17 | code::NOT_FOUND (300) | Resource |
//! | InsufficientBalance | 18 | code::INSUFFICIENT_BALANCE (301) | Resource |
//! | InsufficientValue | 19 | code::INSUFFICIENT_VALUE (302) | Resource |
//! | TransferFailed | 20 | code::TRANSFER_FAILED (303) | Resource |
//! | StorageError | 21 | code::STORAGE_ERROR (400) | System |
//! | ContractCallFailed | 22 | code::CONTRACT_CALL_FAILED (401) | System |

use soroban_sdk::contracterror;

/// Error code categories aligned with shared_utils::error_codes::category
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

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // Validation Errors (1-99) - aligned with shared_utils::error_codes::code
    /// Invalid amount: must be greater than zero (maps to code::INVALID_AMOUNT = 1)
    InvalidAmount = 1,
    /// Invalid duration: must be greater than zero (maps to code::INVALID_DURATION = 2)
    InvalidDuration = 2,
    /// Invalid percent: must be between 0 and 100 (maps to code::INVALID_PERCENT = 3)
    InvalidPercent = 3,
    /// Invalid type: value not allowed (maps to code::INVALID_TYPE = 4)
    InvalidType = 4,
    /// Value out of allowed range (maps to code::OUT_OF_RANGE = 5)
    OutOfRange = 5,
    /// Required field must not be empty (maps to code::EMPTY_STRING = 6)
    EmptyString = 6,

    // Authorization Errors (100-199) - aligned with shared_utils::error_codes::code
    /// Unauthorized: caller not allowed (maps to code::UNAUTHORIZED = 100)
    Unauthorized = 100,
    /// Caller is not the owner (maps to code::NOT_OWNER = 101)
    NotOwner = 101,
    /// Caller is not the admin (maps to code::NOT_ADMIN = 102)
    NotAdmin = 102,
    /// Caller contract not authorized (maps to code::NOT_AUTHORIZED_CONTRACT = 103)
    NotAuthorizedContract = 103,

    // State Errors (200-299) - aligned with shared_utils::error_codes::code
    /// Contract already initialized (maps to code::ALREADY_INITIALIZED = 200)
    AlreadyInitialized = 200,
    /// Contract not initialized (maps to code::NOT_INITIALIZED = 201)
    NotInitialized = 201,
    /// Invalid state for this operation (maps to code::WRONG_STATE = 202)
    WrongState = 202,
    /// Item already processed (maps to code::ALREADY_PROCESSED = 203)
    AlreadyProcessed = 203,
    /// Reentrancy detected (maps to code::REENTRANCY = 204)
    ReentrancyDetected = 204,
    /// Commitment or item not active (maps to code::NOT_ACTIVE = 205)
    NotActive = 205,

    // Resource Errors (300-399) - aligned with shared_utils::error_codes::code
    /// Resource not found (maps to code::NOT_FOUND = 300)
    NotFound = 300,
    /// Insufficient balance (maps to code::INSUFFICIENT_BALANCE = 301)
    InsufficientBalance = 301,
    /// Insufficient commitment value (maps to code::INSUFFICIENT_VALUE = 302)
    InsufficientValue = 302,
    /// Token transfer failed (maps to code::TRANSFER_FAILED = 303)
    TransferFailed = 303,

    // System Errors (400-499) - aligned with shared_utils::error_codes::code
    /// Storage operation failed (maps to code::STORAGE_ERROR = 400)
    StorageError = 400,
    /// Cross-contract call failed (maps to code::CONTRACT_CALL_FAILED = 401)
    ContractCallFailed = 401,
}

impl Error {
    /// Human-readable message for this error (for events and clients).
    ///
    /// Messages are aligned with shared_utils::error_codes::message_for_code
    /// to ensure consistent error reporting across CommitLabs contracts.
    pub fn message(&self) -> &'static str {
        match self {
            // Validation Errors
            Error::InvalidAmount => "Invalid amount: must be greater than zero",
            Error::InvalidDuration => "Invalid duration: must be greater than zero",
            Error::InvalidPercent => "Invalid percent: must be between 0 and 100",
            Error::InvalidType => "Invalid type: value not allowed",
            Error::OutOfRange => "Value out of allowed range",
            Error::EmptyString => "Required field must not be empty",

            // Authorization Errors
            Error::Unauthorized => "Unauthorized: caller not allowed",
            Error::NotOwner => "Caller is not the owner",
            Error::NotAdmin => "Caller is not the admin",
            Error::NotAuthorizedContract => "Caller contract not authorized",

            // State Errors
            Error::AlreadyInitialized => "Contract already initialized",
            Error::NotInitialized => "Contract not initialized",
            Error::WrongState => "Invalid state for this operation",
            Error::AlreadyProcessed => "Item already processed",
            Error::ReentrancyDetected => "Reentrancy detected",
            Error::NotActive => "Commitment or item not active",

            // Resource Errors
            Error::NotFound => "Resource not found",
            Error::InsufficientBalance => "Insufficient balance",
            Error::InsufficientValue => "Insufficient commitment value",
            Error::TransferFailed => "Token transfer failed",

            // System Errors
            Error::StorageError => "Storage operation failed",
            Error::ContractCallFailed => "Cross-contract call failed",
        }
    }

    /// Get the standardized error code value.
    ///
    /// This returns the u32 representation which maps directly to
    /// shared_utils::error_codes::code constants.
    pub fn code(&self) -> u32 {
        *self as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ======================================================================
    // Error Code Mapping Tests
    // ======================================================================

    #[test]
    fn test_validation_error_codes() {
        // Validation errors (1-99) - aligned with shared_utils::error_codes::code
        assert_eq!(Error::InvalidAmount.code(), 1);
        assert_eq!(Error::InvalidDuration.code(), 2);
        assert_eq!(Error::InvalidPercent.code(), 3);
        assert_eq!(Error::InvalidType.code(), 4);
        assert_eq!(Error::OutOfRange.code(), 5);
        assert_eq!(Error::EmptyString.code(), 6);
    }

    #[test]
    fn test_authorization_error_codes() {
        // Authorization errors (100-199) - aligned with shared_utils::error_codes::code
        assert_eq!(Error::Unauthorized.code(), 100);
        assert_eq!(Error::NotOwner.code(), 101);
        assert_eq!(Error::NotAdmin.code(), 102);
        assert_eq!(Error::NotAuthorizedContract.code(), 103);
    }

    #[test]
    fn test_state_error_codes() {
        // State errors (200-299) - aligned with shared_utils::error_codes::code
        assert_eq!(Error::AlreadyInitialized.code(), 200);
        assert_eq!(Error::NotInitialized.code(), 201);
        assert_eq!(Error::WrongState.code(), 202);
        assert_eq!(Error::AlreadyProcessed.code(), 203);
        assert_eq!(Error::ReentrancyDetected.code(), 204);
        assert_eq!(Error::NotActive.code(), 205);
    }

    #[test]
    fn test_resource_error_codes() {
        // Resource errors (300-399) - aligned with shared_utils::error_codes::code
        assert_eq!(Error::NotFound.code(), 300);
        assert_eq!(Error::InsufficientBalance.code(), 301);
        assert_eq!(Error::InsufficientValue.code(), 302);
        assert_eq!(Error::TransferFailed.code(), 303);
    }

    #[test]
    fn test_system_error_codes() {
        // System errors (400-499) - aligned with shared_utils::error_codes::code
        assert_eq!(Error::StorageError.code(), 400);
        assert_eq!(Error::ContractCallFailed.code(), 401);
    }

    // ======================================================================
    // Error Message Tests
    // ======================================================================

    #[test]
    fn test_validation_error_messages() {
        assert_eq!(
            Error::InvalidAmount.message(),
            "Invalid amount: must be greater than zero"
        );
        assert_eq!(
            Error::InvalidDuration.message(),
            "Invalid duration: must be greater than zero"
        );
        assert_eq!(
            Error::InvalidPercent.message(),
            "Invalid percent: must be between 0 and 100"
        );
        assert_eq!(
            Error::InvalidType.message(),
            "Invalid type: value not allowed"
        );
        assert_eq!(Error::OutOfRange.message(), "Value out of allowed range");
        assert_eq!(
            Error::EmptyString.message(),
            "Required field must not be empty"
        );
    }

    #[test]
    fn test_authorization_error_messages() {
        assert_eq!(
            Error::Unauthorized.message(),
            "Unauthorized: caller not allowed"
        );
        assert_eq!(Error::NotOwner.message(), "Caller is not the owner");
        assert_eq!(Error::NotAdmin.message(), "Caller is not the admin");
        assert_eq!(
            Error::NotAuthorizedContract.message(),
            "Caller contract not authorized"
        );
    }

    #[test]
    fn test_state_error_messages() {
        assert_eq!(
            Error::AlreadyInitialized.message(),
            "Contract already initialized"
        );
        assert_eq!(Error::NotInitialized.message(), "Contract not initialized");
        assert_eq!(
            Error::WrongState.message(),
            "Invalid state for this operation"
        );
        assert_eq!(Error::AlreadyProcessed.message(), "Item already processed");
        assert_eq!(Error::ReentrancyDetected.message(), "Reentrancy detected");
        assert_eq!(Error::NotActive.message(), "Commitment or item not active");
    }

    #[test]
    fn test_resource_error_messages() {
        assert_eq!(Error::NotFound.message(), "Resource not found");
        assert_eq!(Error::InsufficientBalance.message(), "Insufficient balance");
        assert_eq!(
            Error::InsufficientValue.message(),
            "Insufficient commitment value"
        );
        assert_eq!(Error::TransferFailed.message(), "Token transfer failed");
    }

    #[test]
    fn test_system_error_messages() {
        assert_eq!(Error::StorageError.message(), "Storage operation failed");
        assert_eq!(
            Error::ContractCallFailed.message(),
            "Cross-contract call failed"
        );
    }

    // ======================================================================
    // Error Category Boundary Tests
    // ======================================================================

    #[test]
    fn test_error_categories_boundaries() {
        // Validation category (1-99)
        assert!(Error::InvalidAmount.code() >= category::VALIDATION_START);
        assert!(Error::InvalidAmount.code() <= category::VALIDATION_END);

        // Authorization category (100-199)
        assert!(Error::Unauthorized.code() >= category::AUTH_START);
        assert!(Error::Unauthorized.code() <= category::AUTH_END);

        // State category (200-299)
        assert!(Error::AlreadyInitialized.code() >= category::STATE_START);
        assert!(Error::AlreadyInitialized.code() <= category::STATE_END);

        // Resource category (300-399)
        assert!(Error::NotFound.code() >= category::RESOURCE_START);
        assert!(Error::NotFound.code() <= category::RESOURCE_END);

        // System category (400-499)
        assert!(Error::StorageError.code() >= category::SYSTEM_START);
        assert!(Error::StorageError.code() <= category::SYSTEM_END);
    }

    // ======================================================================
    // Error Ordering and Comparison Tests
    // ======================================================================

    #[test]
    fn test_error_ordering() {
        // Test that errors maintain proper ordering by code
        assert!(Error::InvalidAmount < Error::InvalidDuration);
        assert!(Error::InvalidDuration < Error::InvalidPercent);
        assert!(Error::EmptyString < Error::Unauthorized);
        assert!(Error::NotAuthorizedContract < Error::AlreadyInitialized);
        assert!(Error::NotActive < Error::NotFound);
        assert!(Error::TransferFailed < Error::StorageError);
    }

    #[test]
    fn test_error_equality() {
        let err1 = Error::InvalidAmount;
        let err2 = Error::InvalidAmount;
        assert_eq!(err1, err2);
        assert_eq!(err1.code(), err2.code());
        assert_eq!(err1.message(), err2.message());

        let err3 = Error::Unauthorized;
        assert_ne!(err1, err3);
    }

    // ======================================================================
    // Error Message Consistency Tests
    // ======================================================================

    #[test]
    fn test_message_consistency() {
        // Verify all errors have non-empty messages
        let errors = [
            Error::InvalidAmount,
            Error::InvalidDuration,
            Error::InvalidPercent,
            Error::InvalidType,
            Error::OutOfRange,
            Error::EmptyString,
            Error::Unauthorized,
            Error::NotOwner,
            Error::NotAdmin,
            Error::NotAuthorizedContract,
            Error::AlreadyInitialized,
            Error::NotInitialized,
            Error::WrongState,
            Error::AlreadyProcessed,
            Error::ReentrancyDetected,
            Error::NotActive,
            Error::NotFound,
            Error::InsufficientBalance,
            Error::InsufficientValue,
            Error::TransferFailed,
            Error::StorageError,
            Error::ContractCallFailed,
        ];

        for err in errors.iter() {
            assert!(
                !err.message().is_empty(),
                "Error {:?} has empty message",
                err
            );
        }
    }
}
