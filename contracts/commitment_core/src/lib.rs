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
//!
//! Formal verification planning placeholder:
//! [`docs/COMMITMENT_CORE_FORMAL_VERIFICATION_SCOPE.md`](../../../docs/COMMITMENT_CORE_FORMAL_VERIFICATION_SCOPE.md)

use shared_utils::{
    emit_error_event, fees, EmergencyControl, Pausable, RateLimiter, SafeMath, TimeUtils,
    Validation,
};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, log, symbol_short, token, Address, Env,
    IntoVal, String, Symbol, Vec,
};

pub mod fuzzing;

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
    AlreadySettled = 11,
    ReentrancyDetected = 12,
    NotActive = 13,
    InvalidStatus = 14,
    NotInitialized = 15,
    NotExpired = 16,
    ValueUpdateViolation = 17,
    NotAuthorizedUpdater = 18,
    ZeroAddress = 19,
    /// Duration would cause expires_at to overflow u64
    ExpirationOverflow = 20,
    /// Invalid fee basis points (must be 0-10000)
    InvalidFeeBps = 21,
    /// Fee recipient not set; cannot withdraw
    FeeRecipientNotSet = 22,
    /// Insufficient collected fees to withdraw
    InsufficientFees = 23,
    /// Arithmetic operation overflowed or underflowed
    ArithmeticOverflow = 24,
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
            CommitmentError::NotAuthorizedUpdater => "Caller is not an authorized value updater",
            CommitmentError::ZeroAddress => "Zero address is not allowed",
            CommitmentError::ExpirationOverflow => {
                "Duration would cause expiration timestamp overflow"
            }
            CommitmentError::InvalidFeeBps => "Invalid fee basis points: must be 0-10000",
            CommitmentError::FeeRecipientNotSet => "Fee recipient not set; cannot withdraw",
            CommitmentError::InsufficientFees => "Insufficient collected fees to withdraw",
            CommitmentError::ArithmeticOverflow => "Arithmetic overflow or underflow",
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
    AuthorizedUpdater(Address),
    AuthorizedGuardian(Address),
    AuthorizedTreasurer(Address),
    AuthorizedOperator(Address),
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
    let zero_str = String::from_str(
        e,
        "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
    );
    let zero_addr = Address::from_string(&zero_str);
    address == &zero_addr
}

fn check_sufficient_balance(e: &Env, owner: &Address, asset_address: &Address, amount: i128) {
    let token_client = token::Client::new(e, asset_address);
    let balance = token_client.balance(owner);
    if balance < amount {
        log!(e, "Insufficient balance: {} < {}", balance, amount);
        fail(
            e,
            CommitmentError::InsufficientBalance,
            "check_sufficient_balance",
        );
    }
}

fn transfer_assets(e: &Env, from: &Address, to: &Address, asset_address: &Address, amount: i128) {
    let token_client = token::Client::new(e, asset_address);
    token_client.transfer(from, to, &amount);
}

/// Helper function to call NFT contract mint function.
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

// fn has_commitment(e: &Env, commitment_id: &String) -> bool {
//     e.storage().instance().has(&DataKey::Commitment(commitment_id.clone()))
// }

fn require_no_reentrancy(e: &Env) {
    if e.storage()
        .instance()
        .get::<_, bool>(&DataKey::ReentrancyGuard)
        .unwrap_or(false)
    {
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

/// Check whether `caller` is an authorized value updater or the admin.
///
/// # Trust boundary
/// Only addresses explicitly added via `add_updater` (admin-only) may call
/// `update_value`. The admin itself is always implicitly authorized so that
/// initial bootstrapping does not require a self-grant.
fn require_authorized_updater(e: &Env, caller: &Address) {
    caller.require_auth();
    // Admin is always authorized
    if let Some(admin) = e.storage().instance().get::<_, Address>(&DataKey::Admin) {
        if *caller == admin {
            return;
        }
    }
    let updaters: Vec<Address> = e
        .storage()
        .instance()
        .get::<_, Vec<Address>>(&DataKey::AuthorizedUpdaters)
        .unwrap_or(Vec::new(e));
    if !updaters.contains(caller) {
        fail(e, CommitmentError::NotAuthorizedUpdater, "require_authorized_updater");
    }
}

fn add_authorized_updater(e: &Env, updater: &Address) {
    let mut updaters: Vec<Address> = e
        .storage()
        .instance()
        .get::<_, Vec<Address>>(&DataKey::AuthorizedUpdaters)
        .unwrap_or(Vec::new(e));
    if !updaters.contains(updater) {
        updaters.push_back(updater.clone());
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedUpdaters, &updaters);
    }
}

fn remove_authorized_updater(e: &Env, updater: &Address) {
    let mut updaters: Vec<Address> = e
        .storage()
        .instance()
        .get::<_, Vec<Address>>(&DataKey::AuthorizedUpdaters)
        .unwrap_or(Vec::new(e));
    if let Some(idx) = updaters.iter().position(|a| a == *updater) {
        updaters.remove(idx as u32);
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedUpdaters, &updaters);
    }
}

fn remove_from_owner_commitments(e: &Env, owner: &Address, commitment_id: &String) {
    let mut commitments: Vec<String> = e
        .storage()
        .instance()
        .get::<_, Vec<String>>(&DataKey::OwnerCommitments(owner.clone()))
        .unwrap_or(Vec::new(e));
    if let Some(idx) = commitments.iter().position(|id| id == *commitment_id) {
        commitments.remove(idx as u32);
        e.storage()
            .instance()
            .set(&DataKey::OwnerCommitments(owner.clone()), &commitments);
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
        let valid_types = ["safe", "balanced", "aggressive"];
        Validation::require_valid_commitment_type(e, &rules.commitment_type, &valid_types);

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
        } else if rules.commitment_type == String::from_str(e, "aggressive")
            && rules.early_exit_penalty < 5
        {
            panic!("Aggressive type: early_exit_penalty must be >= 5");
        }
    }

    fn generate_commitment_id(e: &Env, counter: u64) -> String {
        let mut buf = [0u8; 32];
        buf[0] = b'c';
        buf[1] = b'_';
        let mut n = counter;
        let mut i = 2;
        if n == 0 {
            buf[i] = b'0';
            i += 1;
        } else {
            let mut digits = [0u8; 20];
            let mut count = 0;
            while n > 0 {
                digits[count] = (n % 10) as u8 + b'0';
                n /= 10;
                count += 1;
            }
            for j in 0..count {
                buf[i] = digits[count - 1 - j];
                i += 1;
            }
        }
        String::from_str(e, core::str::from_utf8(&buf[..i]).unwrap_or("c_0"))
    }

    /// Initialize the core contract with its admin and linked NFT contract. The provided `nft_contract` becomes the downstream dependency used by
    /// `create_commitment`, `settle`, and `early_exit`.
    pub fn initialize(e: Env, admin: Address, nft_contract: Address) {
        if e.storage().instance().has(&DataKey::Admin) {
            fail(&e, CommitmentError::AlreadyInitialized, "initialize");
        }

        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage()
            .instance()
            .set(&DataKey::NftContract, &nft_contract);
        e.storage()
            .instance()
            .set(&DataKey::TotalCommitments, &0u64);
        e.storage()
            .instance()
            .set(&DataKey::TotalValueLocked, &0i128);
        e.storage()
            .instance()
            .set(&DataKey::AllCommitmentIds, &Vec::<String>::new(&e));
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);
        e.storage().instance().set(&Pausable::PAUSED_KEY, &false);
        EmergencyControl::set_emergency_mode(&e, false);
    }

    /// Create a new commitment, transfer assets into custody, and mint the paired NFT.
    ///
    /// Call sequence:
    /// 1. validate owner auth, rules, and balances
    /// 2. compute creation fee and net amount
    /// 3. persist commitment state and counters
    /// 4. transfer tokens into this contract
    /// 5. invoke `commitment_nft::mint`
    ///
    /// Because Soroban reverts the entire invocation on panic, a downstream NFT mint
    /// failure should roll back the earlier state writes and token transfer.
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

        let creation_fee_bps: u32 = e
            .storage()
            .instance()
            .get(&DataKey::CreationFeeBps)
            .unwrap_or(0);
        let creation_fee = fuzzing::checked_fee_from_bps(amount, creation_fee_bps)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(&e, CommitmentError::ArithmeticOverflow, "create");
            });
        let net_amount = amount.checked_sub(creation_fee).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::ArithmeticOverflow, "create");
        });

        let expires_at = TimeUtils::checked_calculate_expiration(&e, rules.duration_days)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(&e, CommitmentError::ExpirationOverflow, "create")
            });

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

        check_sufficient_balance(&e, &owner, &asset_address, amount);

        let current_total = e
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::TotalCommitments)
            .unwrap_or(0);
        let nft_contract = e
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::NftContract)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(&e, CommitmentError::NotInitialized, "create")
            });

        // Calculate creation fee if configured
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
        // Net amount locked in commitment (after fee deduction)
        let net_amount = amount - creation_fee;

        // Compute creation fee and net amount before any state writes so that
        // `net_amount` is defined for both the Commitment struct and the TVL update.
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

        let commitment_id = Self::generate_commitment_id(&e, current_total);

        // Calculate creation fee first
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

        // Net amount locked in commitment (after fee deduction)
        let net_amount = amount - creation_fee;

        let commitment = Commitment {
            commitment_id: commitment_id.clone(),
            owner: owner.clone(),
            nft_token_id: 0,
            rules: rules.clone(),
            amount: amount,
            asset_address: asset_address.clone(),
            created_at: TimeUtils::now(&e),
            expires_at,
            current_value: amount,
            status: String::from_str(&e, "active"),
        };

        set_commitment(&e, &commitment);
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
        e.storage()
            .instance()
            .set(&DataKey::TotalCommitments, &(current_total + 1));
        let tvl = e
            .storage()
            .instance()
            .get::<_, i128>(&DataKey::TotalValueLocked)
            .unwrap_or(0);
        let updated_tvl = tvl.checked_add(net_amount).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::ArithmeticOverflow, "create");
        });
        e.storage().instance().set(&DataKey::TotalValueLocked, &updated_tvl);

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
            let updated_fees = current_fees.checked_add(creation_fee).unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(&e, CommitmentError::ArithmeticOverflow, "create");
            });
            e.storage()
                .instance()
                .set(&fee_key, &updated_fees);
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
    /// This is the read API consumed by `attestation_engine` for compliance checks,
    /// health metrics, and commitment-existence validation. It intentionally does not
    /// perform auth checks so downstream contracts can read commitment state.
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| fail(&e, CommitmentError::CommitmentNotFound, "get_commitment"))
    }

    /// List all commitment IDs owned by the given address.
    pub fn list_commitments_by_owner(e: Env, owner: Address) -> Vec<String> {
        Self::get_owner_commitments(e, owner, 0, MAX_PAGE_SIZE)
    }

    /// Return a paginated slice of commitment IDs owned by `owner`.
    ///
    /// # Parameters
    /// - `owner`  – The address whose commitments are queried. No auth required; this is a
    ///              read-only view that does not mutate any storage.
    /// - `offset` – Zero-based index of the first item to return. If `offset` ≥ total count,
    ///              an empty `Vec` is returned rather than panicking.
    /// - `limit`  – Maximum number of items to return per call. Capped internally at
    ///              [`MAX_PAGE_SIZE`] (50) to prevent resource exhaustion. Passing `0` returns
    ///              an empty `Vec`.
    ///
    /// # Returns
    /// A `Vec<String>` of commitment IDs in insertion order, starting at `offset` and
    /// containing at most `min(limit, MAX_PAGE_SIZE)` entries.
    ///
    /// # Resource limits
    /// Soroban's VCPU and memory budgets constrain how many storage reads can occur per
    /// invocation. Stress testing with 50 commitments confirmed that pages of ≤ 50 IDs
    /// stay well within the default budget. Callers should iterate with successive
    /// `offset` values to page through larger sets.
    pub fn get_owner_commitments(e: Env, owner: Address, offset: u32, limit: u32) -> Vec<String> {
        let all: Vec<String> = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::OwnerCommitments(owner))
            .unwrap_or(Vec::new(&e));

        let total = all.len();
        if offset >= total || limit == 0 {
            return Vec::new(&e);
        }

        let effective_limit = limit.min(MAX_PAGE_SIZE);
        let end = (offset + effective_limit).min(total);
        let mut page = Vec::new(&e);
        for i in offset..end {
            page.push_back(all.get(i).unwrap());
        }
        page
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

    /// Get commitment IDs created between two timestamps (inclusive).
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

    pub fn pause(e: Env, caller: Address) {
        caller.require_auth();
        if !Self::is_operator(e.clone(), caller.clone()) {
            fail(&e, CommitmentError::Unauthorized, "pause");
        }
        Pausable::pause(&e);
    }

    /// Unpauses the contract, re-enabling normal operations.
    ///
    /// Restricted to the Operator or Admin role.
    pub fn unpause(e: Env, caller: Address) {
        caller.require_auth();
        if !Self::is_operator(e.clone(), caller.clone()) {
            fail(&e, CommitmentError::Unauthorized, "unpause");
        }
        Pausable::unpause(&e);
    }

    /// Returns true if the contract is currently paused.
    pub fn is_paused(e: Env) -> bool {
        Pausable::is_paused(&e)
    }

    /// Adds an address to the authorized allocators list.
    ///
    /// Restricted to the Admin role.
    /// Authorized allocators can call the `allocate` function to move assets
    /// from commitments to target pools.
    pub fn add_allocator(e: Env, caller: Address, allocator: Address) {
        require_admin(&e, &caller);
        e.storage().instance().set(
            &DataKey::AuthorizedAllocator(contract_address.clone()),
            &true,
        );
        e.events().publish(
            (Symbol::new(&e, "AuthorizedAllocatorAdded"),),
            (allocator, e.ledger().timestamp()),
        );
    }

    /// Removes an address from the authorized allocators list.
    ///
    /// Restricted to the Admin role.
    pub fn remove_allocator(e: Env, caller: Address, allocator: Address) {
        require_admin(&e, &caller);
        e.storage().instance().remove(&DataKey::AuthorizedAllocator(allocator.clone()));
        e.events().publish(
            (Symbol::new(&e, "AuthorizedAllocatorRemoved"),),
            (allocator, e.ledger().timestamp()),
        );
    }

    /// Alias for `add_allocator` to maintain backward compatibility with previous versions.
    pub fn add_authorized_contract(e: Env, caller: Address, contract_address: Address) {
        Self::add_allocator(e, caller, contract_address);
    }

    /// Alias for `remove_allocator` to maintain backward compatibility with previous versions.
    pub fn remove_authorized_contract(e: Env, caller: Address, contract_address: Address) {
        Self::remove_allocator(e, caller, contract_address);
    }

    /// Returns true if the given address is an authorized allocator.
    ///
    /// An address is an authorized allocator if it is the Admin, the registered
    /// Allocation Contract, or an address explicitly added via `add_allocator`.
    pub fn is_allocator(e: Env, address: Address) -> bool {
        let admin = e.storage().instance().get::<_, Address>(&DataKey::Admin);
        if let Some(a) = admin {
            if address == a { return true; }
        }
        if let Some(alloc_contract) = e.storage().instance().get::<_, Address>(&DataKey::AllocationContract) {
            if address == alloc_contract { return true; }
        }
        e.storage()
            .instance()
            .get::<_, bool>(&DataKey::AuthorizedAllocator(address))
            .unwrap_or(false)
    }

    /// Alias for `is_allocator` to maintain backward compatibility with previous versions.
    pub fn is_authorized(e: Env, contract_address: Address) -> bool {
        Self::is_allocator(e, contract_address)
    }

    /// Check if an address is authorized to update commitment values.
    ///
    pub fn is_updater(e: Env, address: Address) -> bool {
        let admin = e.storage().instance().get::<_, Address>(&DataKey::Admin);
        if let Some(a) = admin {
            if address == a {
                return true;
            }
        }
        e.storage()
            .instance()
            .get::<_, bool>(&DataKey::AuthorizedUpdater(address))
            .unwrap_or(false)
    }

    /// Returns true if the given address is an authorized Guardian.
    ///
    /// Guardians are authorized to set the emergency mode and perform emergency
    /// asset withdrawals. The Admin is implicitly a Guardian.
    pub fn is_guardian(e: Env, address: Address) -> bool {
        let admin = e.storage().instance().get::<_, Address>(&DataKey::Admin);
        if let Some(a) = admin {
            if address == a { return true; }
        }
        e.storage()
            .instance()
            .get::<_, bool>(&DataKey::AuthorizedGuardian(address))
            .unwrap_or(false)
    }

    /// Returns true if the given address is an authorized Treasurer.
    ///
    /// Treasurers are authorized to manage protocol fees, including setting rates,
    /// recipients, and performing withdrawals. The Admin is implicitly a Treasurer.
    pub fn is_treasurer(e: Env, address: Address) -> bool {
        let admin = e.storage().instance().get::<_, Address>(&DataKey::Admin);
        if let Some(a) = admin {
            if address == a { return true; }
        }
        e.storage()
            .instance()
            .get::<_, bool>(&DataKey::AuthorizedTreasurer(address))
            .unwrap_or(false)
    }

    /// Returns true if the given address is an authorized Operator.
    ///
    /// Operators are authorized to pause and unpause the contract.
    /// The Admin is implicitly an Operator.
    pub fn is_operator(e: Env, address: Address) -> bool {
        let admin = e.storage().instance().get::<_, Address>(&DataKey::Admin);
        if let Some(a) = admin {
            if address == a { return true; }
        }
        e.storage().instance().get::<_, bool>(&DataKey::AuthorizedOperator(address)).unwrap_or(false)
    }

    /// Update the current value of a commitment.
    ///
    /// This is a restricted state-changing operation that can only be performed by
    /// the admin or an authorized updater. It updates the commitment's valuation
    /// and the total protocol TVL.
    ///
    /// ### Parameters
    /// - `caller`: The address initiating the update. Must be admin or authorized updater.
    /// - `commitment_id`: Unique identifier of the commitment.
    /// - `new_value`: The new valuation to set.
    ///
    /// ### Security Notes
    /// - Requires `caller.require_auth()`.
    /// - Enforces `is_updater` check.
    pub fn update_value(e: Env, caller: Address, commitment_id: String, new_value: i128) {
        caller.require_auth();
        if !Self::is_updater(e.clone(), caller.clone()) {
            fail(&e, CommitmentError::Unauthorized, "upd: unauthorized");
        }
        let fn_symbol = symbol_short!("upd_val");
        RateLimiter::check(&e, &caller, &fn_symbol);
        Validation::require_non_negative(new_value);

        let mut commitment = read_commitment(&e, &commitment_id)
            .unwrap_or_else(|| fail(&e, CommitmentError::CommitmentNotFound, "upd"));
        if commitment.status != String::from_str(&e, "active") {
            fail(&e, CommitmentError::NotActive, "upd");
        }

        let old_value = commitment.current_value;

        // Persist the new value unconditionally — this is the fix for the misleading
        // update_value behavior: the value is always written to storage so that
        // subsequent reads and settlement calculations reflect the update.
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

        // Persist to storage — value and (potentially) status are both written here.
        set_commitment(&e, &commitment);

        // Update TVL by the delta so the aggregate stays consistent with the persisted value.
        let tvl = e.storage().instance().get::<_, i128>(&DataKey::TotalValueLocked).unwrap_or(0);
        let updated_tvl = tvl
            .checked_sub(old_value)
            .and_then(|value| value.checked_add(new_value))
            .unwrap_or_else(|| fail(&e, CommitmentError::ArithmeticOverflow, "upd"));
        e.storage().instance().set(&DataKey::TotalValueLocked, &updated_tvl);
    }

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

    pub fn get_violation_details(e: Env, commitment_id: String) -> (bool, bool, bool, i128, u64) {
        let commitment = read_commitment(&e, &commitment_id).unwrap_or_else(|| {
            fail(
                &e,
                CommitmentError::CommitmentNotFound,
                "get_violation_details",
            )
        });

        let now = e.ledger().timestamp();
        let loss_percent = if commitment.amount > 0 {
            SafeMath::loss_percent(commitment.amount, commitment.current_value)
        } else {
            0
        };
        let loss_violated = loss_percent > commitment.rules.max_loss_percent as i128;
        let duration_violated = now >= commitment.expires_at;
        let has_violations = loss_violated || duration_violated;
        let time_remaining = commitment.expires_at.saturating_sub(now);

        (
            has_violations,
            loss_violated,
            duration_violated,
            loss_percent,
            time_remaining,
        )
    }

    /// Settle an expired commitment, release assets to the owner, and mark the NFT settled.
    ///
    /// Cross-contract dependency: invokes `commitment_nft::settle` after the core state and
    /// token transfer path have been prepared. This flow is guarded by the reentrancy flag and
    /// relies on transaction rollback if the downstream NFT call fails.
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
        let new_tvl = if tvl > settlement_amount {
            SafeMath::sub(tvl, settlement_amount)
        } else {
            0
        };
        e.storage().instance().set(&DataKey::TotalValueLocked, &new_tvl);

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

    /// Exit a commitment before maturity, apply the configured penalty, and mark the NFT inactive.
    ///
    /// # Arguments
    /// * `commitment_id` - The unique identifier of the commitment to exit.
    /// * `caller` - Must be the commitment owner; `require_auth` is enforced.
    ///
    /// # Penalty arithmetic
    /// `penalty = SafeMath::penalty_amount(current_value, early_exit_penalty)`
    /// which computes `(current_value * early_exit_penalty) / 100` using checked
    /// integer arithmetic. Division truncates toward zero, so small values (e.g.
    /// `current_value < 100 / early_exit_penalty`) may yield a zero penalty.
    /// The penalty is credited to `CollectedFees(asset_address)` as protocol revenue.
    /// `returned = current_value - penalty` is transferred back to the owner only
    /// when `returned > 0`; a 100% penalty results in no transfer.
    ///
    /// # Overflow safety
    /// `SafeMath::mul(current_value, early_exit_penalty as i128)` panics with
    /// `"Math: multiplication overflow"` if the intermediate product exceeds `i128::MAX`.
    /// Callers must ensure `current_value * early_exit_penalty <= i128::MAX`.
    /// In practice `early_exit_penalty <= 100`, so values up to `i128::MAX / 100`
    /// are safe.
    ///
    /// # Trust boundaries
    /// - Only the commitment owner (verified via `require_auth` + owner equality check)
    ///   may call this function. Admin and third-party addresses are rejected.
    /// - The commitment must be in `"active"` status; settled, violated, or already-exited
    ///   commitments are rejected with `CommitmentError::NotActive`.
    ///
    /// # Reentrancy
    /// Protected by the `ReentrancyGuard` storage flag. The guard is cleared before
    /// the outbound `commitment_nft::mark_inactive` call to allow the NFT contract to
    /// read commitment state if needed, but the core state mutation is complete before
    /// any cross-contract call.
    ///
    /// # Errors
    /// - `CommitmentError::CommitmentNotFound` — commitment_id does not exist.
    /// - `CommitmentError::Unauthorized` — caller is not the commitment owner.
    /// - `CommitmentError::NotActive` — commitment is not in `"active"` status.
    /// - `CommitmentError::ReentrancyDetected` — reentrant call detected.
    /// - `CommitmentError::NotInitialized` — NFT contract address not set.
    ///
    /// # Cross-contract dependency
    /// Invokes `commitment_nft::mark_inactive` after updating the commitment record
    /// and returning the post-penalty amount to the owner.
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

        let penalty = SafeMath::penalty_amount(
            commitment.current_value,
            commitment.rules.early_exit_penalty,
        );
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
        e.storage()
            .instance()
            .set(&DataKey::TotalValueLocked, &(tvl - original_val));

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
        e.events().publish(
            (symbol_short!("EarlyExt"), commitment_id, caller),
            (penalty, returned, e.ledger().timestamp()),
        );
    }

    pub fn add_updater(e: Env, caller: Address, updater: Address) {
        require_admin(&e, &caller);
        add_authorized_updater(&e, &updater);
    }

    pub fn add_guardian(e: Env, caller: Address, guardian: Address) {
        require_admin(&e, &caller);
        e.storage().instance().set(&DataKey::AuthorizedGuardian(guardian), &true);
    }
    pub fn remove_guardian(e: Env, caller: Address, guardian: Address) {
        require_admin(&e, &caller);
        e.storage().instance().remove(&DataKey::AuthorizedGuardian(guardian));
    }
    pub fn add_treasurer(e: Env, caller: Address, treasurer: Address) {
        require_admin(&e, &caller);
        e.storage().instance().set(&DataKey::AuthorizedTreasurer(treasurer), &true);
    }
    pub fn remove_treasurer(e: Env, caller: Address, treasurer: Address) {
        require_admin(&e, &caller);
        e.storage().instance().remove(&DataKey::AuthorizedTreasurer(treasurer));
    }
    pub fn add_operator(e: Env, caller: Address, operator: Address) {
        require_admin(&e, &caller);
        e.storage().instance().set(&DataKey::AuthorizedOperator(operator), &true);
    }
    pub fn remove_operator(e: Env, caller: Address, operator: Address) {
        require_admin(&e, &caller);
        e.storage().instance().remove(&DataKey::AuthorizedOperator(operator));
    }

    /// Allocates assets from a commitment to a target investment pool.
    ///
    /// This operation is restricted to the admin or an authorized allocator contract.
    /// It reduces the commitment's internal `current_value` and transfers the 
    /// underlying tokens to the target address.
    ///
    /// ### Parameters
    /// - `caller`: The address initiating the allocation. Must be authorized.
    /// - `commitment_id`: Unique identifier of the commitment.
    /// - `target_pool`: Destination address for the tokens.
    /// - `amount`: Quantity of assets to allocate.
    ///
    /// ### Security Notes
    /// - Requires `caller.require_auth()`.
    /// - Enforces `is_allocator` check.
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
        if !Self::is_allocator(e.clone(), caller.clone()) {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::Unauthorized, "allocate: unauthorized");
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

        let mut updated_commitment = commitment;
        updated_commitment.current_value = SafeMath::sub(updated_commitment.current_value, amount);
        set_commitment(&e, &updated_commitment);

        let contract_address = e.current_contract_address();
        let token_client = token::Client::new(&e, &updated_commitment.asset_address);
        token_client.transfer(&contract_address, &target_pool, &amount);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("Alloc"), commitment_id, target_pool),
            (amount, e.ledger().timestamp()),
        );
    }

    /// Removes an address from the authorized updaters list.
    ///
    /// Restricted to the Admin role.
    pub fn remove_updater(e: Env, caller: Address, updater: Address) {
        require_admin(&e, &caller);
        remove_authorized_updater(&e, &updater);
    }

    pub fn set_allocation_contract(e: Env, caller: Address, addr: Address) {
        require_admin(&e, &caller);
        e.storage()
            .instance()
            .set(&DataKey::AllocationContract, &addr);
    }

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

    pub fn set_rate_limit_exempt(e: Env, caller: Address, address: Address, exempt: bool) {
        require_admin(&e, &caller);
        RateLimiter::set_exempt(&e, &address, exempt);
    }

    /// Returns true if the contract is currently in emergency mode.
    pub fn is_emergency_mode(e: Env) -> bool {
        EmergencyControl::is_emergency_mode(&e)
    }

    /// Sets the emergency mode status.
    ///
    /// Restricted to the Guardian or Admin role.
    /// When enabled, new commitments cannot be created, but emergency withdrawals
    /// can be performed.
    pub fn set_emergency_mode(e: Env, caller: Address, enabled: bool) {
        caller.require_auth();
        if !Self::is_guardian(e.clone(), caller.clone()) {
            fail(&e, CommitmentError::Unauthorized, "set_emergency_mode");
        }
        EmergencyControl::set_emergency_mode(&e, enabled);
    }

    /// Performs an emergency withdrawal of assets from the contract.
    ///
    /// Restricted to the Guardian or Admin role.
    /// Can only be called when the contract is in emergency mode.
    pub fn emergency_withdraw(e: Env, caller: Address, asset: Address, to: Address, amount: i128) {
        caller.require_auth();
        if !Self::is_guardian(e.clone(), caller.clone()) {
            fail(&e, CommitmentError::Unauthorized, "emergency_withdraw");
        }
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
    /// * `bps` - Fee rate in basis points. 100 bps = 1%. Must be 0-10000. Restricted to Treasurer or Admin.
    ///
    /// # Security
    /// - Treasurer-only: Uses `is_treasurer` for authorization
    /// - Validates bps is within valid range (0-10000)
    ///
    /// # Errors
    /// - `CommitmentError::Unauthorized` if caller is not treasurer or admin
    /// - `CommitmentError::InvalidFeeBps` if bps > 10000
    pub fn set_creation_fee_bps(e: Env, caller: Address, bps: u32) {
        caller.require_auth();
        if !Self::is_treasurer(e.clone(), caller.clone()) {
            fail(&e, CommitmentError::Unauthorized, "set_creation_fee_bps");
        }
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
    /// - Treasurer-only: Uses `is_treasurer` for authorization
    /// - Validates recipient is not zero address
    ///
    /// # Errors
    /// - `CommitmentError::Unauthorized` if caller is not treasurer or admin
    /// - `CommitmentError::ZeroAddress` if recipient is zero address
    pub fn set_fee_recipient(e: Env, caller: Address, recipient: Address) {
        caller.require_auth();
        if !Self::is_treasurer(e.clone(), caller.clone()) {
            fail(&e, CommitmentError::Unauthorized, "set_fee_recipient");
        }
        if is_zero_address(&e, &recipient) {
            fail(&e, CommitmentError::ZeroAddress, "set_fee_recipient");
        }
        e.storage()
            .instance()
            .set(&DataKey::FeeRecipient, &recipient);
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
    /// - Treasurer-only: Uses `is_treasurer` for authorization
    /// - Reentrancy protection: Uses existing reentrancy guard
    /// - Validates fee recipient is set
    /// - Validates sufficient collected fees exist
    /// - Amount must be positive
    ///
    /// # Errors
    /// - `CommitmentError::Unauthorized` if caller is not treasurer or admin
    /// - `CommitmentError::FeeRecipientNotSet` if recipient not configured
    /// - `CommitmentError::InsufficientFees` if amount > collected fees
    /// - `CommitmentError::InvalidAmount` if amount <= 0
    pub fn withdraw_fees(e: Env, caller: Address, asset_address: Address, amount: i128) {
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);
        caller.require_auth();
        if !Self::is_treasurer(e.clone(), caller.clone()) {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::Unauthorized, "withdraw_fees");
        }
        Validation::require_positive(amount);

        let recipient: Address = e
            .storage()
            .instance()
            .get(&DataKey::FeeRecipient)
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(&e, CommitmentError::FeeRecipientNotSet, "withdraw_fees")
            });

        let fee_key = DataKey::CollectedFees(asset_address.clone());
        let collected: i128 = e.storage().instance().get(&fee_key).unwrap_or(0);
        if collected < amount {
            set_reentrancy_guard(&e, false);
            fail(&e, CommitmentError::InsufficientFees, "withdraw_fees");
        }

        // Update collected fees
        e.storage().instance().set(&fee_key, &SafeMath::sub(collected, amount));

        // Transfer fees to recipient
        transfer_assets(
            &e,
            &e.current_contract_address(),
            &recipient,
            &asset_address,
            amount,
        );

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (Symbol::new(&e, "FeesWithdrawn"), asset_address, recipient),
            (amount, e.ledger().timestamp()),
        );
    }

    /// Get the current creation fee rate in basis points.
    pub fn get_creation_fee_bps(e: Env) -> u32 {
        e.storage()
            .instance()
            .get(&DataKey::CreationFeeBps)
            .unwrap_or(0)
    }

    /// Get the configured fee recipient address.
    pub fn get_fee_recipient(e: Env) -> Option<Address> {
        e.storage().instance().get(&DataKey::FeeRecipient)
    }

    /// Get the collected fees for a specific asset.
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

#[cfg(test)]
mod fuzz_tests;

#[cfg(all(test, feature = "benchmark"))]
mod benchmarks;

#[cfg(test)]
mod test_zero_address;

/// Invariant tests that verify the correctness of every invariant exercised by
/// [`benchmarks_optimized`]. These run under the normal `cargo test` profile
/// (no `benchmark` feature required).
#[cfg(test)]
mod benchmark_invariant_tests;

#[cfg(all(test, feature = "benchmark"))]
mod benchmarks_optimized;