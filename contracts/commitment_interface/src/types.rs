//! Shared interface types for commitment contracts.

use soroban_sdk::{contracttype, Address, String};

/// Rules governing how a commitment behaves over its lifecycle.
///
/// # Security
/// Integrators should treat `commitment_type`, `max_loss_percent`, and
/// `early_exit_penalty` as policy-critical fields. State-changing contracts
/// must validate them before persisting any commitment.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CommitmentRules {
    /// Duration of the commitment in whole days.
    pub duration_days: u32,
    /// Maximum tolerated drawdown before the commitment is considered violated.
    pub max_loss_percent: u32,
    /// Risk class, currently expected to match the live contract values:
    /// `safe`, `balanced`, or `aggressive`.
    pub commitment_type: String,
    /// Penalty percentage charged on early exit.
    pub early_exit_penalty: u32,
    /// Minimum fees required before a fee-based strategy is considered healthy.
    pub min_fee_threshold: i128,
    /// Grace period in days before enforcement actions escalate.
    pub grace_period_days: u32,
}

/// Canonical commitment record returned by live on-chain contracts.
///
/// # Security
/// `status`, `current_value`, and `expires_at` are mutable runtime state. Any
/// contract exposing mutations of these fields must enforce the relevant auth
/// checks and preserve arithmetic safety for value updates.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Commitment {
    /// Unique on-chain identifier such as `c_0`.
    pub commitment_id: String,
    /// Commitment owner whose funds are locked in the core contract.
    pub owner: Address,
    /// Associated NFT token id minted by `commitment_nft`.
    pub nft_token_id: u32,
    /// Policy and risk settings fixed at creation time.
    pub rules: CommitmentRules,
    /// Initial committed amount.
    pub amount: i128,
    /// Token contract address for the committed asset.
    pub asset_address: Address,
    /// Ledger timestamp when the commitment was created.
    pub created_at: u64,
    /// Ledger timestamp when the commitment becomes eligible for settlement.
    pub expires_at: u64,
    /// Latest tracked value for the position.
    pub current_value: i128,
    /// Lifecycle status such as `active`, `settled`, `violated`, or `early_exit`.
    pub status: String,
}

/// Event payload emitted by the live core contract when a commitment is created.
///
/// # Security
/// Consumers should treat this as observational data only. The authoritative
/// state still lives in `get_commitment`, because later lifecycle transitions
/// may change `current_value` or `status`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CommitmentCreatedEvent {
    /// Unique on-chain identifier such as `c_0`.
    pub commitment_id: String,
    /// Commitment owner.
    pub owner: Address,
    /// Initial committed amount.
    pub amount: i128,
    /// Token contract address for the committed asset.
    pub asset_address: Address,
    /// Associated NFT token id minted by `commitment_nft`.
    pub nft_token_id: u32,
    /// Policy and risk settings fixed at creation time.
    pub rules: CommitmentRules,
    /// Ledger timestamp when the event was emitted.
    pub timestamp: u64,
}

/// Event payload emitted by the live core contract when a commitment is settled.
///
/// # Security
/// This payload reflects a completed state transition that also performs
/// token/NFT cross-contract interactions in the live contract.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CommitmentSettledEvent {
    /// Unique on-chain identifier such as `c_0`.
    pub commitment_id: String,
    /// Commitment owner receiving the settlement.
    pub owner: Address,
    /// Amount returned to the owner on settlement.
    pub settlement_amount: i128,
    /// Ledger timestamp when the event was emitted.
    pub timestamp: u64,
}
