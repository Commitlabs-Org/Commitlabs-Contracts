//! Integration Test Suite for Commitlabs Contracts
//!
//! This module provides a comprehensive integration test suite that validates:
//! - Frontend-style contract calls
//! - Cross-contract interactions
//! - Oracle integration
//! - Asset/token contract interactions
//! - End-to-end user flows
//! - Error scenarios and edge cases
//!
//! # Test Organization
//! - `harness`: Reusable test harness and helpers
//! - `frontend_tests`: Frontend-style call simulations
//! - `cross_contract_tests`: Cross-contract interaction tests
//! - `oracle_tests`: Oracle integration tests
//! - `token_tests`: Token/asset interaction tests
//! - `e2e_tests`: End-to-end flow tests
//! - `error_tests`: Error and edge case tests

#![cfg(test)]

pub mod harness;
pub mod frontend_tests;
pub mod cross_contract_tests;
pub mod oracle_tests;
pub mod token_tests;
pub mod e2e_tests;
pub mod error_tests;

// Re-export commonly used items for convenience
pub use harness::*;
