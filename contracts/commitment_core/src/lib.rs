#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, log, token, symbol_short, Address, Env, IntoVal, String,
    Symbol, Vec, Map,
};

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

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    NftContract,
    Commitment(String),        // commitment_id -> Commitment
    OwnerCommitments(Address), // owner -> Vec<commitment_id>
    TotalCommitments,          // counter
}

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

#[contract]
pub struct CommitmentCoreContract;

#[contractimpl]
impl CommitmentCoreContract {
    /// Validate commitment rules
    fn validate_rules(e: &Env, rules: &CommitmentRules) {
        // Duration must be > 0
        if rules.duration_days == 0 {
            log!(e, "Invalid duration: {}", rules.duration_days);
            panic!("Invalid duration");
        }

        // Max loss percent must be between 0 and 100
        if rules.max_loss_percent > 100 {
            log!(e, "Invalid max loss percent: {}", rules.max_loss_percent);
            panic!("Invalid max loss percent");
        }

        // Commitment type must be valid
        let valid_types = ["safe", "balanced", "aggressive"];
        let mut is_valid = false;
        for valid_type in valid_types.iter() {
            if rules.commitment_type == String::from_str(e, valid_type) {
                is_valid = true;
                break;
            }
        }
        if !is_valid {
            log!(e, "Invalid commitment type");
            panic!("Invalid commitment type");
        }
    }

    /// Generate unique commitment ID
    fn generate_commitment_id(e: &Env, _owner: &Address) -> String {
        let _counter = e
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
        // Check if already initialized
        if e.storage().instance().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }

        // Store admin and NFT contract address
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage()
            .instance()
            .set(&DataKey::NftContract, &nft_contract);

        // Initialize total commitments counter
        e.storage()
            .instance()
            .set(&DataKey::TotalCommitments, &0u64);
    }

    /// Create a new commitment
    pub fn create_commitment(
        e: Env,
        owner: Address,
        amount: i128,
        asset_address: Address,
        rules: CommitmentRules,
    ) -> String {
        // Validate amount > 0
        if amount <= 0 {
            log!(&e, "Invalid amount: {}", amount);
            panic!("Invalid amount");
        }

        // Validate rules
        Self::validate_rules(&e, &rules);

        // Generate unique commitment ID
        let commitment_id = Self::generate_commitment_id(&e, &owner);

        // Get NFT contract address
        let nft_contract = e
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| panic!("Contract not initialized"));

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

        // Calculate expiration timestamp (current time + duration in days)
        let current_timestamp = e.ledger().timestamp();
        let expires_at = current_timestamp + (rules.duration_days as u64 * 24 * 60 * 60); // days to seconds

        // Create commitment data
        let commitment = Commitment {
            commitment_id: commitment_id.clone(),
            owner: owner.clone(),
            nft_token_id,
            rules: rules.clone(),
            amount,
            asset_address: asset_address.clone(),
            created_at: current_timestamp,
            expires_at,
            current_value: amount, // Initially same as amount
            status: String::from_str(&e, "active"),
        };

        // Store commitment data
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

    /// Get admin address
    pub fn get_admin(e: Env) -> Address {
        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Contract not initialized"))
    }

    /// Get NFT contract address
    pub fn get_nft_contract(e: Env) -> Address {
        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| panic!("Contract not initialized"))
    }

    /// Update commitment value (called by allocation logic)
    pub fn update_value(e: Env, commitment_id: String, new_value: i128) {
        // TODO: Verify caller is authorized (allocation contract)
        // TODO: Update current_value
        // TODO: Check if max_loss_percent is violated
        
        // Emit value update event
        e.events().publish(
            (symbol_short!("ValUpd"), commitment_id),
            (new_value, e.ledger().timestamp()),
        );
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
    pub fn settle(e: Env, commitment_id: String) {
        // TODO: Verify commitment is expired
        // TODO: Calculate final settlement amount
        let settlement_amount: i128 = 0;
        // TODO: Transfer assets back to owner
        // TODO: Mark commitment as settled
        // TODO: Call NFT contract to mark NFT as settled
        
        // Emit settlement event
        e.events().publish(
            (symbol_short!("Settled"), commitment_id),
            (settlement_amount, e.ledger().timestamp()),
        );
    }

    /// Early exit (with penalty)
    pub fn early_exit(e: Env, commitment_id: String, caller: Address) {
        // TODO: Verify caller is owner
        // TODO: Calculate penalty
        let penalty_amount: i128 = 0;
        let returned_amount: i128 = 0;
        // TODO: Transfer remaining amount (after penalty) to owner
        // TODO: Mark commitment as early_exit
        
        // Emit early exit event
        e.events().publish(
            (symbol_short!("EarlyExt"), commitment_id, caller),
            (penalty_amount, returned_amount, e.ledger().timestamp()),
        );
    }

    /// Allocate liquidity (called by allocation strategy)
    pub fn allocate(e: Env, commitment_id: String, target_pool: Address, amount: i128) {
        // TODO: Verify caller is authorized allocation contract
        // TODO: Verify commitment is active
        // TODO: Transfer assets to target pool
        // TODO: Record allocation
        
        // Emit allocation event
        e.events().publish(
            (symbol_short!("Alloc"), commitment_id, target_pool),
            (amount, e.ledger().timestamp()),
        );
    }
}

#[cfg(test)]
mod tests;
