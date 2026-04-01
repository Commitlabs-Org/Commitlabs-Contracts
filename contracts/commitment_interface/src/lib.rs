#![no_std]
//! Interface-only ABI definitions for the live commitment contracts.
//!
//! # Overview
//!
//! This crate intentionally contains no storage or business logic. It exists to
//! give downstream callers and generated bindings a stable contract surface that
//! mirrors the production `commitment_core` data model.
//!
//! # Error Handling Strategy
//!
//! All errors in this interface are defined in [`error::Error`] which maps directly to
//! `shared_utils::error_codes` for consistency across CommitLabs contracts. Each error
//! variant has a standardized code and message that aligns with the shared utilities.
//!
//! ## Error Code Categories
//!
//! Errors are organized by category following `shared_utils::error_codes::category`:
//!
//! - **Validation (1-99)**: Invalid inputs, out-of-range values
//! - **Authorization (100-199)**: Unauthorized access, insufficient permissions  
//! - **State (200-299)**: Wrong state, already processed
//! - **Resource (300-399)**: Insufficient balance, not found
//! - **System (400-499)**: Storage failures, contract failures
//!
//! For detailed error code mappings, see [`error::Error`].
//!
//! # Security Considerations
//!
//! This is an interface-only crate. Implementations must:
//! - Enforce proper authorization checks before state mutations
//! - Validate all inputs using `shared_utils::Validation` helpers
//! - Apply reentrancy guards on cross-contract calls
//! - Use overflow-safe arithmetic from `shared_utils::SafeMath`
//! - Emit error events via `shared_utils::emit_error_event` before panicking


pub mod error;
pub mod types;

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, String, Symbol, Vec};

use crate::error::Error;
pub use crate::types::{
    Commitment, CommitmentCreatedEvent, CommitmentRules, CommitmentSettledEvent,
};

/// =======================
/// Interface Metadata
/// =======================

pub const INTERFACE_VERSION: u32 = 2;

/// =======================
/// Events
/// =======================

pub const COMMITMENT_CREATED: Symbol = symbol_short!("created");
pub const COMMITMENT_SETTLED: Symbol = symbol_short!("settled");
pub const COMMITMENT_EXITED: Symbol = symbol_short!("exited");

/// =======================
/// Interface Contract
/// =======================

#[contract]
pub struct CommitmentInterface;

#[contractimpl]
impl CommitmentInterface {
    /// Initialize the commitment system.
    ///
    /// # Arguments
    /// * `admin` - Address that controls privileged configuration.
    /// * `nft_contract` - Contract that mints and settles commitment NFTs.
    ///
    /// # Returns
    /// * `Result<(), Error>` - Ok if initialization succeeds
    ///
    /// # Errors
    /// * `Error::AlreadyInitialized` (200) - Contract already initialized
    /// * `Error::NotAuthorizedContract` (103) - Invalid NFT contract address
    ///
    /// # Security
    /// Live implementations must protect this entrypoint so initialization is
    /// single-use and cannot be front-run into an unsafe admin assignment.
    /// Admin authorization should be enforced on all privileged operations.
    pub fn initialize(_env: Env, _admin: Address, _nft_contract: Address) -> Result<(), Error> {
        unimplemented!("interface only")
    }

    /// Create a new commitment.
    ///
    /// # Arguments
    /// * `owner` - Beneficial owner of the commitment.
    /// * `amount` - Initial amount to lock (must be positive).
    /// * `asset_address` - Token contract for the committed asset.
    /// * `rules` - Commitment policy and risk configuration.
    ///
    /// # Returns
    /// * `Result<String, Error>` - Commitment ID on success (format: "c_N")
    ///
    /// # Errors
    /// * `Error::InvalidAmount` (1) - Amount is zero or negative
    /// * `Error::InvalidDuration` (2) - Duration is zero
    /// * `Error::InvalidPercent` (3) - Percentage values out of range (0-100)
    /// * `Error::InvalidType` (4) - Invalid commitment type
    /// * `Error::InsufficientBalance` (301) - Owner has insufficient token balance
    /// * `Error::TransferFailed` (303) - Token transfer failed
    /// * `Error::ContractCallFailed` (401) - NFT minting failed
    ///
    /// # Security
    /// Production contracts must:
    /// - Validate caller trust boundaries
    /// - Require auth where ownership is asserted
    /// - Guard all external transfer/mint flows against reentrancy
    /// - Use rate limiting to prevent abuse
    pub fn create_commitment(
        _env: Env,
        _owner: Address,
        _amount: i128,
        _asset_address: Address,
        _rules: CommitmentRules,
    ) -> Result<String, Error> {
        unimplemented!("interface only")
    }

    /// Fetch an existing commitment by its on-chain identifier.
    ///
    /// # Arguments
    /// * `commitment_id` - Unique commitment identifier (format: "c_N")
    ///
    /// # Returns
    /// * `Result<Commitment, Error>` - Commitment details if found
    ///
    /// # Errors
    /// * `Error::NotFound` (300) - Commitment does not exist
    ///
    /// # Security
    /// This is a read-only view function. No authorization required.
    pub fn get_commitment(_env: Env, _commitment_id: String) -> Result<Commitment, Error> {
        unimplemented!("interface only")
    }

    /// List commitment ids owned by the supplied address.
    ///
    /// # Security
    /// This is a read-only view into ownership-indexed storage in the live
    /// contract. No authorization is required because it does not mutate state.
    pub fn list_commitments_by_owner(_env: Env, _owner: Address) -> Result<Vec<String>, Error> {
        unimplemented!("interface only")
    }

    /// List commitment ids owned by the supplied address.
    pub fn get_owner_commitments(_env: Env, _owner: Address) -> Result<Vec<String>, Error> {
        unimplemented!("interface only")
    }

    /// Return the aggregate number of commitments created so far.
    ///
    /// # Returns
    /// * `Result<u64, Error>` - Total commitment count
    ///
    /// # Errors
    /// Returns 0 if no commitments exist (not an error condition).
    ///
    /// # Security
    /// This is a read-only view function. No authorization required.
    pub fn get_total_commitments(_env: Env) -> Result<u64, Error> {
        unimplemented!("interface only")
    }

    /// Return the aggregate value locked across active commitments.
    ///
    /// # Security
    /// Live implementations derive this from mutable storage updated during
    /// create, value-update, settle, and early-exit flows.
    pub fn get_total_value_locked(_env: Env) -> Result<i128, Error> {
        unimplemented!("interface only")
    }

    /// Return commitment ids created between two timestamps, inclusive.
    pub fn get_commitments_created_between(
        _env: Env,
        _from_ts: u64,
        _to_ts: u64,
    ) -> Result<Vec<String>, Error> {
        unimplemented!("interface only")
    }

    /// Return the configured admin for the live commitment core contract.
    pub fn get_admin(_env: Env) -> Result<Address, Error> {
        unimplemented!("interface only")
    }

    /// Return the linked commitment NFT contract address.
    pub fn get_nft_contract(_env: Env) -> Result<Address, Error> {
        unimplemented!("interface only")
    }

    /// Settle an expired commitment.
    ///
    /// # Arguments
    /// * `commitment_id` - Unique commitment identifier to settle
    ///
    /// # Returns
    /// * `Result<(), Error>` - Ok if settlement succeeds
    ///
    /// # Errors
    /// * `Error::NotFound` (300) - Commitment does not exist
    /// * `Error::NotActive` (205) - Commitment not in active state
    /// * `Error::WrongState` (202) - Commitment already settled or violated
    /// * `Error::ContractCallFailed` (401) - NFT settlement failed
    /// * `Error::TransferFailed` (303) - Asset transfer failed
    ///
    /// # Security
    /// Live implementations must:
    /// - Use checks-effects-interactions pattern
    /// - Apply reentrancy guards on cross-contract calls
    /// - Verify commitment has expired before settlement
    /// - Transfer assets to commitment owner (not caller)
    pub fn settle(_env: Env, _commitment_id: String) -> Result<(), Error> {
        unimplemented!("interface only")
    }

    /// Exit a commitment early on behalf of its owner.
    ///
    /// # Arguments
    /// * `commitment_id` - Unique commitment identifier to exit
    /// * `caller` - Address initiating the early exit (must be owner)
    ///
    /// # Returns
    /// * `Result<(), Error>` - Ok if early exit succeeds
    ///
    /// # Errors
    /// * `Error::NotFound` (300) - Commitment does not exist
    /// * `Error::Unauthorized` (100) - Caller is not the commitment owner
    /// * `Error::NotActive` (205) - Commitment not in active state
    /// * `Error::ContractCallFailed` (401) - NFT state update failed
    /// * `Error::TransferFailed` (303) - Asset transfer failed
    ///
    /// # Security
    /// Live implementations must:
    /// - Enforce owner authorization before any value transfer
    /// - Apply penalty arithmetic with overflow-safe helpers
    /// - Update NFT state to reflect early exit
    /// - Transfer post-penalty amount to owner
    pub fn early_exit(_env: Env, _commitment_id: String, _caller: Address) -> Result<(), Error> {
        unimplemented!("interface only")
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    extern crate alloc;

    use super::INTERFACE_VERSION;
    use alloc::{
        string::{String, ToString},
        vec::Vec,
    };

    const INTERFACE_TYPES: &str = include_str!("types.rs");
    const CORE_SOURCE: &str = include_str!("../../commitment_core/src/lib.rs");
    const ATTESTATION_SOURCE: &str = include_str!("../../attestation_engine/src/lib.rs");
    const NFT_SOURCE: &str = include_str!("../../commitment_nft/src/lib.rs");

    fn extract_block(source: &str, marker: &str) -> String {
        let start = source
            .find(marker)
            .unwrap_or_else(|| panic!("missing marker: {marker}"));
        let rest = &source[start..];
        let open = rest
            .find('{')
            .unwrap_or_else(|| panic!("missing opening brace for: {marker}"));
        let mut depth = 0usize;
        let mut end_index = None;

        for (offset, ch) in rest[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_index = Some(open + offset + 1);
                        break;
                    }
                }
                _ => {}
            }
        }

        rest[..end_index.unwrap_or_else(|| panic!("unclosed block for: {marker}"))].to_string()
    }

    fn normalize(source: &str) -> String {
        source
            .lines()
            .map(|line| line.split("//").next().unwrap_or("").trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn squish(source: &str) -> String {
        source.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn interface_version_tracks_current_abi_generation() {
        assert_eq!(INTERFACE_VERSION, 2);
    }

    #[test]
    fn commitment_rules_source_matches_commitment_core() {
        assert_eq!(
            normalize(&extract_block(
                INTERFACE_TYPES,
                "pub struct CommitmentRules {"
            )),
            normalize(&extract_block(CORE_SOURCE, "pub struct CommitmentRules {"))
        );
    }

    #[test]
    fn commitment_rules_source_matches_attestation_engine() {
        assert_eq!(
            normalize(&extract_block(
                INTERFACE_TYPES,
                "pub struct CommitmentRules {"
            )),
            normalize(&extract_block(
                ATTESTATION_SOURCE,
                "pub struct CommitmentRules {"
            ))
        );
    }

    #[test]
    fn commitment_source_matches_commitment_core() {
        assert_eq!(
            normalize(&extract_block(INTERFACE_TYPES, "pub struct Commitment {")),
            normalize(&extract_block(CORE_SOURCE, "pub struct Commitment {"))
        );
    }

    #[test]
    fn commitment_source_matches_attestation_engine() {
        assert_eq!(
            normalize(&extract_block(INTERFACE_TYPES, "pub struct Commitment {")),
            normalize(&extract_block(
                ATTESTATION_SOURCE,
                "pub struct Commitment {"
            ))
        );
    }

    #[test]
    fn created_event_source_matches_commitment_core() {
        assert_eq!(
            normalize(&extract_block(
                INTERFACE_TYPES,
                "pub struct CommitmentCreatedEvent {"
            )),
            normalize(&extract_block(
                CORE_SOURCE,
                "pub struct CommitmentCreatedEvent {"
            ))
        );
    }

    #[test]
    fn settled_event_source_matches_commitment_core() {
        assert_eq!(
            normalize(&extract_block(
                INTERFACE_TYPES,
                "pub struct CommitmentSettledEvent {"
            )),
            normalize(&extract_block(
                CORE_SOURCE,
                "pub struct CommitmentSettledEvent {"
            ))
        );
    }

    #[test]
    fn commitment_metadata_source_matches_commitment_nft() {
        assert_eq!(
            normalize(&extract_block(INTERFACE_TYPES, "pub struct CommitmentMetadata {")),
            normalize(&extract_block(NFT_SOURCE, "pub struct CommitmentMetadata {"))
        );
    }

    #[test]
    fn commitment_nft_source_matches_commitment_nft() {
        assert_eq!(
            normalize(&extract_block(INTERFACE_TYPES, "pub struct CommitmentNFT {")),
            normalize(&extract_block(NFT_SOURCE, "pub struct CommitmentNFT {"))
        );
    }

    #[test]
    fn live_core_source_contains_expected_interface_signatures() {
        let squished = squish(CORE_SOURCE);

        for signature in [
            "pub fn initialize(e: Env, admin: Address, nft_contract: Address)",
            "pub fn create_commitment( e: Env, owner: Address, amount: i128, asset_address: Address, rules: CommitmentRules, ) -> String",
            "pub fn get_commitment(e: Env, commitment_id: String) -> Commitment",
            "pub fn list_commitments_by_owner(e: Env, owner: Address) -> Vec<String>",
            "pub fn get_owner_commitments(e: Env, owner: Address) -> Vec<String>",
            "pub fn get_total_commitments(e: Env) -> u64",
            "pub fn get_total_value_locked(e: Env) -> i128",
            "pub fn get_commitments_created_between(e: Env, from_ts: u64, to_ts: u64) -> Vec<String>",
            "pub fn get_admin(e: Env) -> Address",
            "pub fn get_nft_contract(e: Env) -> Address",
            "pub fn settle(e: Env, commitment_id: String)",
            "pub fn early_exit(e: Env, commitment_id: String, caller: Address)",
        ] {
            assert!(
                squished.contains(&squish(signature)),
                "missing live-core signature: {signature}"
            );
        }
    }

    #[test]
    fn attestation_engine_reuses_the_same_commitment_types() {
        assert!(ATTESTATION_SOURCE.contains("pub struct CommitmentRules"));
        assert!(ATTESTATION_SOURCE.contains("pub struct Commitment"));
    }

    // ======================================================================
    // Error Code Category Tests
    // ======================================================================

    #[test]
    fn test_error_categories_are_properly_bounded() {
        // Validation category (1-99)
        assert_eq!(category::VALIDATION_START, 1);
        assert_eq!(category::VALIDATION_END, 99);

        // Authorization category (100-199)
        assert_eq!(category::AUTH_START, 100);
        assert_eq!(category::AUTH_END, 199);

        // State category (200-299)
        assert_eq!(category::STATE_START, 200);
        assert_eq!(category::STATE_END, 299);

        // Resource category (300-399)
        assert_eq!(category::RESOURCE_START, 300);
        assert_eq!(category::RESOURCE_END, 399);

        // System category (400-499)
        assert_eq!(category::SYSTEM_START, 400);
        assert_eq!(category::SYSTEM_END, 499);
    }

    #[test]
    fn test_error_codes_in_valid_ranges() {
        // Verify all error codes fall within their expected category ranges
        let validation_errors = [
            Error::InvalidAmount,
            Error::InvalidDuration,
            Error::InvalidPercent,
            Error::InvalidType,
            Error::OutOfRange,
            Error::EmptyString,
        ];
        for err in validation_errors.iter() {
            assert!(
                err.code() >= category::VALIDATION_START && err.code() <= category::VALIDATION_END
            );
        }

        let auth_errors = [
            Error::Unauthorized,
            Error::NotOwner,
            Error::NotAdmin,
            Error::NotAuthorizedContract,
        ];
        for err in auth_errors.iter() {
            assert!(err.code() >= category::AUTH_START && err.code() <= category::AUTH_END);
        }

        let state_errors = [
            Error::AlreadyInitialized,
            Error::NotInitialized,
            Error::WrongState,
            Error::AlreadyProcessed,
            Error::ReentrancyDetected,
            Error::NotActive,
        ];
        for err in state_errors.iter() {
            assert!(err.code() >= category::STATE_START && err.code() <= category::STATE_END);
        }

        let resource_errors = [
            Error::NotFound,
            Error::InsufficientBalance,
            Error::InsufficientValue,
            Error::TransferFailed,
        ];
        for err in resource_errors.iter() {
            assert!(err.code() >= category::RESOURCE_START && err.code() <= category::RESOURCE_END);
        }

        let system_errors = [Error::StorageError, Error::ContractCallFailed];
        for err in system_errors.iter() {
            assert!(err.code() >= category::SYSTEM_START && err.code() <= category::SYSTEM_END);
        }
    }
}
