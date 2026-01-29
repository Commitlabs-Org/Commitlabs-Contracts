//! Standardized error codes for the commitment interface.
//! Aligned with shared_utils::error_codes categories for consistency.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // Resource / Validation (1-99)
    NotFound = 1,
    Unauthorized = 2,
    AlreadyInitialized = 3,
    InvalidAmount = 4,
    InvalidDuration = 5,
    InvalidPercent = 6,
    InvalidType = 7,
    OutOfRange = 8,
    NotOwner = 9,
    NotAdmin = 10,
    NotAuthorizedContract = 11,
    NotInitialized = 12,
    WrongState = 13,
    AlreadyProcessed = 14,
    ReentrancyDetected = 15,
    NotActive = 16,
    InsufficientBalance = 17,
    InsufficientValue = 18,
    TransferFailed = 19,
    StorageError = 20,
    ContractCallFailed = 21,
}

impl Error {
    /// Human-readable message for this error (for events and clients).
    pub fn message(&self) -> &'static str {
        match self {
            Error::NotFound => "Resource not found",
            Error::Unauthorized => "Unauthorized: caller not allowed",
            Error::AlreadyInitialized => "Contract already initialized",
            Error::InvalidAmount => "Invalid amount: must be greater than zero",
            Error::InvalidDuration => "Invalid duration: must be greater than zero",
            Error::InvalidPercent => "Invalid percent: must be between 0 and 100",
            Error::InvalidType => "Invalid type: value not allowed",
            Error::OutOfRange => "Value out of allowed range",
            Error::NotOwner => "Caller is not the owner",
            Error::NotAdmin => "Caller is not the admin",
            Error::NotAuthorizedContract => "Caller contract not authorized",
            Error::NotInitialized => "Contract not initialized",
            Error::WrongState => "Invalid state for this operation",
            Error::AlreadyProcessed => "Item already processed",
            Error::ReentrancyDetected => "Reentrancy detected",
            Error::NotActive => "Commitment or item not active",
            Error::InsufficientBalance => "Insufficient balance",
            Error::InsufficientValue => "Insufficient commitment value",
            Error::TransferFailed => "Token transfer failed",
            Error::StorageError => "Storage operation failed",
            Error::ContractCallFailed => "Cross-contract call failed",
        }
    }
}
