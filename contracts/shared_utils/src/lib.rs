#![no_std]

//! Shared utility library for Soroban smart contracts
//!
//! This library provides common functions, helpers, and patterns used across
//! all CommitLabs contracts including:
//! - Math utilities (safe math, percentages)
//! - Time utilities (timestamps, durations)
//! - Validation utilities
//! - Storage helpers
//! - Error helpers
//! - Access control patterns
//! - Event emission patterns
//! - Rate limiting helpers

pub mod access_control;
pub mod batch;
pub mod emergency;
pub mod error_codes;
pub mod errors;
pub mod events;
pub mod math;
pub mod pausable;
pub mod rate_limiting;
pub mod storage;
pub mod time;
pub mod validation;

#[cfg(test)]
mod tests;

// Re-export commonly used items (explicit only to avoid E0252 glob clashes)
pub use access_control::AccessControl;
pub use batch::{
    BatchConfig, BatchDataKey, BatchError, BatchMode, BatchOperationReport, BatchProcessor,
    BatchResultString, BatchResultVoid, DetailedBatchError, RollbackHelper, StateSnapshot,
};
pub use emergency::EmergencyControl;
pub use error_codes::{category, code, emit_error_event, message_for_code};
pub use errors::ErrorHelper;
pub use events::Events;
pub use math::SafeMath;
pub use pausable::Pausable;
pub use rate_limiting::RateLimiter;
pub use storage::Storage;
pub use time::TimeUtils;
pub use validation::Validation;
pub use error_codes::*;
pub use errors::*;
pub use events::*;
pub use math::*;
pub use pausable::*;
pub use rate_limiting::*;
pub use storage::*;
pub use time::*;
pub use validation::*;
