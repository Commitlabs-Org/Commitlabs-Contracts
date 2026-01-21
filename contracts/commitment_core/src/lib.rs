#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec, Map,
    Val, BytesN, IntoVal,
};
use soroban_sdk::storage::Storage;

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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allocation {
    pub commitment_id: String,
    pub target_pool: Address,
    pub amount: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationTracking {
    pub total_allocated: i128,
    pub allocations: Vec<Allocation>,
}

// Storage Data Keys
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    AuthorizedAllocator(Address),
    Commitment(String),
    CommitmentBalance(String),
    AllocationTracking(String),
    InitFlag,
}

// Error helper functions using panic with error codes
fn panic_unauthorized() -> ! {
    panic!("Unauthorized: caller is not an authorized allocation contract");
}

fn panic_insufficient_balance() -> ! {
    panic!("InsufficientBalance: commitment does not have enough balance");
}

fn panic_inactive_commitment() -> ! {
    panic!("InactiveCommitment: commitment is not active or does not exist");
}

fn panic_transfer_failed() -> ! {
    panic!("TransferFailed: asset transfer failed");
}

fn panic_already_initialized() -> ! {
    panic!("AlreadyInitialized: contract is already initialized");
}

fn panic_invalid_amount() -> ! {
    panic!("InvalidAmount: amount must be greater than zero");
}

// Helper functions for storage operations
fn has_admin(e: &Env) -> bool {
    let key = DataKey::Admin;
    e.storage().instance().has(&key)
}

fn get_admin(e: &Env) -> Address {
    let key = DataKey::Admin;
    e.storage().instance().get(&key).unwrap()
}

fn set_admin(e: &Env, admin: &Address) {
    let key = DataKey::Admin;
    e.storage().instance().set(&key, admin);
}

fn is_authorized_allocator(e: &Env, allocator: &Address) -> bool {
    let key = DataKey::AuthorizedAllocator(allocator.clone());
    if e.storage().instance().has(&key) {
        e.storage().instance().get::<DataKey, bool>(&key).unwrap_or(false)
    } else {
        false
    }
}

fn set_authorized_allocator(e: &Env, allocator: &Address, authorized: bool) {
    let key = DataKey::AuthorizedAllocator(allocator.clone());
    e.storage().instance().set(&key, &authorized);
}

fn get_commitment(e: &Env, commitment_id: &String) -> Option<Commitment> {
    let key = DataKey::Commitment(commitment_id.clone());
    e.storage().persistent().get(&key)
}

fn set_commitment(e: &Env, commitment: &Commitment) {
    let key = DataKey::Commitment(commitment.commitment_id.clone());
    e.storage().persistent().set(&key, commitment);
}

fn get_commitment_balance(e: &Env, commitment_id: &String) -> i128 {
    let key = DataKey::CommitmentBalance(commitment_id.clone());
    e.storage().persistent().get(&key).unwrap_or(0)
}

fn set_commitment_balance(e: &Env, commitment_id: &String, balance: i128) {
    let key = DataKey::CommitmentBalance(commitment_id.clone());
    e.storage().persistent().set(&key, &balance);
}

fn get_allocation_tracking(e: &Env, commitment_id: &String) -> AllocationTracking {
    let key = DataKey::AllocationTracking(commitment_id.clone());
    e.storage().persistent().get(&key).unwrap_or(AllocationTracking {
        total_allocated: 0,
        allocations: Vec::new(&e),
    })
}

fn set_allocation_tracking(e: &Env, commitment_id: &String, tracking: &AllocationTracking) {
    let key = DataKey::AllocationTracking(commitment_id.clone());
    e.storage().persistent().set(&key, tracking);
}

fn is_initialized(e: &Env) -> bool {
    let key = DataKey::InitFlag;
    if e.storage().instance().has(&key) {
        e.storage().instance().get::<DataKey, bool>(&key).unwrap_or(false)
    } else {
        false
    }
}

fn set_initialized(e: &Env) {
    let key = DataKey::InitFlag;
    e.storage().instance().set(&key, &true);
}

// Asset transfer helper function using Stellar asset contract
fn transfer_asset(e: &Env, asset: &Address, from: &Address, to: &Address, amount: i128) {
    if amount <= 0 {
        panic_invalid_amount();
    }

    // Call the asset contract's transfer function
    // The asset contract should have a transfer function with signature:
    // transfer(from: Address, to: Address, amount: i128)
    // Using invoke_contract to call the asset contract's transfer function
    let transfer_symbol = symbol_short!("transfer");
    
    // Invoke the contract's transfer function
    // Note: This assumes the asset contract follows the standard token interface
    let _: () = e.invoke_contract(
        asset,
        &transfer_symbol,
        soroban_sdk::vec![e, from.clone().into_val(e), to.clone().into_val(e), amount.into_val(e)],
    );
}

#[contract]
pub struct CommitmentCoreContract;

#[contractimpl]
impl CommitmentCoreContract {
    /// Initialize the core commitment contract
    pub fn initialize(e: Env, admin: Address, _nft_contract: Address) {
        if is_initialized(&e) {
            panic_already_initialized();
        }
        
        set_admin(&e, &admin);
        set_initialized(&e);
    }

    /// Add an authorized allocation contract
    pub fn add_authorized_allocator(e: Env, allocator: Address) {
        let admin = get_admin(&e);
        admin.require_auth();
        
        set_authorized_allocator(&e, &allocator, true);
    }

    /// Remove an authorized allocation contract
    pub fn remove_authorized_allocator(e: Env, allocator: Address) {
        let admin = get_admin(&e);
        admin.require_auth();
        
        set_authorized_allocator(&e, &allocator, false);
    }

    /// Check if an address is an authorized allocator
    pub fn is_authorized_allocator(e: Env, allocator: Address) -> bool {
        is_authorized_allocator(&e, &allocator)
    }

    /// Create a new commitment
    pub fn create_commitment(
        e: Env,
        owner: Address,
        amount: i128,
        asset_address: Address,
        rules: CommitmentRules,
    ) -> String {
        // TODO: Validate rules
        // TODO: Transfer assets from owner to contract
        // TODO: Call NFT contract to mint Commitment NFT
        // TODO: Store commitment data
        // TODO: Emit creation event
        String::from_str(&e, "commitment_id_placeholder")
    }

    /// Get commitment details
    pub fn get_commitment(e: Env, commitment_id: String) -> Option<Commitment> {
        get_commitment(&e, &commitment_id)
    }

    /// Update commitment value (called by allocation logic)
    pub fn update_value(e: Env, commitment_id: String, new_value: i128) {
        // TODO: Verify caller is authorized (allocation contract)
        // TODO: Update current_value
        // TODO: Check if max_loss_percent is violated
        // TODO: Emit value update event
    }

    /// Check if commitment rules are violated
    pub fn check_violations(e: Env, commitment_id: String) -> bool {
        // TODO: Check if max_loss_percent exceeded
        // TODO: Check if duration expired
        // TODO: Check other rule violations
        false
    }

    /// Settle commitment at maturity
    pub fn settle(e: Env, commitment_id: String) {
        // TODO: Verify commitment is expired
        // TODO: Calculate final settlement amount
        // TODO: Transfer assets back to owner
        // TODO: Mark commitment as settled
        // TODO: Call NFT contract to mark NFT as settled
        // TODO: Emit settlement event
    }

    /// Early exit (with penalty)
    pub fn early_exit(e: Env, commitment_id: String, caller: Address) {
        // TODO: Verify caller is owner
        // TODO: Calculate penalty
        // TODO: Transfer remaining amount (after penalty) to owner
        // TODO: Mark commitment as early_exit
        // TODO: Emit early exit event
    }

    /// Allocate liquidity to a target pool
    /// 
    /// # Arguments
    /// * `caller` - The address of the allocation contract calling this function (must be authorized)
    /// * `commitment_id` - The ID of the commitment
    /// * `target_pool` - The address of the target pool to allocate to
    /// * `amount` - The amount to allocate
    /// 
    /// # Errors
    /// * `Unauthorized` - If caller is not an authorized allocation contract
    /// * `InactiveCommitment` - If commitment is not active
    /// * `InsufficientBalance` - If commitment doesn't have enough balance
    /// * `TransferFailed` - If asset transfer fails
    /// * `InvalidAmount` - If amount is invalid (<= 0)
    /// 
    /// # Note
    /// The allocation contract should pass its own address as the `caller` parameter.
    /// This address must be authorized by the admin before calling this function.
    pub fn allocate(e: Env, caller: Address, commitment_id: String, target_pool: Address, amount: i128) {
        // Verify caller is authorized allocation contract
        if !is_authorized_allocator(&e, &caller) {
            panic_unauthorized();
        }

        // Verify commitment exists and is active
        let commitment = match get_commitment(&e, &commitment_id) {
            Some(c) => c,
            None => panic_inactive_commitment(),
        };

        // Check if commitment is active
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            panic_inactive_commitment();
        }

        // Verify sufficient balance
        let balance = get_commitment_balance(&e, &commitment_id);
        if balance < amount {
            panic_insufficient_balance();
        }

        // Transfer assets to target pool
        let contract_address = e.current_contract_address();
        transfer_asset(&e, &commitment.asset_address, &contract_address, &target_pool, amount);

        // Update commitment balance
        let new_balance = balance - amount;
        set_commitment_balance(&e, &commitment_id, new_balance);

        // Record allocation
        let mut tracking = get_allocation_tracking(&e, &commitment_id);
        let timestamp = e.ledger().timestamp();
        
        let allocation = Allocation {
            commitment_id: commitment_id.clone(),
            target_pool: target_pool.clone(),
            amount,
            timestamp,
        };
        
        tracking.allocations.push_back(allocation.clone());
        tracking.total_allocated += amount;
        set_allocation_tracking(&e, &commitment_id, &tracking);

        // Emit allocation event
        e.events().publish(
            (symbol_short!("alloc"), symbol_short!("cmt_id")),
            commitment_id,
        );
        e.events().publish(
            (symbol_short!("alloc"), symbol_short!("pool")),
            target_pool,
        );
        e.events().publish(
            (symbol_short!("alloc"), symbol_short!("amount")),
            amount,
        );
        e.events().publish(
            (symbol_short!("alloc"), symbol_short!("time")),
            timestamp,
        );
    }

    /// Get allocation tracking for a commitment
    pub fn get_allocation_tracking(e: Env, commitment_id: String) -> AllocationTracking {
        get_allocation_tracking(&e, &commitment_id)
    }

    /// Deallocate liquidity from a pool (optional functionality)
    /// This would be called when liquidity is returned from a pool
    /// 
    /// # Arguments
    /// * `caller` - The address of the allocation contract calling this function (must be authorized)
    /// * `commitment_id` - The ID of the commitment
    /// * `target_pool` - The address of the pool to deallocate from
    /// * `amount` - The amount to deallocate
    pub fn deallocate(e: Env, caller: Address, commitment_id: String, target_pool: Address, amount: i128) {
        // Verify caller is authorized
        if !is_authorized_allocator(&e, &caller) {
            panic_unauthorized();
        }

        // Get commitment
        let commitment = match get_commitment(&e, &commitment_id) {
            Some(c) => c,
            None => panic_inactive_commitment(),
        };

        // Transfer assets back from pool to commitment contract
        let contract_address = e.current_contract_address();
        transfer_asset(&e, &commitment.asset_address, &target_pool, &contract_address, amount);

        // Update commitment balance
        let balance = get_commitment_balance(&e, &commitment_id);
        set_commitment_balance(&e, &commitment_id, balance + amount);

        // Update allocation tracking
        let mut tracking = get_allocation_tracking(&e, &commitment_id);
        tracking.total_allocated -= amount;
        if tracking.total_allocated < 0 {
            tracking.total_allocated = 0;
        }
        set_allocation_tracking(&e, &commitment_id, &tracking);

        // Emit deallocation event
        e.events().publish(
            (symbol_short!("dealloc"), symbol_short!("cmt_id")),
            commitment_id,
        );
        e.events().publish(
            (symbol_short!("dealloc"), symbol_short!("pool")),
            target_pool,
        );
        e.events().publish(
            (symbol_short!("dealloc"), symbol_short!("amount")),
            amount,
        );
    }
}

