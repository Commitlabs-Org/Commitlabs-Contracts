#![no_std]

//! Core commitment lifecycle contract.
//!
//! This contract owns the primary state machine for commitments and coordinates the
//! highest-risk cross-contract calls in the protocol:
//! - outbound writes to `commitment_nft` during create, settle, and early-exit flows
//! - inbound read-only queries from `attestation_engine` through `get_commitment`
//!
//! # Call-graph threat review
//! The end-to-end review for the `commitment_core <-> commitment_nft <-> attestation_engine`
//! call graph lives in:
//! [`docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md#core-nft-attestation-call-graph`](../../../docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md#core-nft-attestation-call-graph)

use shared_utils::{
    emit_error_event, fees, EmergencyControl, Pausable, RateLimiter, SafeMath, TimeUtils,
    Validation,
};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, log, symbol_short, token, Address, Env,
    IntoVal, String, Symbol, Vec,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
/// Standardized errors for commitment lifecycle operations.
pub enum CommitmentError {
    /// Duration must be greater than zero
    InvalidDuration = 1,
    /// Max loss must be between 0 and 100
    InvalidMaxLossPercent = 2,
    /// Commitment type must be 'safe', 'balanced', or 'aggressive'
    InvalidCommitmentType = 3,
    /// Amount must be positive
    InvalidAmount = 4,
    /// User balance too low for commitment amount
    InsufficientBalance = 5,
    /// Underlying token transfer failed
    TransferFailed = 6,
    /// NFT contract call failed
    MintingFailed = 7,
    /// No commitment record found for the provided ID
    CommitmentNotFound = 8,
    /// Caller lacks sufficient authority or role
    Unauthorized = 9,
    /// Contract is already initialized
    AlreadyInitialized = 10,
    /// Commitment already in a terminal state
    AlreadySettled = 11,
    /// Reentrancy detected during cross-contract interaction
    ReentrancyDetected = 12,
    /// Operation requires an 'active' commitment
    NotActive = 13,
    /// Current commitment status prevents this transition
    InvalidStatus = 14,
    /// Contract must be initialized before use
    NotInitialized = 15,
    /// Settle called before maturity
    NotExpired = 16,
    /// Value update violates rule constraints
    ValueUpdateViolation = 17,
    /// Caller is not an authorized updater for this commitment
    NotAuthorizedUpdater = 18,
    /// Zero address is not allowed as a parameter
    ZeroAddress = 19,
    /// Duration would cause expires_at to overflow u64
    ExpirationOverflow = 20,
    /// Invalid fee basis points (must be 0-10000)
    InvalidFeeBps = 21,
    /// Fee recipient not set; cannot withdraw
    FeeRecipientNotSet = 22,
    /// Insufficient collected fees to withdraw
    InsufficientFees = 23,
}

impl CommitmentError {
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
            CommitmentError::AlreadySettled => "Commitment already settled",
            CommitmentError::ReentrancyDetected => "Reentrancy detected",
            CommitmentError::NotActive => "Commitment is not active",
            CommitmentError::InvalidStatus => "Invalid commitment status for this operation",
            CommitmentError::NotInitialized => "Contract not initialized",
            CommitmentError::NotExpired => "Commitment has not expired yet",
            CommitmentError::ValueUpdateViolation => "Commitment has value update violation",
            CommitmentError::NotAuthorizedUpdater => "Commitment has not auth updater",
            CommitmentError::ZeroAddress => "Zero address is not allowed",
            CommitmentError::ExpirationOverflow => "Duration would cause expiration timestamp overflow",
            CommitmentError::InvalidFeeBps => "Invalid fee basis points: must be 0-10000",
            CommitmentError::FeeRecipientNotSet => "Fee recipient not set; cannot withdraw",
            CommitmentError::InsufficientFees => "Insufficient collected fees to withdraw",
        }
    }
}

fn fail(e: &Env, err: CommitmentError, context: &str) -> ! {
    emit_error_event(e, err as u32, context);
    panic!("{}", err.message());
}

#[contracttype]
#[derive(Clone)]
pub struct CommitmentSettledEvent {
    pub commitment_id: String,
    pub owner: Address,
    pub settlement_amount: i128,
    pub timestamp: u64,
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
/// Rules governing a commitment, including risk parameters and penalties.
///
/// ### Commitment Types Semantics:
/// - **Safe**: Low risk. Max loss ≤ 10%, Early exit penalty ≥ 15%. Target: Stable yield pools.
/// - **Balanced**: Medium risk. Max loss ≤ 30%, Early exit penalty ≥ 10%. Target: Mixed yield/growth pools.
/// - **Aggressive**: High risk. Max loss ≤ 100%, Early exit penalty ≥ 5%. Target: High-yield/volatile pools.
pub struct CommitmentRules {
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String, 
    pub early_exit_penalty: u32,
    pub min_fee_threshold: i128,
    pub grace_period_days: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
/// Canonical commitment record.
///
/// ### Storage Note:
/// Stored in instance storage keyed by `DataKey::Commitment(id)`.
pub struct Commitment {
    /// Human-readable unique ID (e.g. "c_123")
    pub commitment_id: String,
    /// Address allowed to settle or exit early
    pub owner: Address,
    /// Token ID of the paired NFT in commitment_nft
    pub nft_token_id: u32,
    /// Rule set applied at creation
    pub rules: CommitmentRules,
    /// Initial locked amount (net of creation fees)
    pub amount: i128,
    /// Underlying asset contract address
    pub asset_address: Address,
    /// Ledger timestamp at creation
    pub created_at: u64,
    /// Ledger timestamp at maturity
    pub expires_at: u64,
    /// Current estimated value in underlying asset
    pub current_value: i128,
    /// Lifecycle status: active, settled, early_exit, violated
    pub status: String, 
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    NftContract,
    AllocationContract,
    Commitment(String),
    OwnerCommitments(Address),
    TotalCommitments,
    ReentrancyGuard,
    TotalValueLocked,
    AuthorizedAllocator(Address),
    AuthorizedUpdaters,
    /// All commitment IDs for time-range queries (analytics). Appended on create.
    AllCommitmentIds,
    /// Fee recipient (protocol treasury) for fee withdrawals
    FeeRecipient,
    /// Creation fee rate in basis points (0-10000)
    CreationFeeBps,
    /// Collected fees per asset (asset -> i128)
    CollectedFees(Address),
}

// --- Internal Helpers ---

fn is_zero_address(e: &Env, address: &Address) -> bool {
    let zero_str = String::from_str(e, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF");
    let zero_addr = Address::from_string(&zero_str);
    address == &zero_addr
}

fn check_sufficient_balance(e: &Env, owner: &Address, asset_address: &Address, amount: i128) {
    let token_client = token::Client::new(e, asset_address);
    let balance = token_client.balance(owner);
    if balance < amount {
        log!(e, "Insufficient balance: {} < {}", balance, amount);
        fail(e, CommitmentError::InsufficientBalance, "check_sufficient_balance");
    }
}

fn transfer_assets(e: &Env, from: &Address, to: &Address, asset_address: &Address, amount: i128) {
    let token_client = token::Client::new(e, asset_address);
    token_client.transfer(from, to, &amount);
}

/// Call the NFT contract mint function.
///
/// # Arguments
/// * `nft_contract` - Address of the commitment_nft contract
/// * `owner` - Address to receive the NFT
/// * `commitment_id` - ID of the commitment
/// * `duration_days` - Duration of the commitment
/// * `max_loss_percent` - Risk parameter
/// * `commitment_type` - String identifier for rule set
/// * `initial_amount` - Initial value locked
/// * `asset_address` - Token address
/// * `early_exit_penalty` - Penalty basis points
///
/// # Returns
/// The u32 token ID of the minted NFT.
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
    early_exit_penalty: u32,
) -> u32 {
    let caller = e.current_contract_address();
    let mut args = Vec::new(e);
    args.push_back(caller.into_val(e));
    args.push_back(owner.clone().into_val(e));
    args.push_back(commitment_id.clone().into_val(e));
    args.push_back(duration_days.into_val(e));
    args.push_back(max_loss_percent.into_val(e));
    args.push_back(commitment_type.clone().into_val(e));
    args.push_back(initial_amount.into_val(e));
    args.push_back(asset_address.clone().into_val(e));
    args.push_back(early_exit_penalty.into_val(e));

    e.invoke_contract::<u32>(nft_contract, &Symbol::new(e, "mint"), args)
}

fn read_commitment(e: &Env, commitment_id: &String) -> Option<Commitment> {
    e.storage().instance().get::<_, Commitment>(&DataKey::Commitment(commitment_id.clone()))
}

fn set_commitment(e: &Env, commitment: &Commitment) {
    e.storage().instance().set(&DataKey::Commitment(commitment.commitment_id.clone()), commitment);
}

fn has_commitment(e: &Env, commitment_id: &String) -> bool {
    e.storage().instance().has(&DataKey::Commitment(commitment_id.clone()))
}

fn require_no_reentrancy(e: &Env) {
    if e.storage().instance().get::<_, bool>(&DataKey::ReentrancyGuard).unwrap_or(false) {
        fail(e, CommitmentError::ReentrancyDetected, "require_no_reentrancy");
    }
}

fn set_reentrancy_guard(e: &Env, value: bool) {
    e.storage().instance().set(&DataKey::ReentrancyGuard, &value);
}

fn require_admin(e: &Env, caller: &Address) {
    caller.require_auth();
    let admin = e.storage().instance().get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| fail(e, CommitmentError::NotInitialized, "require_admin"));
    if *caller != admin {
        fail(e, CommitmentError::Unauthorized, "require_admin");
    }
}

fn add_authorized_updater(e: &Env, updater: &Address) {
    let mut updaters: Vec<Address> = e.storage().instance().get::<_, Vec<Address>>(&DataKey::AuthorizedUpdaters).unwrap_or(Vec::new(e));
    if !updaters.contains(updater) {
        updaters.push_back(updater.clone());
        e.storage().instance().set(&DataKey::AuthorizedUpdaters, &updaters);
    }
}

fn remove_authorized_updater(e: &Env, updater: &Address) {
    let mut updaters: Vec<Address> = e.storage().instance().get::<_, Vec<Address>>(&DataKey::AuthorizedUpdaters).unwrap_or(Vec::new(e));
    if let Some(idx) = updaters.iter().position(|a| a == *updater) {
        updaters.remove(idx as u32);
        e.storage().instance().set(&DataKey::AuthorizedUpdaters, &updaters);
    }
}

fn remove_from_owner_commitments(e: &Env, owner: &Address, commitment_id: &String) {
    let mut commitments: Vec<String> = e.storage().instance().get::<_, Vec<String>>(&DataKey::OwnerCommitments(owner.clone())).unwrap_or(Vec::new(e));
    if let Some(idx) = commitments.iter().position(|id| id == *commitment_id) {
        commitments.remove(idx as u32);
        e.storage().instance().set(&DataKey::OwnerCommitments(owner.clone()), &commitments);
    }
}

#[contract]
/// Main protocol contract for commitment state transitions and asset custody.
///
/// Security-sensitive behavior:
/// - holds user assets during the active commitment lifecycle
/// - calls `commitment_nft` to mirror commitment state into NFT state
/// - serves canonical commitment reads to `attestation_engine`
///
/// Threat review reference:
/// [`docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md#core-nft-attestation-call-graph`](../../../docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md#core-nft-attestation-call-graph)
pub struct CommitmentCoreContract;

#[contractimpl]
impl CommitmentCoreContract {
    fn validate_rules(e: &Env, rules: &CommitmentRules) {
        Validation::require_valid_duration(rules.duration_days);
        Validation::require_valid_percent(rules.max_loss_percent);
        Validation::require_valid_commitment_type(
            e,
            &rules.commitment_type,
            &["safe", "balanced", "aggressive"],
        );

        // Enforce type-specific constraints
        if rules.commitment_type == String::from_str(e, "safe") {
            if rules.max_loss_percent > 10 {
                panic!("Safe type: max_loss_percent must be <= 10");
            }
            if rules.early_exit_penalty < 15 {
                panic!("Safe type: early_exit_penalty must be >= 15");
            }
        } else if rules.commitment_type == String::from_str(e, "balanced") {
            if rules.max_loss_percent > 30 {
                panic!("Balanced type: max_loss_percent must be <= 30");
            }
            if rules.early_exit_penalty < 10 {
                panic!("Balanced type: early_exit_penalty must be >= 10");
            }
        } else if rules.commitment_type == String::from_str(e, "aggressive") {
            if rules.early_exit_penalty < 5 {
                panic!("Aggressive type: early_exit_penalty must be >= 5");
            }
        }
    }

    fn generate_commitment_id(e: &Env, counter: u64) -> String {
        let mut buf = [0u8; 32];
        buf[0] = b'c'; buf[1] = b'_';
        let mut n = counter;
        let mut i = 2;
        if n == 0 { buf[i] = b'0'; i += 1; } else {
            let mut digits = [0u8; 20];
            let mut count = 0;
            while n > 0 { digits[count] = (n % 10) as u8 + b'0'; n /= 10; count += 1; }
            for j in 0..count { buf[i] = digits[count - 1 - j]; i += 1; }
        }
        String::from_str(e, core::str::from_utf8(&buf[..i]).unwrap_or("c_0"))
    }

    /// Initialize the core contract with its admin and linked NFT contract.
    ///
    /// # Arguments
    /// * `admin` - Primary authority allowed to manage configuration and emergency modes
    /// * `nft_contract` - Address of the `commitment_nft` contract used for mirroring state
    ///
    /// # Security
    /// - One-time use: Reverts if already initialized
    /// - Establishes trust boundary: Only the provided `nft_contract` is trusted for mint/settle
    pub fn initialize(e: Env, admin: Address, nft_contract: Address) {
        if e.storage().instance().has(&DataKey::Admin) {
            fail(&e, CommitmentError::AlreadyInitialized, "initialize");
        }

        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage().instance().set(&DataKey::NftContract, &nft_contract);
        e.storage().instance().set(&DataKey::TotalCommitments, &0u64);
        e.storage().instance().set(&DataKey::TotalValueLocked, &0i128);
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedUpdaters, &Vec::<Address>::new(&e));
        e.storage()
            .instance()
            .set(&DataKey::AllCommitmentIds, &Vec::<String>::new(&e));
        e.storage().instance().set(&DataKey::ReentrancyGuard, &false);
        e.storage().instance().set(&Pausable::PAUSED_KEY, &false);
        EmergencyControl::set_emergency_mode(&e, false);
    }

    /// Create a new commitment, transfer assets into custody, and mint the paired NFT.
    ///
    /// # Arguments
    /// * `owner` - Address that will own the commitment and NFT
    /// * `amount` - Gross amount of tokens to commit (before creation fees)
    /// * `asset_address` - Address of the token contract
    /// * `rules` - Commitment rules (duration, risk, penalties)
    ///
    /// # Returns
    /// The generated unique `commitment_id` (String).
    ///
    /// # Security
    /// - Reentrancy: Uses reentrancy guard
    /// - Auth: Requires `owner.require_auth()`
    /// - Consistency: Atomic transfer and NFT minting. Reverts all on failure.
    /// - Fees: Deducts protocol creation fee from `amount`
    pub fn create_commitment(
        e: Env,
        owner: Address,
        amount: i128,
        asset_address: Address,
        rules: CommitmentRules,
    ) -> String {
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);
        Pausable::require_not_paused(&e);
        EmergencyControl::require_not_emergency(&e);
        owner.require_auth();
        if is_zero_address(&e, &owner) {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::ZeroAddress, "create");
        }
        RateLimiter::check(&e, &owner, &symbol_short!("create"));
        Validation::require_positive(amount);
        Self::validate_rules(&e, &rules);
        check_sufficient_balance(&e, &owner, &asset_address, amount);

        let expires_at = TimeUtils::checked_calculate_expiration(&e, rules.duration_days)
            .unwrap_or_else(|| { set_reentrancy_guard(&e, false); fail(&e, CommitmentError::ExpirationOverflow, "create") });

        // Calculate creation fee and net amount
        let creation_fee_bps: u32 = e
            .storage()
            .instance()
            .get(&DataKey::CreationFeeBps)
            .unwrap_or(0);
        let creation_fee = if creation_fee_bps > 0 {
            fees::fee_from_bps(amount, creation_fee_bps)
        } else {
            0
        };
        let net_amount = amount - creation_fee;

        let current_total = e.storage().instance().get::<_, u64>(&DataKey::TotalCommitments).unwrap_or(0);
        let nft_contract = e.storage().instance().get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| { set_reentrancy_guard(&e, false); fail(&e, CommitmentError::NotInitialized, "create") });

        let commitment_id = Self::generate_commitment_id(&e, current_total);
        let commitment = Commitment {
            commitment_id: commitment_id.clone(),
            owner: owner.clone(),
            nft_token_id: 0,
            rules: rules.clone(),
            amount: net_amount,
            asset_address: asset_address.clone(),
            created_at: TimeUtils::now(&e),
            expires_at,
            current_value: net_amount,
            status: String::from_str(&e, "active"),
        };

        set_commitment(&e, &commitment);
        let mut owner_commitments = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::OwnerCommitments(owner.clone()))
            .unwrap_or(Vec::new(&e));
        owner_commitments.push_back(commitment_id.clone());
        e.storage()
            .instance()
            .set(&DataKey::OwnerCommitments(owner.clone()), &owner_commitments);
        e.storage().instance().set(&DataKey::TotalCommitments, &(current_total + 1));
        let tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        e.storage().instance().set(&DataKey::TotalValueLocked, &(tvl + net_amount));

        let mut all_ids = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::AllCommitmentIds)
            .unwrap_or(Vec::new(&e));
        all_ids.push_back(commitment_id.clone());
        e.storage()
            .instance()
            .set(&DataKey::AllCommitmentIds, &all_ids);

        let contract_address = e.current_contract_address();
        transfer_assets(&e, &owner, &contract_address, &asset_address, amount);

        // Add creation fee to collected fees
        if creation_fee > 0 {
            let fee_key = DataKey::CollectedFees(asset_address.clone());
            let current_fees: i128 = e.storage().instance().get(&fee_key).unwrap_or(0);
            e.storage()
                .instance()
                .set(&fee_key, &(current_fees + creation_fee));
        }

        let nft_token_id = call_nft_mint(
            &e,
            &nft_contract,
            &owner,
            &commitment_id,
            rules.duration_days,
            rules.max_loss_percent,
            &rules.commitment_type,
            net_amount,
            &asset_address,
            rules.early_exit_penalty,
        );

        let mut updated_commitment = commitment;
        updated_commitment.nft_token_id = nft_token_id;
        set_commitment(&e, &updated_commitment);
        set_reentrancy_guard(&e, false);

        e.events().publish(
            (symbol_short!("Created"), commitment_id.clone(), owner),
            (amount, rules, nft_token_id, e.ledger().timestamp()),
        );
        commitment_id
    }

    /// Return the canonical commitment record by id.
    ///
    /// # Arguments
    /// * `commitment_id` - Unique ID of the commitment
    ///
    /// # Returns
    /// The `Commitment` struct containing full state details.
    ///
    /// # Security
    /// - View API: Permissionless read; consumed by `attestation_engine`.
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| fail(&e, CommitmentError::CommitmentNotFound, "get_commitment"))
    }

    /// List all commitment IDs owned by the given address.
    pub fn list_commitments_by_owner(e: Env, owner: Address) -> Vec<String> {
        Self::get_owner_commitments(e, owner)
    }

    /// Get all commitment IDs for a specific owner.
    pub fn get_owner_commitments(e: Env, owner: Address) -> Vec<String> {
        e.storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::OwnerCommitments(owner))
            .unwrap_or(Vec::new(&e))
    }

    /// Get the monotonic counter for total commitments ever created.
    pub fn get_total_commitments(e: Env) -> u64 {
        e.storage()
            .instance()
            .get::<_, u64>(&DataKey::TotalCommitments)
            .unwrap_or(0)
    }

    /// Get total value locked (current aggregate of all active commitments).
    pub fn get_total_value_locked(e: Env) -> i128 {
        e.storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0)
    }

    /// Filter commitment IDs created within a specific ledger timestamp range.
    ///
    /// # Arguments
    /// * `from_ts` - Start timestamp (inclusive)
    /// * `to_ts` - End timestamp (inclusive)
    pub fn get_commitments_created_between(e: Env, from_ts: u64, to_ts: u64) -> Vec<String> {
        let all_ids = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::AllCommitmentIds)
            .unwrap_or(Vec::new(&e));
        let mut out = Vec::new(&e);
        for id in all_ids.iter() {
            if let Some(c) = read_commitment(&e, &id) {
                if c.created_at >= from_ts && c.created_at <= to_ts {
                    out.push_back(id.clone());
                }
            }
        }
        out
    }

    /// Get the current administrative address.
    pub fn get_admin(e: Env) -> Address {
        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .unwrap_or_else(|| fail(&e, CommitmentError::NotInitialized, "get_admin"))
    }

    /// Get the linked NFT contract address.
    pub fn get_nft_contract(e: Env) -> Address {
        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| fail(&e, CommitmentError::NotInitialized, "get_nft_contract"))
    }

    /// Pause all state-changing operations (Create, Settle, Exit, Allocate).
    ///
    /// # Authorization
    /// - Admin only
    pub fn pause(e: Env, caller: Address) {
        require_admin(&e, &caller);
        Pausable::pause(&e);
    }

    /// Resume all state-changing operations.
    ///
    /// # Authorization
    /// - Admin only
    pub fn unpause(e: Env, caller: Address) {
        require_admin(&e, &caller);
        Pausable::unpause(&e);
    }

    /// Check if the contract is currently paused.
    pub fn is_paused(e: Env) -> bool {
        Pausable::is_paused(&e)
    }

    /// Authorize a contract (e.g. `allocation_logic`) to call `allocate`.
    ///
    /// # Authorization
    /// - Admin only
    pub fn add_authorized_contract(e: Env, caller: Address, contract_address: Address) {
        require_admin(&e, &caller);
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedAllocator(contract_address.clone()), &true);
        e.events().publish(
            (Symbol::new(&e, "AuthorizedContractAdded"),),
            (contract_address, e.ledger().timestamp()),
        );
    }

    /// Revoke authorization for a contract.
    ///
    /// # Authorization
    /// - Admin only
    pub fn remove_authorized_contract(e: Env, caller: Address, contract_address: Address) {
        require_admin(&e, &caller);
        e.storage()
            .instance()
            .remove(&DataKey::AuthorizedAllocator(contract_address.clone()));
        e.events().publish(
            (Symbol::new(&e, "AuthorizedContractRemoved"),),
            (contract_address, e.ledger().timestamp()),
        );
    }

    /// Check if an address is an authorized allocator (or the admin).
    pub fn is_authorized(e: Env, contract_address: Address) -> bool {
        let admin = e.storage().instance().get::<_, Address>(&DataKey::Admin);
        if let Some(a) = admin {
            if contract_address == a {
                return true;
            }
        }
        e.storage()
            .instance()
            .get::<_, bool>(&DataKey::AuthorizedAllocator(contract_address))
            .unwrap_or(false)
    }

    /// Update the estimated value of a commitment.
    ///
    /// # Arguments
    /// * `commitment_id` - ID of the commitment
    /// * `new_value` - New estimated value in underlying asset
    ///
    /// # Security
    /// - Authenticated update: Only authorized updaters can call (enforced by RateLimiter check on contract)
    /// - TVL impact: Adjusts protocol TVL by `new_value - old_value`
    /// - Violation check: Marks status 'violated' if `new_value` exceeds `max_loss_percent`
    pub fn update_value(e: Env, commitment_id: String, new_value: i128) {
        let fn_symbol = symbol_short!("upd_val");
        let contract_address = e.current_contract_address();
        RateLimiter::check(&e, &contract_address, &fn_symbol);
        Validation::require_non_negative(new_value);

        let mut commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| fail(&e, CommitmentError::CommitmentNotFound, "upd"));
        if commitment.status != String::from_str(&e, "active") {
            fail(&e, CommitmentError::NotActive, "upd");
        }

        let old_value = commitment.current_value;
        commitment.current_value = new_value;

        let loss_percent = if commitment.amount > 0 {
            SafeMath::loss_percent(commitment.amount, new_value)
        } else {
            0
        };
        let violated = loss_percent > commitment.rules.max_loss_percent as i128;

        if violated {
            commitment.status = String::from_str(&e, "violated");
            e.events().publish(
                (symbol_short!("Violated"), commitment_id.clone()),
                (
                    loss_percent,
                    commitment.rules.max_loss_percent,
                    e.ledger().timestamp(),
                ),
            );
        } else {
            e.events().publish(
                (symbol_short!("ValUpd"), commitment_id.clone()),
                (new_value, e.ledger().timestamp()),
            );
        }

        set_commitment(&e, &commitment);
        let tvl = e.storage().instance().get::<_, i128>(&DataKey::TotalValueLocked).unwrap_or(0);
        e.storage().instance().set(&DataKey::TotalValueLocked, &(tvl - old_value + new_value));
    }

    /// Evaluating loss or duration violations for a commitment.
    ///
    /// # Returns
    /// True if commitment is violated (max loss exceeded or expired).
    pub fn check_violations(e: Env, commitment_id: String) -> bool {
        let commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| fail(&e, CommitmentError::CommitmentNotFound, "chk"));
        if commitment.status != String::from_str(&e, "active") {
            return false;
        }

        let current_time = e.ledger().timestamp();
        let loss_percent = if commitment.amount > 0 {
            SafeMath::loss_percent(commitment.amount, commitment.current_value)
        } else {
            0
        };
        let violated = (loss_percent > commitment.rules.max_loss_percent as i128)
            || (current_time >= commitment.expires_at);

        if violated {
            e.events().publish(
                (symbol_short!("Violated"), commitment_id),
                (symbol_short!("RuleViol"), e.ledger().timestamp()),
            );
        }
        violated
    }

    /// Get detailed breakdown of violation status.
    ///
    /// # Returns
    /// Tuple: (has_violations, loss_violated, duration_violated, loss_percent, time_remaining)
    pub fn get_violation_details(e: Env, commitment_id: String) -> (bool, bool, bool, i128, u64) {
        let commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| fail(&e, CommitmentError::CommitmentNotFound, "get_violation_details"));

        let now = e.ledger().timestamp();
        let loss_percent = if commitment.amount > 0 {
            SafeMath::loss_percent(commitment.amount, commitment.current_value)
        } else {
            0
        };
        let loss_violated = loss_percent > commitment.rules.max_loss_percent as i128;
        let duration_violated = now >= commitment.expires_at;
        let has_violations = loss_violated || duration_violated;
        let time_remaining = if now >= commitment.expires_at {
            0
        } else {
            commitment.expires_at - now
        };

        (
            has_violations,
            loss_violated,
            duration_violated,
            loss_percent,
            time_remaining,
        )
    }

    /// Settle an expired commitment and return assets to the owner.
    ///
    /// # Arguments
    /// * `commitment_id` - ID of the expired commitment
    ///
    /// # Security
    /// - Expiry check: Reverts if `ledger.timestamp < expires_at`
    /// - Reentrancy: Uses reentrancy guard
    /// - NFT: Invokes `commitment_nft::settle`
    /// - TVL: Decrements protocol TVL by the settled amount
    pub fn settle(e: Env, commitment_id: String) {
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);
        Pausable::require_not_paused(&e);

        let mut commitment = read_commitment(&e, &commitment_id).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::CommitmentNotFound, "settle")
        });
        let current_time = e.ledger().timestamp();

        if current_time < commitment.expires_at {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::NotExpired, "settle");
        }
        let settled_status = String::from_str(&e, "settled");
        if commitment.status == settled_status {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::AlreadySettled, "settle");
        }
        if commitment.status != String::from_str(&e, "active") {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::NotActive, "settle");
        }

        let settlement_amount = commitment.current_value;
        let owner = commitment.owner.clone();
        commitment.status = settled_status;
        set_commitment(&e, &commitment);
        remove_from_owner_commitments(&e, &owner, &commitment_id);

        let tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        e.storage().instance().set(
            &DataKey::TotalValueLocked,
            &(if tvl > settlement_amount {
                tvl - settlement_amount
            } else {
                0
            }),
        );

        transfer_assets(
            &e,
            &e.current_contract_address(),
            &owner,
            &commitment.asset_address,
            settlement_amount,
        );

        let nft_contract = e
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(&e, CommitmentError::NotInitialized, "settle")
            });
        let mut args = Vec::new(&e);
        args.push_back(e.current_contract_address().into_val(&e));
        args.push_back(commitment.nft_token_id.into_val(&e));
        e.invoke_contract::<()>(&nft_contract, &Symbol::new(&e, "settle"), args);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("Settled"), commitment_id, owner),
            (settlement_amount, e.ledger().timestamp()),
        );
    }

    /// Force-exit an active commitment early, applying the configured penalty.
    ///
    /// # Arguments
    /// * `commitment_id` - ID of the commitment to exit
    /// * `caller` - Must be the current owner. Uses `require_auth`
    ///
    /// # Security
    /// - Reentrancy: Uses reentrancy guard
    /// - Penalty: Calculates and retains penalty via SafeMath
    /// - NFT: Invokes `commitment_nft::mark_inactive`
    /// - TVL: Decrements protocol TVL by the full pre-penalty value
    pub fn early_exit(e: Env, commitment_id: String, caller: Address) {
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);
        Pausable::require_not_paused(&e);

        let mut commitment = read_commitment(&e, &commitment_id).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::CommitmentNotFound, "exit")
        });
        caller.require_auth();
        if commitment.owner != caller {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::Unauthorized, "exit");
        }
        if commitment.status != String::from_str(&e, "active") {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::NotActive, "exit");
        }

        let penalty = SafeMath::penalty_amount(commitment.current_value, commitment.rules.early_exit_penalty);
        let returned = SafeMath::sub(commitment.current_value, penalty);
        let original_val = commitment.current_value;

        // Add penalty to collected fees (protocol revenue)
        if penalty > 0 {
            let fee_key = DataKey::CollectedFees(commitment.asset_address.clone());
            let current_fees: i128 = e.storage().instance().get(&fee_key).unwrap_or(0);
            e.storage()
                .instance()
                .set(&fee_key, &(current_fees + penalty));
        }

        commitment.status = String::from_str(&e, "early_exit");
        commitment.current_value = 0;
        set_commitment(&e, &commitment);

        let tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        e.storage().instance().set(&DataKey::TotalValueLocked, &(tvl - original_val));

        if returned > 0 {
            transfer_assets(
                &e,
                &e.current_contract_address(),
                &commitment.owner,
                &commitment.asset_address,
                returned,
            );
        }

        let nft_contract = e
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(&e, CommitmentError::NotInitialized, "early_exit")
            });

        let mut args = Vec::new(&e);
        args.push_back(e.current_contract_address().into_val(&e));
        args.push_back(commitment.nft_token_id.into_val(&e));
        e.invoke_contract::<()>(&nft_contract, &Symbol::new(&e, "mark_inactive"), args);

        set_reentrancy_guard(&e, false);
        e.events().publish((symbol_short!("EarlyExt"), commitment_id, caller), (penalty, returned, e.ledger().timestamp()));
    }

    /// Add an authorized updater address.
    ///
    /// # Authorization
    /// - Admin only
    pub fn add_updater(e: Env, caller: Address, updater: Address) {
        require_admin(&e, &caller);
        add_authorized_updater(&e, &updater);
    }

    /// Allocate assets from an active commitment to a target pool.
    ///
    /// # Arguments
    /// * `caller` - Must be admin or authorized allocator. Uses `require_auth`
    /// * `commitment_id` - ID of the source commitment
    /// * `target_pool` - Destination address for assets
    /// * `amount` - Amount to allocate
    ///
    /// # Security
    /// - Reentrancy protection: Sets reentrancy guard before cross-contract transfer
    /// - Auth: Requires `caller` auth and checks `is_authorized`
    /// - Data integrity: Updates commitment `current_value` and protocol `TotalValueLocked`
    ///
    /// # Errors
    /// - `CommitmentError::Unauthorized` if caller lacks permission
    /// - `CommitmentError::NotActive` if commitment not active
    /// - `CommitmentError::InsufficientBalance` if amount > current_value
    pub fn allocate(
        e: Env,
        caller: Address,
        commitment_id: String,
        target_pool: Address,
        amount: i128,
    ) {
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);
        Pausable::require_not_paused(&e);

        caller.require_auth();
        if !Self::is_authorized(e.clone(), caller.clone()) {
            set_reentrancy_guard(&e, false);
            fail(
                &e,
                CommitmentError::Unauthorized,
                "allocate: caller not admin or authorized allocator",
            );
        }

        let fn_symbol = symbol_short!("alloc");
        RateLimiter::check(&e, &target_pool, &fn_symbol);

        if amount <= 0 {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::InvalidAmount, "allocate");
        }

        let commitment = read_commitment(&e, &commitment_id).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::CommitmentNotFound, "allocate")
        });

        let active_status = String::from_str(&e, "active");
        if commitment.status != active_status {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::NotActive, "allocate");
        }

        if commitment.current_value < amount {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::InsufficientBalance, "allocate");
        }

        let old_value = commitment.current_value;
        let mut updated_commitment = commitment;
        updated_commitment.current_value = SafeMath::sub(old_value, amount);
        set_commitment(&e, &updated_commitment);

        // Update TVL: decrements by the amount moved out of contract custody
        let tvl = e.storage().instance().get::<_, i128>(&DataKey::TotalValueLocked).unwrap_or(0);
        e.storage().instance().set(&DataKey::TotalValueLocked, &(tvl - amount));

        let contract_address = e.current_contract_address();
        let token_client = token::Client::new(&e, &updated_commitment.asset_address);
        token_client.transfer(&contract_address, &target_pool, &amount);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("Alloc"), commitment_id, target_pool),
            (amount, e.ledger().timestamp()),
        );
    }

    /// Revoke authorized updater permission.
    ///
    /// # Authorization
    /// - Admin only
    pub fn remove_updater(e: Env, caller: Address, updater: Address) {
        require_admin(&e, &caller);
        remove_authorized_updater(&e, &updater);
    }

    /// Set the linked allocation logic contract.
    ///
    /// # Authorization
    /// - Admin only
    pub fn set_allocation_contract(e: Env, caller: Address, addr: Address) {
        require_admin(&e, &caller);
        e.storage().instance().set(&DataKey::AllocationContract, &addr);
    }

    /// List all authorized updater addresses.
    pub fn get_authorized_updaters(e: Env) -> Vec<Address> {
        e.storage()
            .instance()
            .get::<_, Vec<Address>>(&DataKey::AuthorizedUpdaters)
            .unwrap_or(Vec::new(&e))
    }

    /// Configure rate limits for a specific function.
    ///
    /// # Arguments
    /// * `function` - Symbol of the function (e.g. `symbol_short!("create")`)
    /// * `window_seconds` - Time window for the limit
    /// * `max_calls` - Maximum allowed calls within the window
    ///
    /// # Authorization
    /// - Admin only
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

    /// Exempt or unexempt an address from rate limits.
    ///
    /// # Authorization
    /// - Admin only
    pub fn set_rate_limit_exempt(e: Env, caller: Address, address: Address, exempt: bool) {
        require_admin(&e, &caller);
        RateLimiter::set_exempt(&e, &address, exempt);
    }

    /// Check if the contract is in emergency mode.
    pub fn is_emergency_mode(e: Env) -> bool {
        EmergencyControl::is_emergency_mode(&e)
    }

    /// Enable or disable emergency mode.
    ///
    /// # Authorization
    /// - Admin only
    pub fn set_emergency_mode(e: Env, caller: Address, enabled: bool) {
        require_admin(&e, &caller);
        EmergencyControl::set_emergency_mode(&e, enabled);
    }

    /// Protocol-level emergency withdrawal for the admin.
    ///
    /// # Arguments
    /// * `asset` - Token address to withdraw
    /// * `to` - Recipient address
    /// * `amount` - Amount to withdraw
    ///
    /// # Authorization
    /// - Admin only
    /// - Emergency mode MUST be enabled
    pub fn emergency_withdraw(e: Env, caller: Address, asset: Address, to: Address, amount: i128) {
        require_admin(&e, &caller);
        EmergencyControl::require_emergency(&e);
        Validation::require_positive(amount);
        transfer_assets(&e, &e.current_contract_address(), &to, &asset, amount);
    }

    // ========================================================================
    // Fee Management
    // ========================================================================

    /// Set the creation fee rate in basis points (0-10000).
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `bps` - Fee rate in basis points. 100 bps = 1%. Must be 0-10000.
    ///
    /// # Security
    /// - Admin-only: Uses `require_admin` for authorization
    /// - Validates bps is within valid range (0-10000)
    ///
    /// # Errors
    /// - `CommitmentError::Unauthorized` if caller is not admin
    /// - `CommitmentError::InvalidFeeBps` if bps > 10000
    pub fn set_creation_fee_bps(e: Env, caller: Address, bps: u32) {
        require_admin(&e, &caller);
        if bps > fees::BPS_MAX {
            fail(&e, CommitmentError::InvalidFeeBps, "set_creation_fee_bps");
        }
        e.storage().instance().set(&DataKey::CreationFeeBps, &bps);
        e.events().publish(
            (Symbol::new(&e, "CreationFeeSet"),),
            (bps, e.ledger().timestamp()),
        );
    }

    /// Set the fee recipient (protocol treasury) for fee withdrawals.
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `recipient` - Address to receive withdrawn fees
    ///
    /// # Security
    /// - Admin-only: Uses `require_admin` for authorization
    /// - Validates recipient is not zero address
    ///
    /// # Errors
    /// - `CommitmentError::Unauthorized` if caller is not admin
    /// - `CommitmentError::ZeroAddress` if recipient is zero address
    pub fn set_fee_recipient(e: Env, caller: Address, recipient: Address) {
        require_admin(&e, &caller);
        if is_zero_address(&e, &recipient) {
            fail(&e, CommitmentError::ZeroAddress, "set_fee_recipient");
        }
        e.storage().instance().set(&DataKey::FeeRecipient, &recipient);
        e.events().publish(
            (Symbol::new(&e, "FeeRecipientSet"),),
            (recipient.clone(), e.ledger().timestamp()),
        );
    }

    /// Withdraw collected fees to the configured fee recipient.
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `asset_address` - Token address to withdraw fees from
    /// * `amount` - Amount of fees to withdraw
    ///
    /// # Security
    /// - Admin-only: Uses `require_admin` for authorization
    /// - Reentrancy protection: Uses existing reentrancy guard
    /// - Validates fee recipient is set
    /// - Validates sufficient collected fees exist
    /// - Amount must be positive
    ///
    /// # Errors
    /// - `CommitmentError::Unauthorized` if caller is not admin
    /// - `CommitmentError::FeeRecipientNotSet` if recipient not configured
    /// - `CommitmentError::InsufficientFees` if amount > collected fees
    /// - `CommitmentError::InvalidAmount` if amount <= 0
    pub fn withdraw_fees(e: Env, caller: Address, asset_address: Address, amount: i128) {
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);
        require_admin(&e, &caller);
        Validation::require_positive(amount);

        // Check fee recipient is set
        let recipient: Address = e
            .storage()
            .instance()
            .get(&DataKey::FeeRecipient)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(&e, CommitmentError::FeeRecipientNotSet, "withdraw_fees")
            });

        // Check sufficient collected fees
        let fee_key = DataKey::CollectedFees(asset_address.clone());
        let collected: i128 = e.storage().instance().get(&fee_key).unwrap_or(0);
        if collected < amount {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::InsufficientFees, "withdraw_fees");
        }

        // Update collected fees
        e.storage().instance().set(&fee_key, &(collected - amount));

        // Transfer fees to recipient
        transfer_assets(&e, &e.current_contract_address(), &recipient, &asset_address, amount);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (Symbol::new(&e, "FeesWithdrawn"), asset_address, recipient),
            (amount, e.ledger().timestamp()),
        );
    }

    /// Get the current creation fee rate in basis points.
    ///
    /// # Returns
    /// Fee rate in basis points (0-10000). Returns 0 if not set.
    pub fn get_creation_fee_bps(e: Env) -> u32 {
        e.storage()
            .instance()
            .get(&DataKey::CreationFeeBps)
            .unwrap_or(0)
    }

    /// Get the configured fee recipient address.
    ///
    /// # Returns
    /// Fee recipient address, or None if not set.
    pub fn get_fee_recipient(e: Env) -> Option<Address> {
        e.storage().instance().get(&DataKey::FeeRecipient)
    }

    /// Get the collected fees for a specific asset.
    ///
    /// # Arguments
    /// * `asset_address` - Token address to query
    ///
    /// # Returns
    /// Amount of collected fees for the asset. Returns 0 if none collected.
    pub fn get_collected_fees(e: Env, asset_address: Address) -> i128 {
        e.storage()
            .instance()
            .get(&DataKey::CollectedFees(asset_address))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod emergency_tests;

#[cfg(test)]
mod fee_tests;

#[cfg(all(test, feature = "benchmark"))]
mod benchmarks;

#[cfg(test)]
mod test_zero_address;
