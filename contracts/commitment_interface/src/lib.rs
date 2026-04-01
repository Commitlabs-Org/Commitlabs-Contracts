#![no_std]
//! Interface-only ABI definitions for the live commitment contracts.
//!
//! This crate intentionally contains no storage or business logic. It exists to
//! give downstream callers and generated bindings a stable contract surface that
//! mirrors the production `commitment_core` data model.


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
    /// # Errors
    /// Interface-only crate; callers should expect the live implementation to
    /// reject re-initialization and invalid addresses.
    ///
    /// # Security
    /// Live implementations must protect this entrypoint so initialization is
    /// single-use and cannot be front-run into an unsafe admin assignment.
    pub fn initialize(_env: Env, _admin: Address, _nft_contract: Address) -> Result<(), Error> {
        unimplemented!("interface only")
    }

    /// Create a new commitment.
    ///
    /// # Arguments
    /// * `owner` - Beneficial owner of the commitment.
    /// * `amount` - Initial amount to lock.
    /// * `asset_address` - Token contract for the committed asset.
    /// * `rules` - Commitment policy and risk configuration.
    ///
    /// # Errors
    /// Live implementations may reject invalid amounts, invalid rules,
    /// insufficient balance, or unauthorized state transitions.
    ///
    /// # Security
    /// Production contracts must validate caller trust boundaries, require auth
    /// where ownership is asserted, and guard all external transfer/mint flows
    /// against reentrancy.
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
    /// # Security
    /// Live implementations mutate storage and perform token/NFT cross-contract
    /// calls. They must use checks-effects-interactions and reentrancy guards.
    pub fn settle(_env: Env, _commitment_id: String) -> Result<(), Error> {
        unimplemented!("interface only")
    }

    /// Exit a commitment early on behalf of its owner.
    ///
    /// # Security
    /// Live implementations must enforce owner authorization before any value
    /// transfer and must apply penalty arithmetic with overflow-safe helpers.
    pub fn early_exit(_env: Env, _commitment_id: String, _caller: Address) -> Result<(), Error> {
        unimplemented!("interface only")
    }
}

#[cfg(test)]
mod tests {
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
}
