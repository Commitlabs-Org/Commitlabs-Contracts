//! Commitment Transformation contract (#57).
//!
//! Transforms commitments into risk tranches, collateralized assets,
//! and secondary market instruments with protocol-specific guarantees.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};
use shared_utils::{Validation, emit_error_event};

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
        fail(e, TransformationError::ReentrancyDetected, "require_no_reentrancy");
    }
}

fn set_reentrancy_guard(e: &Env, value: bool) {
    e.storage().instance().set(&DataKey::ReentrancyGuard, &value);
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
        e.storage().instance().set(&DataKey::CoreContract, &core_contract);
        e.storage().instance().set(&DataKey::TransformationFeeBps, &0u32);
        e.storage().instance().set(&DataKey::TrancheSetCounter, &0u64);
    }

    /// Set transformation fee in basis points (0-10000). Admin only.
    pub fn set_transformation_fee(e: Env, caller: Address, fee_bps: u32) {
        require_admin(&e, &caller);
        if fee_bps > 10000 {
            fail(&e, TransformationError::InvalidFeeBps, "set_transformation_fee");
        }
        e.storage().instance().set(&DataKey::TransformationFeeBps, &fee_bps);
        e.events().publish(
            (symbol_short!("FeeSet"), caller),
            (fee_bps, e.ledger().timestamp()),
        );
    }

    /// Set or clear authorized transformer contract. Admin only.
    pub fn set_authorized_transformer(e: Env, caller: Address, transformer: Address, allowed: bool) {
        require_admin(&e, &caller);
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedTransformer(transformer.clone()), &allowed);
        e.events().publish(
            (symbol_short!("AuthSet"), transformer),
            (allowed, e.ledger().timestamp()),
        );
    }

    /// Split a commitment into risk tranches. Caller must be commitment owner or authorized.
    /// tranche_share_bps: e.g. [6000, 3000, 1000] for 60% senior, 30% mezzanine, 10% equity.
    pub fn create_tranches(
        e: Env,
        caller: Address,
        commitment_id: String,
        total_value: i128,
        tranche_share_bps: Vec<u32>,
        risk_levels: Vec<String>,
    ) -> String {
        require_authorized(&e, &caller);
        require_no_reentrancy(&e);
        set_reentrancy_guard(&e, true);

        Validation::require_positive(total_value);
        if tranche_share_bps.len() != risk_levels.len() || tranche_share_bps.len() == 0 {
            set_reentrancy_guard(&e, false);
            fail(&e, TransformationError::InvalidTrancheRatios, "create_tranches");
        }
        let mut sum_bps: u32 = 0;
        for bps in tranche_share_bps.iter() {
            sum_bps = sum_bps.saturating_add(bps);
        }
        if sum_bps != 10000 {
            set_reentrancy_guard(&e, false);
            fail(&e, TransformationError::InvalidTrancheRatios, "create_tranches");
        }

        let fee_bps: u32 = e
            .storage()
            .instance()
            .get::<_, u32>(&DataKey::TransformationFeeBps)
            .unwrap_or(0);
        let fee_amount = (total_value * fee_bps as i128) / 10000i128;

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
        e.storage()
            .instance()
            .set(&DataKey::CommitmentTrancheSets(commitment_id.clone()), &sets);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("TrCreated"), transformation_id.clone(), caller),
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
            (commitment_id, collateral_amount, asset_address, e.ledger().timestamp()),
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
        e.storage()
            .instance()
            .set(&DataKey::SecondaryInstrument(instrument_id.clone()), &instrument);

        let mut list = e
            .storage()
            .instance()
            .get::<_, Vec<String>>(&DataKey::CommitmentInstruments(commitment_id.clone()))
            .unwrap_or(Vec::new(&e));
        list.push_back(instrument_id.clone());
        e.storage()
            .instance()
            .set(&DataKey::CommitmentInstruments(commitment_id.clone()), &list);

        set_reentrancy_guard(&e, false);
        e.events().publish(
            (symbol_short!("SecCreat"), instrument_id.clone(), caller),
            (commitment_id, instrument_type, amount, e.ledger().timestamp()),
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
        e.storage()
            .instance()
            .set(&DataKey::ProtocolGuarantee(guarantee_id.clone()), &guarantee);

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
            (commitment_id, guarantee_type, terms_hash, e.ledger().timestamp()),
        );
        guarantee_id
    }

    /// Get tranche set by ID.
    pub fn get_tranche_set(e: Env, transformation_id: String) -> TrancheSet {
        e.storage()
            .instance()
            .get::<_, TrancheSet>(&DataKey::TrancheSet(transformation_id.clone()))
            .unwrap_or_else(|| fail(&e, TransformationError::TransformationNotFound, "get_tranche_set"))
    }

    /// Get collateralized asset by ID.
    pub fn get_collateralized_asset(e: Env, asset_id: String) -> CollateralizedAsset {
        e.storage()
            .instance()
            .get::<_, CollateralizedAsset>(&DataKey::CollateralizedAsset(asset_id.clone()))
            .unwrap_or_else(|| fail(&e, TransformationError::TransformationNotFound, "get_collateralized_asset"))
    }

    /// Get secondary instrument by ID.
    pub fn get_secondary_instrument(e: Env, instrument_id: String) -> SecondaryInstrument {
        e.storage()
            .instance()
            .get::<_, SecondaryInstrument>(&DataKey::SecondaryInstrument(instrument_id.clone()))
            .unwrap_or_else(|| fail(&e, TransformationError::TransformationNotFound, "get_secondary_instrument"))
    }

    /// Get protocol guarantee by ID.
    pub fn get_protocol_guarantee(e: Env, guarantee_id: String) -> ProtocolGuarantee {
        e.storage()
            .instance()
            .get::<_, ProtocolGuarantee>(&DataKey::ProtocolGuarantee(guarantee_id.clone()))
            .unwrap_or_else(|| fail(&e, TransformationError::TransformationNotFound, "get_protocol_guarantee"))
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
