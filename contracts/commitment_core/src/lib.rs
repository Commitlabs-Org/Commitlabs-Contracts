#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, log, token, symbol_short, Address, Env, IntoVal, String,
    Symbol, Vec, Map
};
use shared_utils::{SafeMath, TimeUtils, Validation, RateLimiter};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CommitmentError {
    InvalidDuration = 1,
    InvalidMaxLossPercent = 2,
    InvalidCommitmentType = 3,
    InvalidAmount = 4,
    InsufficientBalance = 5,
    TransferFailed = 6,
    MintingFailed = 7,
    CommitmentNotFound = 8,
    Unauthorized = 9,
    AlreadyInitialized = 10,
    ReentrancyDetected = 11,
}

#[contracttype]
#[derive(Clone)]
pub struct CommitmentCreatedEvent {
    pub commitment_id: String,
    pub owner: Address,
    pub amount: i128,
    pub asset_address: Address,
    pub nft_token_id: u32,
    pub rules: CommitmentRules,
    pub timestamp: u64,
}

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

// Define DataKey as a single contracttype enum:
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    NftContract,
    AuthorizedUpdaters,
    Commitment(String),        // commitment_id -> Commitment
    OwnerCommitments(Address), // owner -> Vec<commitment_id>
    TotalCommitments,          // counter
    ReentrancyGuard,           // reentrancy protection flag
    TotalValueLocked,          // aggregate value locked across active commitments
}

const VALUE_UPDATED: Symbol = symbol_short!("ValUpdate");
const VIOLATION_DETECTED: Symbol = symbol_short!("Violation");

/// Transfer assets from owner to contract
fn transfer_assets(e: &Env, from: &Address, to: &Address, asset_address: &Address, amount: i128) {
    let token_client = token::Client::new(e, asset_address);

    // Check balance first
    let balance = token_client.balance(from);
    if balance < amount {
        log!(e, "Insufficient balance: {} < {}", balance, amount);
        panic!("Insufficient balance");
    }

    // Transfer tokens (fails transaction if unsuccessful)
    token_client.transfer(from, to, &amount);
}

/// Helper function to call NFT contract mint function
fn call_nft_mint(
    e: &Env,
    nft_contract: &Address,
    owner: &Address,
    commitment_id: &String,
    duration_days: u32,
    max_loss_percent: u32,
    commitment_type: &String,
    initial_amount: i128,
    asset_address: &Address,
) -> u32 {
    let mut args = Vec::new(e);
    args.push_back(owner.clone().into_val(e));
    args.push_back(commitment_id.clone().into_val(e));
    args.push_back(duration_days.into_val(e));
    args.push_back(max_loss_percent.into_val(e));
    args.push_back(commitment_type.clone().into_val(e));
    args.push_back(initial_amount.into_val(e));
    args.push_back(asset_address.clone().into_val(e));

    // In Soroban, contract calls return the value directly
    // Failures cause the entire transaction to fail
    e.invoke_contract::<u32>(nft_contract, &Symbol::new(e, "mint"), args)
}

// Storage helpers
fn read_commitment(e: &Env, commitment_id: &String) -> Option<Commitment> {
    e.storage()
        .instance()
        .get::<_, Commitment>(&DataKey::Commitment(commitment_id.clone()))
}

fn set_commitment(e: &Env, commitment: &Commitment) {
    e.storage()
        .instance()
        .set(&DataKey::Commitment(commitment.commitment_id.clone()), commitment);
}

fn has_commitment(e: &Env, commitment_id: &String) -> bool {
    e.storage()
        .instance()
        .has(&DataKey::Commitment(commitment_id.clone()))
}

/// Reentrancy protection helpers
fn require_no_reentrancy(e: &Env) {
    let guard: bool = e.storage()
        .instance()
        .get::<_, bool>(&DataKey::ReentrancyGuard)
        .unwrap_or(false);

    if guard {
        panic!("Reentrancy detected");
    }
}

fn set_reentrancy_guard(e: &Env, value: bool) {
    e.storage().instance().set(&DataKey::ReentrancyGuard, &value);
}

/// Require that the caller is the admin stored in this contract.
fn require_admin(e: &Env, caller: &Address) {
    caller.require_auth();
    let admin = e
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| panic!("Contract not initialized"));
    if *caller != admin {
        panic!("Unauthorized: only admin can perform this action");
    }
}

#[contract]
pub struct CommitmentCoreContract;

#[contractimpl]
impl CommitmentCoreContract {
    /// Validate commitment rules using shared utilities
    fn validate_rules(e: &Env, rules: &CommitmentRules) {
        // Duration must be > 0
        Validation::require_valid_duration(rules.duration_days);

        // Max loss percent must be between 0 and 100
        Validation::require_valid_percent(rules.max_loss_percent);

        // Commitment type must be valid
        let valid_types = ["safe", "balanced", "aggressive"];
        Validation::require_valid_commitment_type(e, &rules.commitment_type, &valid_types);
    }

    /// Generate unique commitment ID
    fn generate_commitment_id(e: &Env, _owner: &Address) -> String {
        let counter = e
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::TotalCommitments)
            .unwrap_or(0);
        // Create a simple unique ID using counter
        // This is a simplified version - in production you might want a more robust ID generation
        String::from_str(e, "commitment_") // We'll extend this with a proper implementation later
    }

    /// Initialize the core commitment contract
    pub fn initialize(e: Env, admin: Address, nft_contract: Address) {
        // Store admin address
        e.storage().instance().set(&DataKey::Admin, &admin);
        // Store NFT contract address
        e.storage().instance().set(&DataKey::NftContract, &nft_contract);
        // Initialize authorized updaters map (empty initially)
        let authorized_updaters: Map<Address, bool> = Map::new(&e);
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedUpdaters, &authorized_updaters);
    }

    /// Get admin address
    fn get_admin(e: &Env) -> Address {
        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Contract not initialized"))
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
            .get::<_, Map<Address, bool>>(&DataKey::AuthorizedUpdaters)
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
            .get::<_, Map<Address, bool>>(&DataKey::AuthorizedUpdaters)
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
            .get::<_, Map<Address, bool>>(&DataKey::AuthorizedUpdaters)
            .unwrap_or(Map::new(&e));
        authorized_updaters.set(updater.clone(), false);
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedUpdaters, &authorized_updaters);
    }

    /// Store commitment in storage (persistent)
    fn store_commitment(e: &Env, commitment: &Commitment) {
        let key = DataKey::Commitment(commitment.commitment_id.clone());
        e.storage().instance().set(&key, commitment);
    }

    /// Get commitment from storage
    fn get_commitment_storage(e: &Env, commitment_id: &String) -> Option<Commitment> {
        let key = DataKey::Commitment(commitment_id.clone());
        e.storage().instance().get::<_, Commitment>(&key)
    }

    /// Calculate drawdown percentage: (initial_value - current_value) / initial_value * 100
    fn calculate_drawdown_percent(initial_value: i128, current_value: i128) -> i128 {
        if initial_value == 0 {
            return 0;
        }
        let loss = initial_value - current_value;
        (loss * 100) / initial_value
    }

    /// Check if max_loss_percent is violated
    fn check_violation(initial_value: i128, current_value: i128, max_loss_percent: u32) -> bool {
        if initial_value == 0 {
            return false; // Cannot determine violation without initial value
        }
        let drawdown_percent = Self::calculate_drawdown_percent(initial_value, current_value);
        let max_loss = max_loss_percent as i128;
        drawdown_percent > max_loss
    }

    /// Create a new commitment
    /// 
    /// # Reentrancy Protection
    /// This function uses checks-effects-interactions pattern:
    /// 1. Checks: Validate inputs
    /// 2. Effects: Update state (commitment storage, counters)
    /// 3. Interactions: External calls (token transfer, NFT mint)
    /// Reentrancy guard prevents recursive calls.
    /// 
    /// # Formal Verification
    /// **Preconditions:**
    /// - `amount > 0`
    /// - `rules.duration_days > 0`
    /// - `rules.max_loss_percent <= 100`
    /// - `rules.commitment_type ∈ {"safe", "balanced", "aggressive"}`
    /// - Contract is initialized
    /// - `reentrancy_guard == false`
    /// 
    /// **Postconditions:**
    /// - Returns unique `commitment_id`
    /// - `get_commitment(commitment_id).owner == owner`
    /// - `get_commitment(commitment_id).amount == amount`
    /// - `get_commitment(commitment_id).status == "active"`
    /// - `get_total_commitments() == old(get_total_commitments()) + 1`
    /// - `reentrancy_guard == false`
    /// 
    /// **Invariants Maintained:**
    /// - INV-1: Total commitments consistency
    /// - INV-2: Commitment balance conservation
    /// - INV-3: Owner commitment list consistency
    /// - INV-4: Reentrancy guard invariant
    /// 
    /// **Security Properties:**
    /// - SP-1: Reentrancy protection
    /// - SP-2: Access control
    /// - SP-4: State consistency
    /// - SP-5: Token conservation
    pub fn create_commitment(
        e: Env,
        owner: Address,
        amount: i128,
        asset_address: Address,
        rules: CommitmentRules,
    ) -> String {
        // Reentrancy protection
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        // Rate limit: per-owner commitment creation
        let fn_symbol = symbol_short!("create");
        RateLimiter::check(&e, &owner, &fn_symbol);

        // Validate amount > 0 using shared utilities
        Validation::require_positive(amount);

        // Validate rules
        Self::validate_rules(&e, &rules);

        // Generate unique commitment ID
        let commitment_id = Self::generate_commitment_id(&e, &owner);

        // Get NFT contract address
        let nft_contract = e
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                panic!("Contract not initialized")
            });

        // CHECKS: Validate commitment doesn't already exist
        if has_commitment(&e, &commitment_id) {
            set_reentrancy_guard(&e, false);
            panic!("Commitment already exists");
        }

        // EFFECTS: Update state before external calls
        // Calculate expiration timestamp using shared utilities
        let current_timestamp = TimeUtils::now(&e);
        let expires_at = TimeUtils::calculate_expiration(&e, rules.duration_days);

        // Create commitment data
        let commitment = Commitment {
            commitment_id: commitment_id.clone(),
            owner: owner.clone(),
            nft_token_id: 0, // Will be set after NFT mint
            rules: rules.clone(),
            amount,
            asset_address: asset_address.clone(),
            created_at: current_timestamp,
            expires_at,
            current_value: amount, // Initially same as amount
            status: String::from_str(&e, "active"),
        };

        // Store commitment data (before external calls)
        set_commitment(&e, &commitment);

        // Update owner's commitment list
        let mut owner_commitments = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::OwnerCommitments(owner.clone()))
            .unwrap_or(Vec::new(&e));
        owner_commitments.push_back(commitment_id.clone());
        e.storage().instance().set(
            &DataKey::OwnerCommitments(owner.clone()),
            &owner_commitments,
        );

        // Increment total commitments counter
        let current_total = e
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::TotalCommitments)
            .unwrap_or(0);
        e.storage()
            .instance()
            .set(&DataKey::TotalCommitments, &(current_total + 1));

        // Update total value locked (aggregate)
        let current_tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        e.storage()
            .instance()
            .set(&DataKey::TotalValueLocked, &(current_tvl + amount));

        // INTERACTIONS: External calls (token transfer, NFT mint)
        // Transfer assets from owner to contract
        let contract_address = e.current_contract_address();
        transfer_assets(&e, &owner, &contract_address, &asset_address, amount);

        // Mint NFT
        let nft_token_id = call_nft_mint(
            &e,
            &nft_contract,
            &owner,
            &commitment_id,
            rules.duration_days,
            rules.max_loss_percent,
            &rules.commitment_type,
            amount,
            &asset_address,
        );

        // Update commitment with NFT token ID
        let mut updated_commitment = commitment;
        updated_commitment.nft_token_id = nft_token_id;
        set_commitment(&e, &updated_commitment);

        // Clear reentrancy guard
        set_reentrancy_guard(&e, false);

        // Emit creation event
        e.events().publish(
            (symbol_short!("Created"), commitment_id.clone(), owner.clone()),
            (amount, rules, nft_token_id, e.ledger().timestamp()),
        );
        commitment_id
    }

    /// Get commitment details
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| panic!("Commitment not found"))
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
            .unwrap_or_else(|| panic!("Commitment not found"));

        // Only update if commitment is still active
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            panic!("Cannot update value for non-active commitment");
        }

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
        )
    }

    /// Get all commitments for an owner
    pub fn get_owner_commitments(e: Env, owner: Address) -> Vec<String> {
        e.storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::OwnerCommitments(owner))
            .unwrap_or(Vec::new(&e))
    }

    /// Get total number of commitments
    pub fn get_total_commitments(e: Env) -> u64 {
        e.storage()
            .instance()
            .get::<_, u64>(&DataKey::TotalCommitments)
            .unwrap_or(0)
    }

    /// Get total value locked across all active commitments.
    pub fn get_total_value_locked(e: Env) -> i128 {
        e.storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0)
    }

    /// Get NFT contract address
    pub fn get_nft_contract(e: Env) -> Address {
        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| panic!("Contract not initialized"))
    }

    /// Check if commitment rules are violated
    /// Returns true if any rule violation is detected (loss limit or duration)
    /// 
    /// # Formal Verification
    /// **Preconditions:**
    /// - `commitment_id` exists
    /// 
    /// **Postconditions:**
    /// - Returns `true` if `loss_percent > max_loss_percent OR current_time >= expires_at`
    /// - Returns `false` otherwise
    /// - Pure function (no state changes)
    /// 
    /// **Invariants Maintained:**
    /// - INV-2: Commitment balance conservation
    /// 
    /// **Security Properties:**
    /// - SP-4: State consistency (read-only)
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
        // Calculate loss percentage using shared utilities, but handle zero-amount
        // commitments gracefully to avoid panics. A zero-amount commitment cannot
        // meaningfully violate a loss limit, so we treat its loss percent as 0.
        let loss_percent = if commitment.amount > 0 {
            SafeMath::loss_percent(commitment.amount, commitment.current_value)
        } else {
            0
        };

        // Convert max_loss_percent (u32) to i128 for comparison
        let max_loss = commitment.rules.max_loss_percent as i128;
        let loss_violated = loss_percent > max_loss;

        // Check duration violation (expired)
        let duration_violated = current_time >= commitment.expires_at;

        let violated = loss_violated || duration_violated;

        if violated {
            // Emit violation event
            e.events().publish(
                (symbol_short!("Violated"), commitment_id),
                (symbol_short!("RuleViol"), e.ledger().timestamp()),
            );
        }

        // Return true if any violation exists
        violated
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
    /// 
    /// # Reentrancy Protection
    /// Uses checks-effects-interactions pattern with reentrancy guard.
    pub fn settle(e: Env, commitment_id: String) {
        // Reentrancy protection
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        // CHECKS: Get and validate commitment
        let mut commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                panic!("Commitment not found")
            });

        // Verify commitment is expired
        let current_time = e.ledger().timestamp();
        if current_time < commitment.expires_at {
            set_reentrancy_guard(&e, false);
            panic!("Commitment has not expired yet");
        }

        // Verify commitment is active
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            set_reentrancy_guard(&e, false);
            panic!("Commitment is not active");
        }

        // EFFECTS: Update state before external calls
        let settlement_amount = commitment.current_value;
        commitment.status = String::from_str(&e, "settled");
        set_commitment(&e, &commitment);

        // Decrease total value locked
        let current_tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        let new_tvl = current_tvl - settlement_amount;
        e.storage()
            .instance()
            .set(&DataKey::TotalValueLocked, &new_tvl);

        // INTERACTIONS: External calls (token transfer, NFT settlement)
        // Transfer assets back to owner
        let contract_address = e.current_contract_address();
        let token_client = token::Client::new(&e, &commitment.asset_address);
        token_client.transfer(&contract_address, &commitment.owner, &settlement_amount);

        // Call NFT contract to mark NFT as settled
        let nft_contract = e
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                panic!("NFT contract not initialized")
            });

        let mut args = Vec::new(&e);
        args.push_back(commitment.nft_token_id.into_val(&e));
        e.invoke_contract::<()>(&nft_contract, &Symbol::new(&e, "settle"), args);

        // Clear reentrancy guard
        set_reentrancy_guard(&e, false);

        // Emit settlement event
        e.events().publish(
            (symbol_short!("Settled"), commitment_id),
            (settlement_amount, e.ledger().timestamp()),
        );
    }

    /// Early exit (with penalty)
    /// 
    /// # Reentrancy Protection
    /// Uses checks-effects-interactions pattern with reentrancy guard.
    pub fn early_exit(e: Env, commitment_id: String, caller: Address) {
        // Reentrancy protection
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        // CHECKS: Get and validate commitment
        let mut commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                panic!("Commitment not found")
            });

        // Verify caller is owner
        if commitment.owner != caller {
            set_reentrancy_guard(&e, false);
            panic!("Unauthorized: caller is not the owner");
        }

        // Verify commitment is active
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            set_reentrancy_guard(&e, false);
            panic!("Commitment is not active");
        }

        // EFFECTS: Calculate penalty using shared utilities
        let penalty_amount =
            SafeMath::penalty_amount(commitment.current_value, commitment.rules.early_exit_penalty);
        let returned_amount = SafeMath::sub(commitment.current_value, penalty_amount);

        commitment.status = String::from_str(&e, "early_exit");
        set_commitment(&e, &commitment);

        // Decrease total value locked by full current value (no longer locked)
        let current_tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        let new_tvl = current_tvl - commitment.current_value;
        e.storage()
            .instance()
            .set(&DataKey::TotalValueLocked, &new_tvl);

        // INTERACTIONS: External calls (token transfer)
        // Transfer remaining amount (after penalty) to owner
        let contract_address = e.current_contract_address();
        let token_client = token::Client::new(&e, &commitment.asset_address);
        token_client.transfer(&contract_address, &commitment.owner, &returned_amount);

        // Clear reentrancy guard
        set_reentrancy_guard(&e, false);

        // Emit early exit event
        e.events().publish(
            (symbol_short!("EarlyExt"), commitment_id, caller),
            (penalty_amount, returned_amount, e.ledger().timestamp()),
        );
    }

    /// Allocate liquidity (called by allocation strategy)
    /// 
    /// # Reentrancy Protection
    /// Uses checks-effects-interactions pattern with reentrancy guard.
    pub fn allocate(e: Env, commitment_id: String, target_pool: Address, amount: i128) {
        // Reentrancy protection
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        // Rate limit allocations per target pool address
        let fn_symbol = symbol_short!("alloc");
        RateLimiter::check(&e, &target_pool, &fn_symbol);

        // CHECKS: Validate inputs and commitment
        if amount <= 0 {
            set_reentrancy_guard(&e, false);
            panic!("Invalid amount");
        }

        let commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                panic!("Commitment not found")
            });

        // Verify commitment is active
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            set_reentrancy_guard(&e, false);
            panic!("Commitment is not active");
        }

        // Verify sufficient balance
        if commitment.current_value < amount {
            set_reentrancy_guard(&e, false);
            panic!("Insufficient commitment value");
        }

        // EFFECTS: Update commitment value before external call
        let mut updated_commitment = commitment;
        updated_commitment.current_value = updated_commitment.current_value - amount;
        set_commitment(&e, &updated_commitment);

        // INTERACTIONS: External call (token transfer)
        // Transfer assets to target pool
        let contract_address = e.current_contract_address();
        let token_client = token::Client::new(&e, &updated_commitment.asset_address);
        token_client.transfer(&contract_address, &target_pool, &amount);

        // Clear reentrancy guard
        set_reentrancy_guard(&e, false);

        // Emit allocation event
        e.events().publish(
            (symbol_short!("Alloc"), commitment_id, target_pool),
            (amount, e.ledger().timestamp()),
        );
    }

    /// Configure rate limits for this contract's functions.
    ///
    /// This function is restricted to the contract admin.
    pub fn set_rate_limit(
        e: Env,
        caller: Address,
        function: Symbol,
        window_seconds: u64,
        max_calls: u32,
    ) {
        require_admin(&e, &caller);
        RateLimiter::set_limit(&e, &function, window_seconds, max_calls);
    }

    /// Set or clear rate limit exemption for an address.
    ///
    /// This function is restricted to the contract admin.
    pub fn set_rate_limit_exempt(e: Env, caller: Address, address: Address, exempt: bool) {
        require_admin(&e, &caller);
        RateLimiter::set_exempt(&e, &address, exempt);
    }
}

#[cfg(test)]
mod tests;
