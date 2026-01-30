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

pub mod math;
pub mod time;
pub mod validation;
pub mod storage;
pub mod errors;
pub mod error_codes;
pub mod access_control;
pub mod events;
pub mod rate_limiting;
pub mod fees;

#[cfg(test)]
mod tests;

// Re-export commonly used items
pub use math::*;
pub use time::*;
pub use validation::*;
pub use storage::*;
pub use errors::*;
pub use error_codes::*;
pub use access_control::*;
pub use events::*;
pub use rate_limiting::*;
pub use fees::*;
