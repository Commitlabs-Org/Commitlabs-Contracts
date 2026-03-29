//! Commitment Transformation contract (#57).
//!
//! Transforms commitments into risk tranches, collateralized assets,
//! and secondary market instruments with protocol-specific guarantees.
//!
//! Authorization model:
//! - commitment owners may create owner-bound transformations for their own commitments
//! - admin may configure the contract and may also execute owner-bound transformations
//! - authorized protocol transformers may execute owner-bound transformations and add guarantees
//! - protocol guarantees are reserved to protocol roles and are not owner-callable

#![no_std]

use shared_utils::{emit_error_event, fees, Validation};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Env,
    IntoVal, String, Symbol, TryIntoVal, Val, Vec,
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
struct CoreCommitmentRules {
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String,
    pub early_exit_penalty: u32,
    pub min_fee_threshold: i128,
    pub grace_period_days: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
struct CoreCommitment {
    pub commitment_id: String,
    pub owner: Address,
    pub nft_token_id: u32,
    pub rules: CoreCommitmentRules,
    pub amount: i128,
    pub asset_address: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub current_value: i128,
    pub status: String,
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
    let admin = read_admin(e);
    if *caller != admin {
        fail(e, TransformationError::Unauthorized, "require_admin");
    }
}

fn read_admin(e: &Env) -> Address {
    e.storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| fail(e, TransformationError::NotInitialized, "read_admin"))
}

fn read_core_contract(e: &Env) -> Address {
    e.storage()
        .instance()
        .get::<_, Address>(&DataKey::CoreContract)
        .unwrap_or_else(|| fail(e, TransformationError::NotInitialized, "read_core_contract"))
}

fn is_authorized_transformer_address(e: &Env, caller: &Address) -> bool {
    e.storage()
        .instance()
        .get::<_, bool>(&DataKey::AuthorizedTransformer(caller.clone()))
        .unwrap_or(false)
}

fn is_protocol_role(e: &Env, caller: &Address) -> bool {
    *caller == read_admin(e) || is_authorized_transformer_address(e, caller)
}

fn load_commitment(e: &Env, commitment_id: &String) -> CoreCommitment {
    let core_contract = read_core_contract(e);
    let mut args = Vec::new(e);
    args.push_back(commitment_id.clone().into_val(e));

    let commitment_val: Val = match e.try_invoke_contract::<Val, soroban_sdk::Error>(
        &core_contract,
        &Symbol::new(e, "get_commitment"),
        args,
    ) {
        Ok(Ok(val)) => val,
        _ => fail(e, TransformationError::CommitmentNotFound, "load_commitment"),
    };

    commitment_val
        .try_into_val(e)
        .unwrap_or_else(|_| fail(e, TransformationError::CommitmentNotFound, "load_commitment"))
}

fn require_owner_or_protocol(e: &Env, caller: &Address, commitment_id: &String) -> CoreCommitment {
    caller.require_auth();
    let commitment = load_commitment(e, commitment_id);
    if *caller == commitment.owner || is_protocol_role(e, caller) {
        return commitment;
    }

    fail(e, TransformationError::Unauthorized, "require_owner_or_protocol");
}

fn require_protocol_role(e: &Env, caller: &Address) {
    caller.require_auth();
    if !is_protocol_role(e, caller) {
        fail(e, TransformationError::Unauthorized, "require_protocol_role");
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
    /// - `admin`: configuration authority for fees and protocol role assignment.
    /// - `core_contract`: canonical `commitment_core` contract used to resolve commitment ownership.
    ///
    /// # Errors
    /// - Panics with `AlreadyInitialized` if called more than once.
    ///
    /// # Security
    /// - Single-use initializer.
    /// - Stores the trust boundary used for owner resolution on all transformation writes.
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
        e.storage().instance().set(&DataKey::ReentrancyGuard, &false);
        e.storage()
            .instance()
            .set(&DataKey::TrancheSetCounter, &0u64);
    }

    /// Set the transformation fee in basis points.
    ///
    /// # Parameters
    /// - `caller`: must match the stored admin address.
    /// - `fee_bps`: fee rate in the inclusive range `0..=10000`.
    ///
    /// # Errors
    /// - `Unauthorized` if `caller` is not admin.
    /// - `InvalidFeeBps` if `fee_bps > 10000`.
    ///
    /// # Security
    /// - Admin-only configuration.
    /// - Fee math later uses checked basis-point helpers from `shared_utils::fees`.
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

    /// Add or remove a protocol transformer role.
    ///
    /// # Parameters
    /// - `caller`: must match the stored admin address.
    /// - `transformer`: address granted or revoked protocol-executor privileges.
    /// - `allowed`: `true` to authorize, `false` to revoke.
    ///
    /// # Errors
    /// - `Unauthorized` if `caller` is not admin.
    ///
    /// # Security
    /// - Admin-only role management.
    /// - Protocol transformers may execute owner-bound transformations and create guarantees.
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

    /// Split a commitment into risk tranches.
    ///
    /// # Parameters
    /// - `caller`: commitment owner, admin, or authorized transformer.
    /// - `commitment_id`: canonical commitment identifier resolved through `commitment_core`.
    /// - `total_value`: gross value to split before fees.
    /// - `tranche_share_bps`: tranche shares in basis points; must sum to `10000`.
    /// - `risk_levels`: tranche labels parallel to `tranche_share_bps`.
    /// - `fee_asset`: asset collected when a transformation fee is configured.
    ///
    /// # Errors
    /// - `CommitmentNotFound` if `commitment_core` has no such commitment.
    /// - `Unauthorized` if `caller` is neither owner nor protocol role.
    /// - `InvalidAmount` if `total_value <= 0`.
    /// - `InvalidTrancheRatios` if lengths mismatch or shares do not sum to `10000`.
    ///
    /// # Security
    /// - Reads commitment ownership from the configured core contract before mutating storage.
    /// - Uses a reentrancy guard around fee collection and state writes.
    /// - Fee math uses checked basis-point helpers and rounds down in the protocol's favor.
    /// - Stored `owner` is always the canonical commitment owner, even when a protocol actor executes.
    pub fn create_tranches(
        e: Env,
        caller: Address,
        commitment_id: String,
        total_value: i128,
        tranche_share_bps: Vec<u32>,
        risk_levels: Vec<String>,
        fee_asset: Address,
    ) -> String {
        let commitment = require_owner_or_protocol(&e, &caller, &commitment_id);
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
            sum_bps = sum_bps
                .checked_add(bps)
                .unwrap_or_else(|| {
                    set_reentrancy_guard(&e, false);
                    fail(
                        &e,
                        TransformationError::InvalidTrancheRatios,
                        "create_tranches",
                    )
                });
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
        let fee_amount = fees::fee_from_bps(total_value, fee_bps);

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
        let net_value = total_value.checked_sub(fee_amount).unwrap_or_else(|| {
            set_reentrancy_guard(&e, false);
            fail(&e, TransformationError::InvalidAmount, "create_tranches")
        });
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
            owner: commitment.owner.clone(),
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
    ///
    /// # Parameters
    /// - `caller`: commitment owner, admin, or authorized transformer.
    /// - `commitment_id`: canonical commitment identifier.
    /// - `collateral_amount`: positive amount represented by the derived asset.
    /// - `asset_address`: asset contract associated with the derived position.
    ///
    /// # Errors
    /// - `CommitmentNotFound` if the core contract cannot resolve the commitment.
    /// - `Unauthorized` if `caller` is neither owner nor protocol role.
    /// - `InvalidAmount` if `collateral_amount <= 0`.
    ///
    /// # Security
    /// - Owner is sourced from `commitment_core`, not from the caller.
    /// - Uses a reentrancy guard even though the current flow has no outbound contract writes.
    pub fn collateralize(
        e: Env,
        caller: Address,
        commitment_id: String,
        collateral_amount: i128,
        asset_address: Address,
    ) -> String {
        let commitment = require_owner_or_protocol(&e, &caller, &commitment_id);
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
            owner: commitment.owner.clone(),
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

    /// Create a secondary market instrument.
    ///
    /// # Parameters
    /// - `caller`: commitment owner, admin, or authorized transformer.
    /// - `commitment_id`: canonical commitment identifier.
    /// - `instrument_type`: free-form instrument label such as `receivable`, `option`, or `warrant`.
    /// - `amount`: positive face amount of the derived instrument.
    ///
    /// # Errors
    /// - `CommitmentNotFound` if the core contract cannot resolve the commitment.
    /// - `Unauthorized` if `caller` is neither owner nor protocol role.
    /// - `InvalidAmount` if `amount <= 0`.
    ///
    /// # Security
    /// - Owner is sourced from `commitment_core`, not from the caller.
    /// - Uses a reentrancy guard around storage mutation.
    pub fn create_secondary_instrument(
        e: Env,
        caller: Address,
        commitment_id: String,
        instrument_type: String,
        amount: i128,
    ) -> String {
        let commitment = require_owner_or_protocol(&e, &caller, &commitment_id);
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
            owner: commitment.owner.clone(),
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
    ///
    /// # Parameters
    /// - `caller`: admin or authorized transformer.
    /// - `commitment_id`: canonical commitment identifier.
    /// - `guarantee_type`: protocol-defined guarantee class.
    /// - `terms_hash`: immutable reference to off-chain or versioned guarantee terms.
    ///
    /// # Errors
    /// - `CommitmentNotFound` if the core contract cannot resolve the commitment.
    /// - `Unauthorized` if `caller` is not a protocol role.
    ///
    /// # Security
    /// - Guarantees are protocol-controlled metadata; commitment owners cannot mint them directly.
    /// - Uses a reentrancy guard around storage mutation.
    pub fn add_protocol_guarantee(
        e: Env,
        caller: Address,
        commitment_id: String,
        guarantee_type: String,
        terms_hash: String,
    ) -> String {
        require_protocol_role(&e, &caller);
        load_commitment(&e, &commitment_id);
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
        read_admin(&e)
    }

    /// Return the configured canonical core contract used for owner resolution.
    pub fn get_core_contract(e: Env) -> Address {
        read_core_contract(&e)
    }

    /// Return whether an address currently has the authorized transformer role.
    pub fn is_authorized_transformer(e: Env, address: Address) -> bool {
        is_authorized_transformer_address(&e, &address)
    }

    /// Return the configured transformation fee in basis points.
    pub fn get_transformation_fee_bps(e: Env) -> u32 {
        e.storage()
            .instance()
            .get::<_, u32>(&DataKey::TransformationFeeBps)
            .unwrap_or(0)
    }

    /// Set the fee recipient treasury.
    ///
    /// # Parameters
    /// - `caller`: must match the stored admin address.
    /// - `recipient`: treasury address that receives future fee withdrawals.
    ///
    /// # Errors
    /// - `Unauthorized` if `caller` is not admin.
    ///
    /// # Security
    /// - Admin-only configuration.
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

    /// Withdraw collected transformation fees to the configured treasury.
    ///
    /// # Parameters
    /// - `caller`: must match the stored admin address.
    /// - `asset_address`: asset bucket to withdraw from.
    /// - `amount`: positive amount, capped by `CollectedFees(asset_address)`.
    ///
    /// # Errors
    /// - `Unauthorized` if `caller` is not admin.
    /// - `InvalidAmount` if `amount <= 0`.
    /// - `FeeRecipientNotSet` if no treasury has been configured.
    /// - `InsufficientFees` if `amount` exceeds collected fees for that asset.
    ///
    /// # Security
    /// - Admin-only payout path.
    /// - Uses stored per-asset fee buckets and token transfer after state decrement.
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
