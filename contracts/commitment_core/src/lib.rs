#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentRules {
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String, // "safe", "balanced", "aggressive"
    pub early_exit_penalty: u32,
    pub min_fee_threshold: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commitment {
    pub commitment_id: String,
    pub owner: Address,
    pub nft_token_id: u32,
    pub rules: CommitmentRules,
    pub amount: i128,
    pub asset_address: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub current_value: i128,
    pub status: String, // "active", "settled", "violated", "early_exit"
}

#[contract]
pub struct CommitmentCoreContract;

// Storage keys - using Symbol for efficient storage (max 9 chars)
fn commitment_key(_e: &Env) -> Symbol {
    symbol_short!("Commit")
}

fn admin_key(_e: &Env) -> Symbol {
    symbol_short!("Admin")
}

fn nft_contract_key(_e: &Env) -> Symbol {
    symbol_short!("NFT")
}

// Error types for better error handling
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitmentError {
    NotFound = 1,
    AlreadySettled = 2,
    NotExpired = 3,
    Unauthorized = 4,
    InvalidRules = 5,
}

// Storage helpers
fn read_commitment(e: &Env, commitment_id: &String) -> Option<Commitment> {
    let key = (commitment_key(e), commitment_id.clone());
    e.storage().persistent().get(&key)
}

fn set_commitment(e: &Env, commitment: &Commitment) {
    let key = (commitment_key(e), commitment.commitment_id.clone());
    e.storage().persistent().set(&key, commitment);
}

fn has_commitment(e: &Env, commitment_id: &String) -> bool {
    let key = (commitment_key(e), commitment_id.clone());
    e.storage().persistent().has(&key)
}

#[contractimpl]
impl CommitmentCoreContract {
    /// Initialize the core commitment contract
    pub fn initialize(_e: Env, _admin: Address, _nft_contract: Address) {
        // TODO: Store admin and NFT contract address
        // TODO: Initialize storage
    }

    /// Create a new commitment
    pub fn create_commitment(
        e: Env,
        _owner: Address,
        _amount: i128,
        _asset_address: Address,
        _rules: CommitmentRules,
    ) -> String {
        // TODO: Validate rules
        // TODO: Transfer assets from owner to contract
        // TODO: Call NFT contract to mint Commitment NFT
        // TODO: Store commitment data
        // TODO: Emit creation event
        String::from_str(&e, "commitment_id_placeholder")
    }

    /// Get commitment details
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"))
    }

    /// Update commitment value (called by allocation logic)
    pub fn update_value(_e: Env, _commitment_id: String, _new_value: i128) {
        // TODO: Verify caller is authorized (allocation contract)
        // TODO: Update current_value
        // TODO: Check if max_loss_percent is violated
        // TODO: Emit value update event
    }

    /// Check if commitment rules are violated
    /// Returns true if any rule violation is detected (loss limit or duration)
    pub fn check_violations(e: Env, commitment_id: String) -> bool {
        let commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"));

        // Skip check if already settled or violated
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            return false; // Already processed
        }

        let current_time = e.ledger().timestamp();

        // Check loss limit violation
        // Calculate loss percentage: ((amount - current_value) / amount) * 100
        let loss_amount = commitment.amount - commitment.current_value;
        let loss_percent = if commitment.amount > 0 {
            // Use i128 arithmetic to avoid overflow
            // loss_percent = (loss_amount * 100) / amount
            (loss_amount * 100) / commitment.amount
        } else {
            0
        };

        // Convert max_loss_percent (u32) to i128 for comparison
        let max_loss = commitment.rules.max_loss_percent as i128;
        let loss_violated = loss_percent > max_loss;

        // Check duration violation (expired)
        let duration_violated = current_time >= commitment.expires_at;

        // Return true if any violation exists
        loss_violated || duration_violated
    }

    /// Get detailed violation information
    /// Returns a tuple: (has_violations, loss_violated, duration_violated, loss_percent, time_remaining)
    pub fn get_violation_details(
        e: Env,
        commitment_id: String,
    ) -> (bool, bool, bool, i128, u64) {
        let commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"));

        let current_time = e.ledger().timestamp();

        // Calculate loss percentage
        let loss_amount = commitment.amount - commitment.current_value;
        let loss_percent = if commitment.amount > 0 {
            (loss_amount * 100) / commitment.amount
        } else {
            0
        };

        // Check loss limit violation
        let max_loss = commitment.rules.max_loss_percent as i128;
        let loss_violated = loss_percent > max_loss;

        // Check duration violation
        let duration_violated = current_time >= commitment.expires_at;

        // Calculate time remaining (0 if expired)
        let time_remaining = if current_time < commitment.expires_at {
            commitment.expires_at - current_time
        } else {
            0
        };

        let has_violations = loss_violated || duration_violated;

        (has_violations, loss_violated, duration_violated, loss_percent, time_remaining)
    }

    /// Settle commitment at maturity
    pub fn settle(_e: Env, _commitment_id: String) {
        // TODO: Verify commitment is expired
        // TODO: Calculate final settlement amount
        // TODO: Transfer assets back to owner
        // TODO: Mark commitment as settled
        // TODO: Call NFT contract to mark NFT as settled
        // TODO: Emit settlement event
    }

    /// Early exit (with penalty)
    pub fn early_exit(_e: Env, _commitment_id: String, _caller: Address) {
        // TODO: Verify caller is owner
        // TODO: Calculate penalty
        // TODO: Transfer remaining amount (after penalty) to owner
        // TODO: Mark commitment as early_exit
        // TODO: Emit early exit event
    }

    /// Allocate liquidity (called by allocation strategy)
    pub fn allocate(_e: Env, _commitment_id: String, _target_pool: Address, _amount: i128) {
        // TODO: Verify caller is authorized allocation contract
        // TODO: Verify commitment is active
        // TODO: Transfer assets to target pool
        // TODO: Record allocation
        // TODO: Emit allocation event
    }
}

#[cfg(test)]
mod tests;

