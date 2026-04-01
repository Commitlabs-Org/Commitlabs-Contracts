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
pub mod fees;
pub mod math;
pub mod pausable;
pub mod rate_limiting;
pub mod storage;
pub mod time;
pub mod validation;
pub mod fee;

#[cfg(all(test, not(target_family = "wasm")))]
mod tests;

// Re-export all public items from each utility module
pub use access_control::AccessControl;
pub use batch::{
    BatchConfig, BatchDataKey, BatchError, BatchMode, BatchOperationReport, BatchProcessor,
    BatchResultString, BatchResultVoid, DetailedBatchError, RollbackHelper, StateSnapshot,
};
pub use emergency::EmergencyControl;

