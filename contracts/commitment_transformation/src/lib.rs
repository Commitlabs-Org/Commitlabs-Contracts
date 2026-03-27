//! # Commitment Transformation Contract
//!
//! Transforms commitments into risk tranches, collateralized assets,
//! and secondary market instruments with protocol-specific guarantees.
//!
//! ## Trust Boundaries
//! - **Admin**: sole authority over fee settings, fee-recipient, and transformer allowlist.
//! - **Authorized transformers**: may call `create_tranches`, `collateralize`,
//!   `create_secondary_instrument`, and `add_protocol_guarantee`.
//! - **Anyone**: read-only getters only.
//!
//! ## Storage Mutation Summary
//! | Key | Mutated by |
//! |-----|-----------|
//! | `Admin` | `initialize` |
//! | `CoreContract` | `initialize` |
//! | `TransformationFeeBps` | `initialize`, `set_transformation_fee` |
//! | `FeeRecipient` | `set_fee_recipient` |
//! | `AuthorizedTransformer(addr)` | `set_authorized_transformer` |
//! | `TrancheSet(id)` | `create_tranches` |
//! | `CollateralizedAsset(id)` | `collateralize` |
//! | `SecondaryInstrument(id)` | `create_secondary_instrument` |
//! | `ProtocolGuarantee(id)` | `add_protocol_guarantee` |
//! | `CollectedFees(asset)` | `create_tranches` (accumulate), `withdraw_fees` (drain) |
//! | `ReentrancyGuard` | all state-mutating calls |
//!
//! ## Arithmetic Safety
//! All fee and tranche calculations use `i128` arithmetic.  The only
//! potentially surprising truncation is integer division:
//! `fee = (total_value * fee_bps) / 10_000` and
//! `tranche_amount = (net_value * bps) / 10_000`.
//! Both round toward zero (floor for positive values), meaning dust
//! amounts may be retained in the contract.  Callers should be aware
//! that the sum of tranche amounts can be up to `n – 1` stroops less
//! than `net_value` where `n` is the number of tranches.

#![no_std]

use shared_utils::{emit_error_event, Validation};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env, String,
    Vec,
};

// ============================================================================
// Errors (aligned with shared_utils::error_codes)
// ============================================================================

/// All error conditions that the transformation contract can surface.
///
/// Each discriminant maps to the same integer as the
/// `shared_utils::error_codes` table so that off-chain observers can
/// decode them uniformly.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TransformationError {
    /// Amount argument is zero or negative.
    InvalidAmount = 1,
    /// Tranche BPS array does not sum to 10 000, is empty, or has a
    /// different length than the `risk_levels` array.
    InvalidTrancheRatios = 2,
    /// `fee_bps` argument exceeds 10 000 (100 %).
    InvalidFeeBps = 3,
    /// Caller is not the admin and is not in the authorized-transformer list.
    Unauthorized = 4,
    /// A function requiring prior initialization was called before
    /// `initialize`.
    NotInitialized = 5,
    /// `initialize` was called on an already-initialized contract.
    AlreadyInitialized = 6,
    /// Referenced commitment ID does not exist in the core contract.
    /// Reserved for future cross-contract commitment validation.
    CommitmentNotFound = 7,
    /// The requested transformation, collateral, instrument, or guarantee
    /// record does not exist in storage.
    TransformationNotFound = 8,
    /// The commitment or transformation is in a state that does not allow
    /// the requested operation.
    /// Reserved for future lifecycle-enforcement logic.
    InvalidState = 9,
    /// A re-entrant call was detected via the in-storage reentrancy guard.
    ReentrancyDetected = 10,
    /// `withdraw_fees` was called but `set_fee_recipient` has never been
    /// called on this contract.
    FeeRecipientNotSet = 11,
    /// `withdraw_fees` requested more than the currently collected balance
    /// for the given asset.
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskTranche {
    pub tranche_id: String,
    pub commitment_id: String,
    pub risk_level: String, // "senior", "mezzanine", "equity"
    pub amount: i128,
    pub share_bps: u32,
    pub created_at: u64,
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
    ///
    /// # Parameters
    /// - `admin` – Address that will own admin privileges (fee configuration,
    ///   transformer allowlist, fee withdrawal).
    /// - `core_contract` – Address of the `commitment_core` contract.
    ///   Stored for future cross-contract commitment validation.
    ///
    /// # Errors
    /// - [`TransformationError::AlreadyInitialized`] if called more than once.
    ///
    /// # Security
    /// No auth is required to call `initialize`, but the `admin` address
    /// supplied here becomes the sole privileged actor thereafter.  Deploy
    /// scripts must call this immediately after contract deployment to
    /// prevent a front-running attack.
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

    /// Set the protocol fee charged on each tranche creation.
    ///
    /// # Parameters
    /// - `caller` – Must be the admin; `require_auth` is enforced.
    /// - `fee_bps` – Fee in basis points (0 – 10 000).  10 000 = 100 %.
    ///
    /// # Errors
    /// - [`TransformationError::Unauthorized`] if `caller` is not the admin.
    /// - [`TransformationError::InvalidFeeBps`] if `fee_bps > 10_000`.
    ///
    /// # Events
    /// Emits `("FeeSet", caller) → (fee_bps, timestamp)`.
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

    /// Grant or revoke authorization for a transformer address.
    ///
    /// # Parameters
    /// - `caller` – Must be the admin; `require_auth` is enforced.
    /// - `transformer` – Address to authorize or revoke.
    /// - `allowed` – `true` to grant, `false` to revoke.
    ///
    /// # Errors
    /// - [`TransformationError::Unauthorized`] if `caller` is not the admin.
    ///
    /// # Events
    /// Emits `("AuthSet", transformer) → (allowed, timestamp)`.
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

    /// Split a commitment into a set of risk tranches.
    ///
    /// # Parameters
    /// - `caller` – Must be authorized (admin or in transformer allowlist);
    ///   `require_auth` is enforced.
    /// - `commitment_id` – Identifier of the underlying commitment.
    /// - `total_value` – Gross value being tranched (in asset base units,
    ///   must be > 0).
    /// - `tranche_share_bps` – Per-tranche allocation in basis points.
    ///   Must be non-empty, same length as `risk_levels`, and sum to
    ///   exactly 10 000.
    /// - `risk_levels` – Human-readable risk label per tranche, e.g.
    ///   `"senior"`, `"mezzanine"`, `"equity"`.
    /// - `fee_asset` – Token contract used to collect the transformation
    ///   fee.  Only a real token transfer is performed when
    ///   `transformation_fee_bps > 0`.
    ///
    /// # Returns
    /// The generated `transformation_id` (opaque string key).
    ///
    /// # Errors
    /// - [`TransformationError::Unauthorized`] – caller not authorized.
    /// - [`TransformationError::ReentrancyDetected`] – nested call guard
    ///   (should be unreachable in normal operation).
    /// - [`TransformationError::InvalidTrancheRatios`] – empty array, length
    ///   mismatch, or BPS sum ≠ 10 000.
    /// - Panics via [`shared_utils::Validation::require_positive`] if
    ///   `total_value ≤ 0`.
    ///
    /// # Events
    /// Emits `("TrCreated", transformation_id, caller) → (total_value, fee_amount, timestamp)`.
    ///
    /// # Security
    /// Reentrancy-guarded.  Fee transfer is performed as an external
    /// interaction *inside* the guard; state is finalized before the guard
    /// is released.
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
        if tranche_share_bps.len() != risk_levels.len() || tranche_share_bps.len() == 0 {
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
        for (i, (bps, risk)) in tranche_share_bps.iter().zip(risk_levels.iter()).enumerate() {
            let bps_u32: u32 = bps;
            let amount = (net_value * bps_u32 as i128) / 10000i128;
            let tranche_id = format_tranformation_id(&e, "t", counter * 10 + i as u64);
            tranches.push_back(RiskTranche {
                tranche_id: tranche_id.clone(),
                commitment_id: commitment_id.clone(),
                risk_level: risk.clone(),
                amount,
                share_bps: bps_u32,
                created_at: e.ledger().timestamp(),
            });
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

    /// Create a collateralized asset record backed by a commitment.
    ///
    /// # Parameters
    /// - `caller` – Must be authorized; `require_auth` is enforced.
    /// - `commitment_id` – Identifier of the backing commitment.
    /// - `collateral_amount` – Amount of `asset_address` tokens pledged
    ///   (must be > 0).
    /// - `asset_address` – Token contract address of the collateral asset.
    ///
    /// # Returns
    /// The generated `asset_id` (opaque string key).
    ///
    /// # Errors
    /// - [`TransformationError::Unauthorized`] – caller not authorized.
    /// - [`TransformationError::ReentrancyDetected`] – reentrancy guard.
    /// - Panics via `Validation::require_positive` if `collateral_amount ≤ 0`.
    ///
    /// # Events
    /// Emits `("Collater", asset_id, caller) → (commitment_id, collateral_amount, asset_address, timestamp)`.
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
    ///
    /// # Parameters
    /// - `caller` – Must be authorized; `require_auth` is enforced.
    /// - `commitment_id` – Identifier of the underlying commitment.
    /// - `instrument_type` – Instrument class, e.g. `"receivable"`,
    ///   `"option"`, `"warrant"`.
    /// - `amount` – Face/notional amount (must be > 0).
    ///
    /// # Returns
    /// The generated `instrument_id` (opaque string key).
    ///
    /// # Errors
    /// - [`TransformationError::Unauthorized`] – caller not authorized.
    /// - [`TransformationError::ReentrancyDetected`] – reentrancy guard.
    /// - Panics via `Validation::require_positive` if `amount ≤ 0`.
    ///
    /// # Events
    /// Emits `("SecCreat", instrument_id, caller) → (commitment_id, instrument_type, amount, timestamp)`.
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

    /// Attach a protocol-specific guarantee to a commitment.
    ///
    /// # Parameters
    /// - `caller` – Must be authorized; `require_auth` is enforced.
    /// - `commitment_id` – Target commitment.
    /// - `guarantee_type` – Guarantee category, e.g. `"liquidity_backstop"`.
    /// - `terms_hash` – Off-chain content hash of the guarantee terms.
    ///
    /// # Returns
    /// The generated `guarantee_id` (opaque string key).
    ///
    /// # Errors
    /// - [`TransformationError::Unauthorized`] – caller not authorized.
    /// - [`TransformationError::ReentrancyDetected`] – reentrancy guard.
    ///
    /// # Events
    /// Emits `("GuarAdded", guarantee_id, caller) → (commitment_id, guarantee_type, terms_hash, timestamp)`.
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

    /// Fetch a [`TrancheSet`] by its `transformation_id`.
    ///
    /// # Errors
    /// - [`TransformationError::TransformationNotFound`] if the ID is unknown.
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

    /// Fetch a [`CollateralizedAsset`] by its `asset_id`.
    ///
    /// # Errors
    /// - [`TransformationError::TransformationNotFound`] if the ID is unknown.
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

    /// Fetch a [`SecondaryInstrument`] by its `instrument_id`.
    ///
    /// # Errors
    /// - [`TransformationError::TransformationNotFound`] if the ID is unknown.
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

    /// Fetch a [`ProtocolGuarantee`] by its `guarantee_id`.
    ///
    /// # Errors
    /// - [`TransformationError::TransformationNotFound`] if the ID is unknown.
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

    /// Set the fee recipient (protocol treasury) for fee withdrawals.
    ///
    /// # Parameters
    /// - `caller` – Must be the admin; `require_auth` is enforced.
    /// - `recipient` – Address that will receive withdrawn fees.
    ///
    /// # Errors
    /// - [`TransformationError::Unauthorized`] if `caller` is not the admin.
    ///
    /// # Events
    /// Emits `("FeeRecip", caller) → (recipient, timestamp)`.
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

    /// Withdraw collected transformation fees to the configured fee recipient.
    ///
    /// # Parameters
    /// - `caller` – Must be the admin; `require_auth` is enforced.
    /// - `asset_address` – Token contract whose collected balance to draw
    ///   from.
    /// - `amount` – Amount to transfer (must be > 0, ≤ collected balance).
    ///
    /// # Errors
    /// - [`TransformationError::Unauthorized`] – caller not the admin.
    /// - [`TransformationError::InvalidAmount`] – `amount ≤ 0`.
    /// - [`TransformationError::FeeRecipientNotSet`] – `set_fee_recipient`
    ///   has never been called.
    /// - [`TransformationError::InsufficientFees`] – `amount` exceeds the
    ///   collected balance for `asset_address`.
    ///
    /// # Events
    /// Emits `("FeesWith", caller, recipient) → (asset_address, amount, timestamp)`.
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
