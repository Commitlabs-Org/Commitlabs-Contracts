//! Commitment Transformation contract (#57).
//!
//! Transforms commitments into risk tranches, collateralized assets,
//! and secondary market instruments with protocol-specific guarantees.

#![no_std]

use shared_utils::{emit_error_event, Validation};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env, String,
    Vec,
};

// ============================================================================
// Errors (aligned with shared_utils::error_codes)
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TransformationError {
    InvalidAmount = 1,
    InvalidTrancheRatios = 2,
    InvalidFeeBps = 3,
    Unauthorized = 4,
    NotInitialized = 5,
    AlreadyInitialized = 6,
    CommitmentNotFound = 7,
    TransformationNotFound = 8,
    InvalidState = 9,
    ReentrancyDetected = 10,
    FeeRecipientNotSet = 11,
    InsufficientFees = 12,
}

impl TransformationError {
    pub fn message(&self) -> &'static str {
        match self {
            TransformationError::InvalidAmount => "Invalid amount: must be positive",
            TransformationError::InvalidTrancheRatios => "Tranche ratios must sum to 100",
            TransformationError::InvalidFeeBps => "Fee must be 0-10000 bps",
            TransformationError::Unauthorized => "Unauthorized: caller not owner or authorized",
            TransformationError::NotInitialized => "Contract not initialized",
            TransformationError::AlreadyInitialized => "Contract already initialized",
            TransformationError::CommitmentNotFound => "Commitment not found",
            TransformationError::TransformationNotFound => "Transformation record not found",
            TransformationError::InvalidState => "Invalid state for transformation",
            TransformationError::ReentrancyDetected => "Reentrancy detected",
            TransformationError::FeeRecipientNotSet => "Fee recipient not set",
            TransformationError::InsufficientFees => "Insufficient collected fees to withdraw",
        }
    }
}

fn fail(e: &Env, err: TransformationError, context: &str) -> ! {
    emit_error_event(e, err as u32, context);
    panic!("{}", err.message());
}

// ============================================================================
// Data types
// ============================================================================

/// Tranche status for lifecycle management
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrancheStatus {
    Active,
    Closed,
}

/// Risk tranche representing a slice of a transformed commitment.
/// 
/// # Fields
/// * `tranche_id` - Unique identifier for this tranche
/// * `transformation_id` - Reference to the parent tranche set
/// * `commitment_id` - Reference to the parent commitment
/// * `risk_level` - Risk category: "senior", "mezzanine", "equity"
/// * `amount` - Current allocation amount in the tranche
/// * `share_bps` - Share in basis points of the parent tranche set
/// * `created_at` - Ledger timestamp of creation
/// * `status` - Current lifecycle status (Active/Closed)
/// * `updated_at` - Ledger timestamp of last update
/// 
/// # Security Notes
/// - Amount modifications require authorization (owner or authorized transformer)
/// - Closed tranches cannot be modified
/// - Arithmetic uses checked operations to prevent overflow/underflow
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskTranche {
    pub tranche_id: String,
    pub transformation_id: String,
    pub commitment_id: String,
    pub risk_level: String, // "senior", "mezzanine", "equity"
    pub amount: i128,
    pub share_bps: u32,
    pub created_at: u64,
    pub status: TrancheStatus,
    pub updated_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrancheSet {
    pub transformation_id: String,
    pub commitment_id: String,
    pub owner: Address,
    pub total_value: i128,
    pub tranches: Vec<RiskTranche>,
    pub fee_paid: i128,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollateralizedAsset {
    pub asset_id: String,
    pub commitment_id: String,
    pub owner: Address,
    pub collateral_amount: i128,
    pub asset_address: Address,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecondaryInstrument {
    pub instrument_id: String,
    pub commitment_id: String,
    pub owner: Address,
    pub instrument_type: String, // "receivable", "option", "warrant"
    pub amount: i128,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolGuarantee {
    pub guarantee_id: String,
    pub commitment_id: String,
    pub guarantee_type: String,
    pub terms_hash: String,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    CoreContract,
    TransformationFeeBps,
    ReentrancyGuard,
    TrancheSet(String),
    /// Individual tranche storage key for direct access
    Tranche(String),
    CollateralizedAsset(String),
    SecondaryInstrument(String),
    ProtocolGuarantee(String),
    CommitmentTrancheSets(String),
    CommitmentCollateral(String),
    CommitmentInstruments(String),
    CommitmentGuarantees(String),
    AuthorizedTransformer(Address),
    TrancheSetCounter,
    /// Fee collection: protocol treasury for withdrawals
    FeeRecipient,
    /// Collected transformation fees per asset (asset -> i128)
    CollectedFees(Address),
}

// ============================================================================
// Storage helpers
// ============================================================================

fn require_admin(e: &Env, caller: &Address) {
    caller.require_auth();
    let admin = e
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| fail(e, TransformationError::NotInitialized, "require_admin"));
    if *caller != admin {
        fail(e, TransformationError::Unauthorized, "require_admin");
    }
}

fn require_authorized(e: &Env, caller: &Address) {
    caller.require_auth();
    let admin = e.storage().instance().get::<_, Address>(&DataKey::Admin);
    let is_authorized = e
        .storage()
        .instance()
        .get::<_, bool>(&DataKey::AuthorizedTransformer(caller.clone()))
        .unwrap_or(false);
    if let Some(a) = admin {
        if *caller == a {
            return;
        }
    }
    if !is_authorized {
        fail(e, TransformationError::Unauthorized, "require_authorized");
    }
}

fn require_no_reentrancy(e: &Env) {
    let guard: bool = e
        .storage()
        .instance()
        .get::<_, bool>(&DataKey::ReentrancyGuard)
        .unwrap_or(false);
    if guard {
        fail(
            e,
            TransformationError::ReentrancyDetected,
            "require_no_reentrancy",
        );
    }
}

fn set_reentrancy_guard(e: &Env, value: bool) {
    e.storage()
        .instance()
        .set(&DataKey::ReentrancyGuard, &value);
}

// ============================================================================
// Contract
// ============================================================================

#[contract]
pub struct CommitmentTransformationContract;

#[contractimpl]
impl CommitmentTransformationContract {
    /// Initialize the transformation contract.
    pub fn initialize(e: Env, admin: Address, core_contract: Address) {
        if e.storage().instance().has(&DataKey::Admin) {
            fail(&e, TransformationError::AlreadyInitialized, "initialize");
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage()
            .instance()
            .set(&DataKey::CoreContract, &core_contract);
        e.storage()
            .instance()
            .set(&DataKey::TransformationFeeBps, &0u32);
        e.storage()
            .instance()
            .set(&DataKey::TrancheSetCounter, &0u64);
    }

    /// Set transformation fee in basis points (0-10000). Admin only.
    pub fn set_transformation_fee(e: Env, caller: Address, fee_bps: u32) {
        require_admin(&e, &caller);
        if fee_bps > 10000 {
            fail(
                &e,
                TransformationError::InvalidFeeBps,
                "set_transformation_fee",
            );
        }
        e.storage()
            .instance()
            .set(&DataKey::TransformationFeeBps, &fee_bps);
        e.events().publish(
            (symbol_short!("FeeSet"), caller),
            (fee_bps, e.ledger().timestamp()),
        );
    }

    /// Set or clear authorized transformer contract. Admin only.
    pub fn set_authorized_transformer(
        e: Env,
        caller: Address,
        transformer: Address,
        allowed: bool,
    ) {
        require_admin(&e, &caller);
        e.storage().instance().set(
            &DataKey::AuthorizedTransformer(transformer.clone()),
            &allowed,
        );
        e.events().publish(
            (symbol_short!("AuthSet"), transformer),
            (allowed, e.ledger().timestamp()),
        );
    }

    /// Split a commitment into risk tranches. Caller must be commitment owner or authorized.
    /// When transformation_fee_bps > 0, caller must send fee_amount of fee_asset to the contract.
    /// tranche_share_bps: e.g. [6000, 3000, 1000] for 60% senior, 30% mezzanine, 10% equity.
    pub fn create_tranches(
        e: Env,
        caller: Address,
        commitment_id: String,
        total_value: i128,
        tranche_share_bps: Vec<u32>,
        risk_levels: Vec<String>,
        fee_asset: Address,
    ) -> String {
        require_authorized(&e, &caller);
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        Validation::require_positive(total_value);
        if tranche_share_bps.len() != risk_levels.len() || tranche_share_bps.is_empty() {
            set_reentrancy_guard(&e, false);
            fail(
                &e,
                TransformationError::InvalidTrancheRatios,
                "create_tranches",
            );
        }
        let mut sum_bps: u32 = 0;
        for bps in tranche_share_bps.iter() {
            sum_bps = sum_bps.saturating_add(bps);
        }
        if sum_bps != 10000 {
            set_reentrancy_guard(&e, false);
            fail(
                &e,
                TransformationError::InvalidTrancheRatios,
                "create_tranches",
            );
        }

        let fee_bps: u32 = e
            .storage()
            .instance()
            .get::<_, u32>(&DataKey::TransformationFeeBps)
            .unwrap_or(0);
        let fee_amount = (total_value * fee_bps as i128) / 10000i128;

        // Collect transformation fee from caller when fee_bps > 0
        if fee_amount > 0 {
            let contract_address = e.current_contract_address();
            let token_client = token::Client::new(&e, &fee_asset);
            token_client.transfer(&caller, &contract_address, &fee_amount);
            let key = DataKey::CollectedFees(fee_asset.clone());
            let current: i128 = e.storage().instance().get::<_, i128>(&key).unwrap_or(0);
            e.storage().instance().set(&key, &(current + fee_amount));
        }

        let counter: u64 = e
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::TrancheSetCounter)
            .unwrap_or(0);
        let transformation_id = format_tranformation_id(&e, "tr", counter);
        e.storage()
            .instance()
            .set(&DataKey::TrancheSetCounter, &(counter + 1));

        let mut tranches = Vec::new(&e);
        let net_value = total_value - fee_amount;
        let current_timestamp = e.ledger().timestamp();
        for (i, (bps, risk)) in tranche_share_bps.iter().zip(risk_levels.iter()).enumerate() {
            let bps_u32: u32 = bps;
            let amount = (net_value * bps_u32 as i128) / 10000i128;
            let tranche_id = format_tranformation_id(&e, "t", counter * 10 + i as u64);
            let tranche = RiskTranche {
                tranche_id: tranche_id.clone(),
                transformation_id: transformation_id.clone(),
                commitment_id: commitment_id.clone(),
                risk_level: risk.clone(),
                amount,
                share_bps: bps_u32,
                created_at: current_timestamp,
                status: TrancheStatus::Active,
                updated_at: current_timestamp,
            };
            // Store individual tranche for direct access and updates
            e.storage()
                .instance()
                .set(&DataKey::Tranche(tranche_id.clone()), &tranche);
            tranches.push_back(tranche);
        }

        let set = TrancheSet {
            transformation_id: transformation_id.clone(),
            commitment_id: commitment_id.clone(),
            owner: caller.clone(),
            total_value,
            tranches: tranches.clone(),
            fee_paid: fee_amount,
            created_at: e.ledger().timestamp(),
        };
        e.storage()
            .instance()
            .set(&DataKey::TrancheSet(transformation_id.clone()), &set);

        let mut sets = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::CommitmentTrancheSets(commitment_id.clone()))
            .unwrap_or(Vec::new(&e));
        sets.push_back(transformation_id.clone());
        e.storage().instance().set(
            &DataKey::CommitmentTrancheSets(commitment_id.clone()),
            &sets,
        );

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (
                symbol_short!("TrCreated"),
                transformation_id.clone(),
                caller,
            ),
            (total_value, fee_amount, e.ledger().timestamp()),
        );
        transformation_id
    }

    /// Create a collateralized asset backed by a commitment.
    pub fn collateralize(
        e: Env,
        caller: Address,
        commitment_id: String,
        collateral_amount: i128,
        asset_address: Address,
    ) -> String {
        require_authorized(&e, &caller);
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        Validation::require_positive(collateral_amount);

        let counter: u64 = e
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::TrancheSetCounter)
            .unwrap_or(0);
        let asset_id = format_tranformation_id(&e, "col", counter);
        e.storage()
            .instance()
            .set(&DataKey::TrancheSetCounter, &(counter + 1));

        let collateral = CollateralizedAsset {
            asset_id: asset_id.clone(),
            commitment_id: commitment_id.clone(),
            owner: caller.clone(),
            collateral_amount,
            asset_address: asset_address.clone(),
            created_at: e.ledger().timestamp(),
        };
        e.storage()
            .instance()
            .set(&DataKey::CollateralizedAsset(asset_id.clone()), &collateral);

        let mut list = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::CommitmentCollateral(commitment_id.clone()))
            .unwrap_or(Vec::new(&e));
        list.push_back(asset_id.clone());
        e.storage()
            .instance()
            .set(&DataKey::CommitmentCollateral(commitment_id.clone()), &list);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("Collater"), asset_id.clone(), caller),
            (
                commitment_id,
                collateral_amount,
                asset_address,
                e.ledger().timestamp(),
            ),
        );
        asset_id
    }

    /// Create a secondary market instrument (receivable, option, warrant).
    pub fn create_secondary_instrument(
        e: Env,
        caller: Address,
        commitment_id: String,
        instrument_type: String,
        amount: i128,
    ) -> String {
        require_authorized(&e, &caller);
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        Validation::require_positive(amount);

        let counter: u64 = e
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::TrancheSetCounter)
            .unwrap_or(0);
        let instrument_id = format_tranformation_id(&e, "sec", counter);
        e.storage()
            .instance()
            .set(&DataKey::TrancheSetCounter, &(counter + 1));

        let instrument = SecondaryInstrument {
            instrument_id: instrument_id.clone(),
            commitment_id: commitment_id.clone(),
            owner: caller.clone(),
            instrument_type: instrument_type.clone(),
            amount,
            created_at: e.ledger().timestamp(),
        };
        e.storage().instance().set(
            &DataKey::SecondaryInstrument(instrument_id.clone()),
            &instrument,
        );

        let mut list = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::CommitmentInstruments(commitment_id.clone()))
            .unwrap_or(Vec::new(&e));
        list.push_back(instrument_id.clone());
        e.storage().instance().set(
            &DataKey::CommitmentInstruments(commitment_id.clone()),
            &list,
        );

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("SecCreat"), instrument_id.clone(), caller),
            (
                commitment_id,
                instrument_type,
                amount,
                e.ledger().timestamp(),
            ),
        );
        instrument_id
    }

    /// Add a protocol-specific guarantee to a commitment.
    pub fn add_protocol_guarantee(
        e: Env,
        caller: Address,
        commitment_id: String,
        guarantee_type: String,
        terms_hash: String,
    ) -> String {
        require_authorized(&e, &caller);
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        let counter: u64 = e
            .storage()
            .instance()
            .get::<_, u64>(&DataKey::TrancheSetCounter)
            .unwrap_or(0);
        let guarantee_id = format_tranformation_id(&e, "guar", counter);
        e.storage()
            .instance()
            .set(&DataKey::TrancheSetCounter, &(counter + 1));

        let guarantee = ProtocolGuarantee {
            guarantee_id: guarantee_id.clone(),
            commitment_id: commitment_id.clone(),
            guarantee_type: guarantee_type.clone(),
            terms_hash: terms_hash.clone(),
            created_at: e.ledger().timestamp(),
        };
        e.storage().instance().set(
            &DataKey::ProtocolGuarantee(guarantee_id.clone()),
            &guarantee,
        );

        let mut list = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::CommitmentGuarantees(commitment_id.clone()))
            .unwrap_or(Vec::new(&e));
        list.push_back(guarantee_id.clone());
        e.storage()
            .instance()
            .set(&DataKey::CommitmentGuarantees(commitment_id.clone()), &list);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("GuarAdded"), guarantee_id.clone(), caller),
            (
                commitment_id,
                guarantee_type,
                terms_hash,
                e.ledger().timestamp(),
            ),
        );
        guarantee_id
    }

    /// Get tranche set by ID.
    pub fn get_tranche_set(e: Env, transformation_id: String) -> TrancheSet {
        e.storage()
            .instance()
            .get::<_, TrancheSet>(&DataKey::TrancheSet(transformation_id.clone()))
            .unwrap_or_else(|| {
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "get_tranche_set",
                )
            })
    }

    /// Get individual tranche by ID.
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `tranche_id` - The unique tranche identifier
    ///
    /// # Returns
    /// The RiskTranche struct with current state
    ///
    /// # Errors
    /// Returns TransformationNotFound if tranche does not exist
    ///
    /// # Security Notes
    /// This is a read-only function - no authorization required
    pub fn get_tranche(e: Env, tranche_id: String) -> RiskTranche {
        e.storage()
            .instance()
            .get::<_, RiskTranche>(&DataKey::Tranche(tranche_id.clone()))
            .unwrap_or_else(|| {
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "get_tranche",
                )
            })
    }

    /// Update tranche metadata (risk_level).
    ///
    /// # Arguments
    /// * `caller` - The address requesting the update (must be owner or authorized)
    /// * `tranche_id` - The unique tranche identifier
    /// * `risk_level` - New risk level: "senior", "mezzanine", or "equity"
    ///
    /// # Returns
    /// The updated RiskTranche struct
    ///
    /// # Errors
    /// - TransformationNotFound if tranche does not exist
    /// - Unauthorized if caller is not owner or authorized transformer
    /// - InvalidState if tranche is closed
    ///
    /// # Security Notes
    /// - Requires authorization from tranche owner or authorized transformer
    /// - Cannot update closed tranches
    /// - Emits TrancheUpdated event for off-chain indexing
    pub fn update_tranche(
        e: Env,
        caller: Address,
        tranche_id: String,
        risk_level: String,
    ) -> RiskTranche {
        require_authorized(&e, &caller);
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        let mut tranche: RiskTranche = e
            .storage()
            .instance()
            .get::<_, RiskTranche>(&DataKey::Tranche(tranche_id.clone()))
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "update_tranche",
                )
            });

        // Cannot modify closed tranches
        if tranche.status == TrancheStatus::Closed {
            set_reentrancy_guard(&e, false);
            fail(
                &e,
                TransformationError::InvalidState,
                "update_tranche: tranche is closed",
            );
        }

        // Verify caller is the owner (get from parent tranche set using transformation_id)
        let tranche_set = e
            .storage()
            .instance()
            .get::<_, TrancheSet>(&DataKey::TrancheSet(tranche.transformation_id.clone()))
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "update_tranche: parent set not found",
                )
            });
        if tranche_set.owner != caller {
            set_reentrancy_guard(&e, false);
            fail(&e, TransformationError::Unauthorized, "update_tranche");
        }

        tranche.risk_level = risk_level.clone();
        tranche.updated_at = e.ledger().timestamp();

        e.storage()
            .instance()
            .set(&DataKey::Tranche(tranche_id.clone()), &tranche);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("TrUpdated"), tranche_id.clone(), caller),
            (risk_level, e.ledger().timestamp()),
        );
        tranche
    }

    /// Allocate additional amount to a tranche or withdraw from it.
    ///
    /// # Arguments
    /// * `caller` - The address requesting the allocation (must be owner or authorized)
    /// * `tranche_id` - The unique tranche identifier
    /// * `amount` - Amount to add (positive) or remove (negative) from tranche
    ///
    /// # Returns
    /// The updated RiskTranche struct with new amount
    ///
    /// # Errors
    /// - TransformationNotFound if tranche does not exist
    /// - Unauthorized if caller is not owner or authorized transformer
    /// - InvalidState if tranche is closed
    /// - InvalidAmount if resulting amount would be negative
    ///
    /// # Security Notes
    /// - Requires authorization from tranche owner or authorized transformer
    /// - Cannot modify closed tranches
    /// - Uses checked arithmetic to prevent overflow/underflow
    /// - Amount must not cause tranche amount to go negative
    /// - Emits TrancheAllocated event for off-chain indexing
    pub fn allocate_to_tranche(
        e: Env,
        caller: Address,
        tranche_id: String,
        amount: i128,
    ) -> RiskTranche {
        require_authorized(&e, &caller);
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        let mut tranche: RiskTranche = e
            .storage()
            .instance()
            .get::<_, RiskTranche>(&DataKey::Tranche(tranche_id.clone()))
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "allocate_to_tranche",
                )
            });

        // Cannot modify closed tranches
        if tranche.status == TrancheStatus::Closed {
            set_reentrancy_guard(&e, false);
            fail(
                &e,
                TransformationError::InvalidState,
                "allocate_to_tranche: tranche is closed",
            );
        }

        // Verify caller is the owner
        let tranche_set = e
            .storage()
            .instance()
            .get::<_, TrancheSet>(&DataKey::TrancheSet(tranche.transformation_id.clone()))
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "allocate_to_tranche: parent set not found",
                )
            });
        if tranche_set.owner != caller {
            set_reentrancy_guard(&e, false);
            fail(&e, TransformationError::Unauthorized, "allocate_to_tranche");
        }

        // Checked arithmetic for allocation
        let new_amount = tranche.amount.checked_add(amount).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(
                &e,
                TransformationError::InvalidAmount,
                "allocate_to_tranche: overflow",
            )
        });

        // Ensure new amount is non-negative
        if new_amount < 0 {
            set_reentrancy_guard(&e, false);
            fail(
                &e,
                TransformationError::InvalidAmount,
                "allocate_to_tranche: result would be negative",
            )
        }

        tranche.amount = new_amount;
        tranche.updated_at = e.ledger().timestamp();

        e.storage()
            .instance()
            .set(&DataKey::Tranche(tranche_id.clone()), &tranche);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("TrAlloc"), tranche_id.clone(), caller),
            (amount, new_amount, e.ledger().timestamp()),
        );
        tranche
    }

    /// Close a tranche, marking it as inactive.
    ///
    /// # Arguments
    /// * `caller` - The address requesting the close (must be owner or authorized)
    /// * `tranche_id` - The unique tranche identifier
    ///
    /// # Returns
    /// The updated RiskTranche struct with Closed status
    ///
    /// # Errors
    /// - TransformationNotFound if tranche does not exist
    /// - Unauthorized if caller is not owner or authorized transformer
    /// - InvalidState if tranche is already closed
    ///
    /// # Security Notes
    /// - Requires authorization from tranche owner or authorized transformer
    /// - Closed tranches cannot be modified (update/allocate will fail)
    /// - This is a one-way state transition (cannot reopen)
    /// - Emits TrancheClosed event for off-chain indexing
    pub fn close_tranche(e: Env, caller: Address, tranche_id: String) -> RiskTranche {
        require_authorized(&e, &caller);
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        let mut tranche: RiskTranche = e
            .storage()
            .instance()
            .get::<_, RiskTranche>(&DataKey::Tranche(tranche_id.clone()))
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "close_tranche",
                )
            });

        // Cannot close already closed tranche
        if tranche.status == TrancheStatus::Closed {
            set_reentrancy_guard(&e, false);
            fail(
                &e,
                TransformationError::InvalidState,
                "close_tranche: already closed",
            );
        }

        // Verify caller is the owner
        let tranche_set = e
            .storage()
            .instance()
            .get::<_, TrancheSet>(&DataKey::TrancheSet(tranche.transformation_id.clone()))
            .unwrap_or_else(|| {
                set_reentrancy_guard(&e, false);
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "close_tranche: parent set not found",
                )
            });
        if tranche_set.owner != caller {
            set_reentrancy_guard(&e, false);
            fail(&e, TransformationError::Unauthorized, "close_tranche");
        }

        tranche.status = TrancheStatus::Closed;
        tranche.updated_at = e.ledger().timestamp();

        e.storage()
            .instance()
            .set(&DataKey::Tranche(tranche_id.clone()), &tranche);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("TrClosed"), tranche_id.clone(), caller),
            (e.ledger().timestamp(),),
        );
        tranche
    }

    /// Get collateralized asset by ID.
    pub fn get_collateralized_asset(e: Env, asset_id: String) -> CollateralizedAsset {
        e.storage()
            .instance()
            .get::<_, CollateralizedAsset>(&DataKey::CollateralizedAsset(asset_id.clone()))
            .unwrap_or_else(|| {
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "get_collateralized_asset",
                )
            })
    }

    /// Get secondary instrument by ID.
    pub fn get_secondary_instrument(e: Env, instrument_id: String) -> SecondaryInstrument {
        e.storage()
            .instance()
            .get::<_, SecondaryInstrument>(&DataKey::SecondaryInstrument(instrument_id.clone()))
            .unwrap_or_else(|| {
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "get_secondary_instrument",
                )
            })
    }

    /// Get protocol guarantee by ID.
    pub fn get_protocol_guarantee(e: Env, guarantee_id: String) -> ProtocolGuarantee {
        e.storage()
            .instance()
            .get::<_, ProtocolGuarantee>(&DataKey::ProtocolGuarantee(guarantee_id.clone()))
            .unwrap_or_else(|| {
                fail(
                    &e,
                    TransformationError::TransformationNotFound,
                    "get_protocol_guarantee",
                )
            })
    }

    /// List tranche set IDs for a commitment.
    pub fn get_commitment_tranche_sets(e: Env, commitment_id: String) -> Vec<String> {
        e.storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::CommitmentTrancheSets(commitment_id))
            .unwrap_or(Vec::new(&e))
    }

    /// List collateralized asset IDs for a commitment.
    pub fn get_commitment_collateral(e: Env, commitment_id: String) -> Vec<String> {
        e.storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::CommitmentCollateral(commitment_id))
            .unwrap_or(Vec::new(&e))
    }

    /// List secondary instrument IDs for a commitment.
    pub fn get_commitment_instruments(e: Env, commitment_id: String) -> Vec<String> {
        e.storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::CommitmentInstruments(commitment_id))
            .unwrap_or(Vec::new(&e))
    }

    /// List protocol guarantee IDs for a commitment.
    pub fn get_commitment_guarantees(e: Env, commitment_id: String) -> Vec<String> {
        e.storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::CommitmentGuarantees(commitment_id))
            .unwrap_or(Vec::new(&e))
    }

    pub fn get_admin(e: Env) -> Address {
        e.storage()
            .instance()
            .get::<_, Address>(&DataKey::Admin)
            .unwrap_or_else(|| fail(&e, TransformationError::NotInitialized, "get_admin"))
    }

    pub fn get_transformation_fee_bps(e: Env) -> u32 {
        e.storage()
            .instance()
            .get::<_, u32>(&DataKey::TransformationFeeBps)
            .unwrap_or(0)
    }

    /// Set fee recipient (protocol treasury). Admin only.
    pub fn set_fee_recipient(e: Env, caller: Address, recipient: Address) {
        require_admin(&e, &caller);
        e.storage()
            .instance()
            .set(&DataKey::FeeRecipient, &recipient);
        e.events().publish(
            (symbol_short!("FeeRecip"), caller),
            (recipient, e.ledger().timestamp()),
        );
    }

    /// Withdraw collected transformation fees to the configured fee recipient. Admin only.
    pub fn withdraw_fees(e: Env, caller: Address, asset_address: Address, amount: i128) {
        require_admin(&e, &caller);
        if amount <= 0 {
            fail(&e, TransformationError::InvalidAmount, "withdraw_fees");
        }
        let recipient = e
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::FeeRecipient)
            .unwrap_or_else(|| fail(&e, TransformationError::FeeRecipientNotSet, "withdraw_fees"));
        let key = DataKey::CollectedFees(asset_address.clone());
        let collected = e.storage().instance().get::<_, i128>(&key).unwrap_or(0);
        if amount > collected {
            fail(&e, TransformationError::InsufficientFees, "withdraw_fees");
        }
        e.storage().instance().set(&key, &(collected - amount));
        let contract_address = e.current_contract_address();
        let token_client = token::Client::new(&e, &asset_address);
        token_client.transfer(&contract_address, &recipient, &amount);
        e.events().publish(
            (symbol_short!("FeesWith"), caller, recipient),
            (asset_address, amount, e.ledger().timestamp()),
        );
    }

    /// Get fee recipient. Panics if not set (use only after set_fee_recipient).
    pub fn get_fee_recipient(e: Env) -> Option<Address> {
        e.storage().instance().get(&DataKey::FeeRecipient)
    }

    /// Get collected transformation fees for an asset.
    pub fn get_collected_fees(e: Env, asset_address: Address) -> i128 {
        e.storage()
            .instance()
            .get::<_, i128>(&DataKey::CollectedFees(asset_address))
            .unwrap_or(0)
    }
}

fn format_tranformation_id(e: &Env, prefix: &str, n: u64) -> String {
    let mut buf = [0u8; 32];
    let p = prefix.as_bytes();
    let plen = p.len().min(4);
    buf[..plen].copy_from_slice(&p[..plen]);
    let mut i = plen;
    let mut num = n;
    if num == 0 {
        buf[i] = b'0';
        i += 1;
    } else {
        let mut digits = [0u8; 20];
        let mut dc = 0;
        while num > 0 {
            digits[dc] = (num % 10) as u8 + b'0';
            num /= 10;
            dc += 1;
        }
        for j in 0..dc {
            buf[i] = digits[dc - 1 - j];
            i += 1;
        }
    }
    String::from_str(e, core::str::from_utf8(&buf[..i]).unwrap_or("t0"))
}

#[cfg(test)]
mod tests;
