#![no_std]

use shared_utils::{
    emit_error_event, EmergencyControl, RateLimiter, SafeMath, TimeUtils, Validation,
};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, log, symbol_short, token, Address, Env,
    IntoVal, String, Symbol, Vec,
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
    ReentrancyDetected = 11,
    NotActive = 12,
    InvalidStatus = 13,
    NotInitialized = 14,
    NotExpired = 15,
}

impl CommitmentError {
    /// Human-readable message for debugging and error events.
    pub fn message(&self) -> &'static str {
        match self {
            CommitmentError::InvalidDuration => "Invalid duration: must be greater than zero",
            CommitmentError::InvalidMaxLossPercent => "Invalid max loss: must be 0-100",
            CommitmentError::InvalidCommitmentType => "Invalid commitment type",
            CommitmentError::InvalidAmount => "Invalid amount: must be greater than zero",
            CommitmentError::InsufficientBalance => "Insufficient balance",
            CommitmentError::TransferFailed => "Token transfer failed",
            CommitmentError::MintingFailed => "NFT minting failed",
            CommitmentError::CommitmentNotFound => "Commitment not found",
            CommitmentError::Unauthorized => "Unauthorized: caller not allowed",
            CommitmentError::AlreadyInitialized => "Contract already initialized",
            CommitmentError::ReentrancyDetected => "Reentrancy detected",
            CommitmentError::NotActive => "Commitment is not active",
            CommitmentError::InvalidStatus => "Invalid commitment status for this operation",
            CommitmentError::NotInitialized => "Contract not initialized",
            CommitmentError::NotExpired => "Commitment has not expired yet",
        }
    }
}

/// Emit error event and panic with standardized message (for indexers and UX).
fn fail(e: &Env, err: CommitmentError, context: &str) -> ! {
    emit_error_event(e, err as u32, context);
    panic!("{}", err.message());
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
    ReentrancyGuard,           // reentrancy protection flag
    TotalValueLocked,          // aggregate value locked across active commitments
}

/// Transfer assets from owner to contract
fn transfer_assets(e: &Env, from: &Address, to: &Address, asset_address: &Address, amount: i128) {
    let token_client = token::Client::new(e, asset_address);

    // Check balance first
    let balance = token_client.balance(from);
    if balance < amount {
        log!(e, "Insufficient balance: {} < {}", balance, amount);
        fail(e, CommitmentError::InsufficientBalance, "transfer_assets");
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
    e.storage().instance().set(
        &DataKey::Commitment(commitment.commitment_id.clone()),
        commitment,
    );
}

fn has_commitment(e: &Env, commitment_id: &String) -> bool {
    e.storage()
        .instance()
        .has(&DataKey::Commitment(commitment_id.clone()))
}

/// Reentrancy protection helpers
fn require_no_reentrancy(e: &Env) {
    let guard: bool = e
        .storage()
        .instance()
        .get::<_, bool>(&DataKey::ReentrancyGuard)
        .unwrap_or(false);

    if guard {
        fail(
            e,
            CommitmentError::ReentrancyDetected,
            "require_no_reentrancy",
        );
    }
}

fn set_reentrancy_guard(e: &Env, value: bool) {
    e.storage()
        .instance()
        .set(&DataKey::ReentrancyGuard, &value);
}

/// Require that the caller is the admin stored in this contract.
fn require_admin(e: &Env, caller: &Address) {
    caller.require_auth();
    let admin = e
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| fail(e, CommitmentError::NotInitialized, "require_admin"));
    if *caller != admin {
        fail(e, CommitmentError::Unauthorized, "require_admin");
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
    /// Optimized: Uses counter to create unique ID efficiently
    fn generate_commitment_id(e: &Env, counter: u64) -> String {
        // OPTIMIZATION: Use counter directly as string to minimize allocations
        // This is more gas-efficient than string concatenation
        let mut buf = [0u8; 32];
        let prefix = b"c_";
        buf[0] = prefix[0];
        buf[1] = prefix[1];

        // Convert counter to string representation
        let mut n = counter;
        let mut i = 2;
        if n == 0 {
            buf[i] = b'0';
            i += 1;
        } else {
            let mut digits = [0u8; 20];
            let mut digit_count = 0;
            while n > 0 {
                digits[digit_count] = (n % 10) as u8 + b'0';
                n /= 10;
                digit_count += 1;
            }
            // Reverse digits
            for j in 0..digit_count {
                buf[i] = digits[digit_count - 1 - j];
                i += 1;
            }
        }

        String::from_str(e, core::str::from_utf8(&buf[..i]).unwrap_or("c_0"))
    }

    /// Initialize the core commitment contract
    pub fn initialize(e: Env, admin: Address, nft_contract: Address) {
        // Check if already initialized
        if e.storage().instance().has(&DataKey::Admin) {
            fail(&e, CommitmentError::AlreadyInitialized, "initialize");
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

        // Initialize total value locked counter
        e.storage()
            .instance()
            .set(&DataKey::TotalValueLocked, &0i128);
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
        EmergencyControl::require_not_emergency(&e);

        // Rate limit: per-owner commitment creation
        let fn_symbol = symbol_short!("create");
        RateLimiter::check(&e, &owner, &fn_symbol);

        // Validate amount > 0 using shared utilities
        Validation::require_positive(amount);

        // Validate rules
        Self::validate_rules(&e, &rules);

        // OPTIMIZATION: Read both counters and NFT contract once to minimize storage operations
        let (current_total, current_tvl, nft_contract) = {
            let total = e
                .storage()
                .instance()
                .get::<_, u64>(&DataKey::TotalCommitments)
                .unwrap_or(0);
            let tvl = e
                .storage()
                .instance()
                .get::<_, i128>(&DataKey::TotalValueLocked)
                .unwrap_or(0);
            let nft = e
                .storage()
                .instance()
                .get::<_, Address>(&DataKey::NftContract)
                .unwrap_or_else(|| {
                    set_reentrancy_guard(&e, false);
                    fail(&e, CommitmentError::NotInitialized, "create_commitment")
                });
            (total, tvl, nft)
        };

        // Generate unique commitment ID using counter
        let commitment_id = Self::generate_commitment_id(&e, current_total);

        // CHECKS: Validate commitment doesn't already exist
        if has_commitment(&e, &commitment_id) {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::InvalidStatus, "create_commitment");
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

        // OPTIMIZATION: Increment both counters using already-read values
        e.storage()
            .instance()
            .set(&DataKey::TotalCommitments, &(current_total + 1));
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
            (
                symbol_short!("Created"),
                commitment_id.clone(),
                owner.clone(),
            ),
            (amount, rules, nft_token_id, e.ledger().timestamp()),
        );
        commitment_id
    }

    /// Get commitment details
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| fail(&e, CommitmentError::CommitmentNotFound, "get_commitment"))
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

    /// Get admin address
    pub fn get_admin(e: Env) -> Address {
        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .unwrap_or_else(|| fail(&e, CommitmentError::NotInitialized, "get_admin"))
    }

    /// Get NFT contract address
    pub fn get_nft_contract(e: Env) -> Address {
        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| fail(&e, CommitmentError::NotInitialized, "get_nft_contract"))
    }

    /// Update commitment value (called by allocation logic or oracle-fed keeper).
    /// Persists new_value to commitment.current_value and updates TotalValueLocked.
    pub fn update_value(e: Env, commitment_id: String, new_value: i128) {
        // Global per-function rate limit (per contract instance)
        let fn_symbol = symbol_short!("upd_val");
        let contract_address = e.current_contract_address();
        RateLimiter::check(&e, &contract_address, &fn_symbol);
        EmergencyControl::require_not_emergency(&e);

        Validation::require_non_negative(new_value);

        let mut commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| fail(&e, CommitmentError::CommitmentNotFound, "update_value"));

        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            fail(&e, CommitmentError::NotActive, "update_value");
        }

        let old_value = commitment.current_value;
        commitment.current_value = new_value;
        set_commitment(&e, &commitment);

        // Adjust TotalValueLocked: TVL -= old_value, TVL += new_value
        let current_tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        let new_tvl = current_tvl - old_value + new_value;
        e.storage()
            .instance()
            .set(&DataKey::TotalValueLocked, &new_tvl);

        e.events().publish(
            (symbol_short!("ValUpd"), commitment_id),
            (new_value, e.ledger().timestamp()),
        );
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
            .unwrap_or_else(|| fail(&e, CommitmentError::CommitmentNotFound, "check_violations"));

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
    pub fn get_violation_details(e: Env, commitment_id: String) -> (bool, bool, bool, i128, u64) {
        let commitment = read_commitment(&e, &commitment_id).unwrap_or_else(|| {
            fail(
                &e,
                CommitmentError::CommitmentNotFound,
                "get_violation_details",
            )
        });

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

        (
            has_violations,
            loss_violated,
            duration_violated,
            loss_percent,
            time_remaining,
        )
    }

    /// Settle commitment at maturity
    ///
    /// # Reentrancy Protection
    /// Uses checks-effects-interactions pattern with reentrancy guard.
    pub fn settle(e: Env, commitment_id: String) {
        // Reentrancy protection
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);
        EmergencyControl::require_not_emergency(&e);

        // CHECKS: Get and validate commitment
        let mut commitment = read_commitment(&e, &commitment_id).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::CommitmentNotFound, "settle")
        });

        // Verify commitment is expired
        let current_time = e.ledger().timestamp();
        if current_time < commitment.expires_at {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::NotExpired, "settle");
        }

        // Verify commitment is active
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::NotActive, "settle");
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
                fail(&e, CommitmentError::NotInitialized, "settle")
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

    pub fn early_exit(e: Env, commitment_id: String, caller: Address) {
        // Reentrancy protection
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);
        EmergencyControl::require_not_emergency(&e);

        // CHECKS: Get and validate commitment
        let mut commitment = read_commitment(&e, &commitment_id).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::CommitmentNotFound, "early_exit")
        });

        // Verify caller is owner
        caller.require_auth();
        if commitment.owner != caller {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::Unauthorized, "early_exit");
        }

        // Verify commitment is active
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::NotActive, "early_exit");
        }

        // EFFECTS: Calculate penalty using shared utilities
        let penalty_amount = SafeMath::penalty_amount(
            commitment.current_value,
            commitment.rules.early_exit_penalty,
        );
        let returned_amount = SafeMath::sub(commitment.current_value, penalty_amount);

        // Update commitment status to early_exit
        commitment.status = String::from_str(&e, "early_exit");
        commitment.current_value = 0; // All value has been distributed
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

        if returned_amount > 0 {
            token_client.transfer(&contract_address, &commitment.owner, &returned_amount);
        }

        // Call NFT contract to update NFT status (mark as inactive/early_exited)
        let nft_contract = e
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(&e, CommitmentError::NotInitialized, "early_exit")
            });

        // Call settle on NFT to mark it as inactive
        let mut args = Vec::new(&e);
        args.push_back(commitment.nft_token_id.into_val(&e));
        e.invoke_contract::<()>(&nft_contract, &Symbol::new(&e, "settle"), args);

        // Clear reentrancy guard
        set_reentrancy_guard(&e, false);

        // Emit early exit event with detailed information
        e.events().publish(
            (
                symbol_short!("EarlyExt"),
                commitment_id.clone(),
                caller.clone(),
            ),
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
        EmergencyControl::require_not_emergency(&e);

        // Rate limit allocations per target pool address
        let fn_symbol = symbol_short!("alloc");
        RateLimiter::check(&e, &target_pool, &fn_symbol);

        // CHECKS: Validate inputs and commitment
        if amount <= 0 {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::InvalidAmount, "allocate");
        }

        let commitment = read_commitment(&e, &commitment_id).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::CommitmentNotFound, "allocate")
        });

        // Verify commitment is active
        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::NotActive, "allocate");
        }

        // Verify sufficient balance
        if commitment.current_value < amount {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::InsufficientBalance, "allocate");
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

    // ========================================================================
    // Emergency Functions (Issue #62)
    // ========================================================================

    /// Toggle emergency mode (admin only)
    pub fn set_emergency_mode(e: Env, caller: Address, enabled: bool) {
        require_admin(&e, &caller);
        EmergencyControl::set_emergency_mode(&e, enabled);
    }

    /// Check if in emergency mode
    pub fn is_emergency_mode(e: Env) -> bool {
        EmergencyControl::is_emergency_mode(&e)
    }

    /// Emergency withdrawal of funds (admin only)
    /// This allows rescuing funds from the contract to a safe address if needed.
    pub fn emergency_withdraw(
        e: Env,
        caller: Address,
        asset_address: Address,
        to: Address,
        amount: i128,
    ) {
        require_admin(&e, &caller);
        EmergencyControl::require_emergency(&e);

        let token_client = token::Client::new(&e, &asset_address);
        token_client.transfer(&e.current_contract_address(), &to, &amount);

        e.events().publish(
            (symbol_short!("EmgWthdr"), asset_address, to),
            (amount, e.ledger().timestamp()),
        );
    }

    /// Force settle a commitment in emergency (admin only)
    /// This bypasses maturity checks and fees.
    pub fn emergency_settle(e: Env, caller: Address, commitment_id: String) {
        require_admin(&e, &caller);
        EmergencyControl::require_emergency(&e);

        let mut commitment =
            read_commitment(&e, &commitment_id).unwrap_or_else(|| panic!("Commitment not found"));

        // Mark as settled
        commitment.status = String::from_str(&e, "settled");
        let settlement_amount = commitment.current_value;
        commitment.current_value = 0;
        set_commitment(&e, &commitment);

        // Adjust TVL
        let current_tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        let new_tvl = current_tvl - settlement_amount;
        e.storage()
            .instance()
            .set(&DataKey::TotalValueLocked, &new_tvl);

        // Transfer funds back to owner
        let token_client = token::Client::new(&e, &commitment.asset_address);
        token_client.transfer(
            &e.current_contract_address(),
            &commitment.owner,
            &settlement_amount,
        );

        // Update NFT
        let nft_contract = e
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| panic!("NFT contract not initialized"));
        let mut args = Vec::new(&e);
        args.push_back(commitment.nft_token_id.into_val(&e));
        e.invoke_contract::<()>(&nft_contract, &Symbol::new(&e, "settle"), args);

        e.events().publish(
            (symbol_short!("EmgSettl"), commitment_id),
            (settlement_amount, e.ledger().timestamp()),
        );
    }

    /// Change commitment parameters in emergency (admin only)
    /// This allows fixing stuck commitments or adjusting state during recovery.
    pub fn emergency_update_commitment(
        e: Env,
        caller: Address,
        commitment_id: String,
        new_value: i128,
        new_status: String,
        new_expires_at: u64,
    ) {
        require_admin(&e, &caller);
        EmergencyControl::require_emergency(&e);

        let mut commitment =
            read_commitment(&e, &commitment_id).unwrap_or_else(|| panic!("Commitment not found"));

        // Adjust TVL first
        let current_tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        let new_tvl = current_tvl - commitment.current_value + new_value;
        e.storage()
            .instance()
            .set(&DataKey::TotalValueLocked, &new_tvl);

        commitment.current_value = new_value;
        commitment.status = new_status;
        commitment.expires_at = new_expires_at;

        set_commitment(&e, &commitment);

        e.events().publish(
            (symbol_short!("EmgUpd"), commitment_id),
            (e.ledger().timestamp(),),
        );
    }
}

mod emergency_tests;
#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "benchmark"))]
mod benchmarks;
