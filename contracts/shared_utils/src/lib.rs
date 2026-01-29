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
pub mod emergency;
pub mod error_codes;
pub mod errors;
pub mod events;
pub mod math;
pub mod rate_limiting;
pub mod storage;
pub mod time;
pub mod validation;

#[cfg(test)]
mod tests;

// Re-export commonly used items
pub use access_control::*;
pub use emergency::EmergencyControl;
pub use error_codes::*;
pub use errors::*;
pub use events::*;
pub use math::*;
pub use rate_limiting::*;
pub use storage::Storage;
pub use time::*;
pub use validation::*;
