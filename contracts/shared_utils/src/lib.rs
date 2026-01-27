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
pub mod access_control;
pub mod events;
pub mod rate_limiting;

#[cfg(test)]
mod tests;

// Re-export commonly used items
pub use math::*;
pub use time::*;
pub use validation::*;
pub use storage::*;
pub use errors::*;
pub use access_control::*;
pub use events::*;
pub use rate_limiting::*;
