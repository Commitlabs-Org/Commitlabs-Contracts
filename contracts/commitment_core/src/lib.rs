#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Map, String, Symbol,
};

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

// Storage keys
#[contracttype]
pub enum DataKey {
    Admin,
    NftContract,
    AuthorizedUpdaters,
    Commitment(String), // commitment_id -> Commitment
}

// Event topics
const VALUE_UPDATED: Symbol = symbol_short!("ValUpdate");
const VIOLATION_DETECTED: Symbol = symbol_short!("Violation");

#[contract]
pub struct CommitmentCoreContract;

#[contractimpl]
impl CommitmentCoreContract {
    /// Initialize the core commitment contract
    pub fn initialize(e: Env, admin: Address, nft_contract: Address) {
        // Store admin address
        e.storage().instance().set(&DataKey::Admin, &admin);
        // Store NFT contract address
        e.storage().instance().set(&DataKey::NftContract, &nft_contract);
        // Initialize authorized updaters map (empty initially)
        let authorized_updaters: Map<Address, bool> = Map::new(&e);
        e.storage().instance().set(&DataKey::AuthorizedUpdaters, &authorized_updaters);
    }

    /// Get admin address
    fn get_admin(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not initialized")
    }

    /// Check if caller is admin
    fn is_admin(e: &Env, caller: &Address) -> bool {
        let admin = Self::get_admin(e);
        admin == *caller
    }

    /// Check if caller is authorized updater
    fn is_authorized_updater(e: &Env, caller: &Address) -> bool {
        let admin = Self::get_admin(e);
        // Admin is always authorized
        if admin == *caller {
            return true;
        }
        // Check authorized updaters map
        let authorized_updaters: Map<Address, bool> = e
            .storage()
            .instance()
            .get(&DataKey::AuthorizedUpdaters)
            .unwrap_or(Map::new(e));
        authorized_updaters.get(caller.clone()).unwrap_or(false)
    }

    /// Add an authorized updater (admin only)
    pub fn add_authorized_updater(e: Env, caller: Address, updater: Address) {
        caller.require_auth();
        if !Self::is_admin(&e, &caller) {
            panic!("Only admin can add authorized updaters");
        }

        let mut authorized_updaters: Map<Address, bool> = e
            .storage()
            .instance()
            .get(&DataKey::AuthorizedUpdaters)
            .unwrap_or(Map::new(&e));
        authorized_updaters.set(updater.clone(), true);
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedUpdaters, &authorized_updaters);
    }

    /// Remove an authorized updater (admin only)
    pub fn remove_authorized_updater(e: Env, caller: Address, updater: Address) {
        caller.require_auth();
        if !Self::is_admin(&e, &caller) {
            panic!("Only admin can remove authorized updaters");
        }

        let mut authorized_updaters: Map<Address, bool> = e
            .storage()
            .instance()
            .get(&DataKey::AuthorizedUpdaters)
            .unwrap_or(Map::new(&e));
        authorized_updaters.set(updater.clone(), false);
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedUpdaters, &authorized_updaters);
    }

    /// Store commitment in storage
    fn store_commitment(e: &Env, commitment: &Commitment) {
        let key = DataKey::Commitment(commitment.commitment_id.clone());
        e.storage().persistent().set(&key, commitment);
    }

    /// Get commitment from storage
    fn get_commitment_storage(e: &Env, commitment_id: &String) -> Option<Commitment> {
        let key = DataKey::Commitment(commitment_id.clone());
        e.storage().persistent().get(&key)
    }

    /// Calculate drawdown percentage: (initial_value - current_value) / initial_value * 100
    fn calculate_drawdown_percent(initial_value: i128, current_value: i128) -> i128 {
        if initial_value == 0 {
            return 0;
        }

        // Calculate loss: initial_value - current_value
        let loss = initial_value - current_value;

        // Calculate percentage: (loss * 100) / initial_value
        (loss * 100) / initial_value
    }

    /// Check if max_loss_percent is violated
    fn check_violation(initial_value: i128, current_value: i128, max_loss_percent: u32) -> bool {
        if initial_value == 0 {
            return false; // Cannot determine violation without initial value
        }

        let drawdown_percent = Self::calculate_drawdown_percent(initial_value, current_value);
        
        // Convert max_loss_percent to i128 for comparison
        let max_loss = max_loss_percent as i128;
        
        // Violation occurs if drawdown exceeds max_loss_percent
        drawdown_percent > max_loss
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
        Self::get_commitment_storage(&e, &commitment_id)
            .expect("Commitment not found")
    }

    /// Update commitment value (called by allocation logic)
    pub fn update_value(e: Env, caller: Address, commitment_id: String, new_value: i128) {
        // Verify caller is authorized (allocation contract or admin)
        caller.require_auth();
        if !Self::is_authorized_updater(&e, &caller) {
            panic!("Caller is not authorized to update values");
        }

        // Retrieve commitment from storage
        let mut commitment = Self::get_commitment_storage(&e, &commitment_id)
            .expect("Commitment not found");

        // Only update if commitment is still active
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            panic!("Cannot update value for non-active commitment");
        }

        // Store old value for event
        let old_value = commitment.current_value;
        let initial_value = commitment.amount;

        // Update current_value
        commitment.current_value = new_value;

        // Calculate drawdown percentage
        let drawdown_percent = Self::calculate_drawdown_percent(initial_value, new_value);

        // Check if max_loss_percent is violated
        let is_violated = Self::check_violation(
            initial_value,
            new_value,
            commitment.rules.max_loss_percent,
        );

        // If violated, mark commitment as "violated"
        if is_violated {
            let violated_status = String::from_str(&e, "violated");
            commitment.status = violated_status;

            // Emit violation event
            e.events().publish(
                (VIOLATION_DETECTED,),
                (
                    commitment_id.clone(),
                    initial_value,
                    new_value,
                    drawdown_percent,
                    commitment.rules.max_loss_percent,
                ),
            );
        }

        // Store updated commitment
        Self::store_commitment(&e, &commitment);

        // Emit value update event
        e.events().publish(
            (VALUE_UPDATED,),
            (
                commitment_id,
                old_value,
                new_value,
                drawdown_percent,
                is_violated,
            ),
        );
    }

    /// Check if commitment rules are violated
    pub fn check_violations(e: Env, commitment_id: String) -> bool {
        let commitment = Self::get_commitment_storage(&e, &commitment_id)
            .expect("Commitment not found");

        // Check if max_loss_percent exceeded
        let is_violated = Self::check_violation(
            commitment.amount,
            commitment.current_value,
            commitment.rules.max_loss_percent,
        );

        if is_violated {
            return true;
        }

        // Check if duration expired
        let current_time = e.ledger().timestamp();
        if current_time >= commitment.expires_at {
            return true;
        }

        // Check if status is already violated
        let violated_status = String::from_str(&e, "violated");
        if commitment.status == violated_status {
            return true;
        }

        false
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

