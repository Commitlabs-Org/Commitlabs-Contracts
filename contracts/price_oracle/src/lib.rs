#![no_std]

//! # Price Oracle contract for CommitLabs
//!
//! ## Summary
//! Provides whitelisted price feeds with validation, time-based validity (staleness),
//! and optional fallback. Used for value calculation, drawdown, compliance, and fees.
//!
//! ## Manipulation Resistance Assumptions
//! This contract is a push-based oracle registry. It assumes:
//! - Oracle addresses added by the admin are trusted to publish honest prices
//! - Consumers enforce freshness through `get_price_valid` and `max_staleness_seconds`
//! - A single whitelisted oracle update can replace the latest value for an asset
//! - There is no medianization, TWAP, quorum, or cross-source reconciliation on-chain
//!
//! Integrators should only use this contract when those trust assumptions are acceptable
//! for their asset and risk model. See:
//! [`docs/THREAT_MODEL.md#price-oracle-manipulation-resistance-assumptions`](../../../docs/THREAT_MODEL.md#price-oracle-manipulation-resistance-assumptions)

use shared_utils::Validation;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
};

pub const CURRENT_VERSION: u32 = 1;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OracleError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    OracleNotWhitelisted = 4,
    PriceNotFound = 5,
    StalePrice = 6,
    InvalidPrice = 7,
    InvalidStaleness = 8,
    InvalidWasmHash = 9,
    InvalidVersion = 10,
    AlreadyMigrated = 11,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
/// Last published price snapshot for a single asset.
///
/// `updated_at` is the ledger timestamp of the accepted oracle write. Consumers that
/// need freshness guarantees should prefer `get_price_valid` instead of trusting
/// `get_price` directly.
pub struct PriceData {
    pub price: i128,
    pub updated_at: u64,
    pub decimals: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
/// Oracle-level configuration used for freshness checks.
pub struct OracleConfig {
    pub max_staleness_seconds: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    /// Default max age (seconds) for price validity (legacy)
    MaxStalenessSeconds,
    /// Whitelist: set of Address that can call set_price
    OracleWhitelist(Address),
    /// Price per asset: asset_address -> PriceData
    Price(Address),
    /// Oracle configuration (v1+)
    OracleConfig,
    /// Contract version
    Version,
}

fn read_admin(e: &Env) -> Address {
    e.storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| panic!("Contract not initialized"))
}


fn is_whitelisted(e: &Env, addr: &Address) -> bool {
    e.storage()
        .instance()
        .get::<_, bool>(&DataKey::OracleWhitelist(addr.clone()))
        .unwrap_or(false)
}

fn require_whitelisted(e: &Env, caller: &Address) {
    caller.require_auth();
    if !is_whitelisted(e, caller) {
        panic!("Oracle not whitelisted");
    }
}

fn read_version(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get::<_, u32>(&DataKey::Version)
        .unwrap_or(0)
}

fn write_version(e: &Env, version: u32) {
    e.storage().instance().set(&DataKey::Version, &version);
}

fn read_config(e: &Env) -> OracleConfig {
    if let Some(config) = e
        .storage()
        .instance()
        .get::<_, OracleConfig>(&DataKey::OracleConfig)
    {
        return config;
    }
    let legacy = e
        .storage()
        .instance()
        .get::<_, u64>(&DataKey::MaxStalenessSeconds)
        .unwrap_or(3600);
    OracleConfig {
        max_staleness_seconds: legacy,
    }
}

fn write_config(e: &Env, config: &OracleConfig) {
    e.storage().instance().set(&DataKey::OracleConfig, config);
}

fn set_max_staleness_internal(e: &Env, seconds: u64) {
    let config = OracleConfig {
        max_staleness_seconds: seconds,
    };
    write_config(e, &config);
    if e.storage().instance().has(&DataKey::MaxStalenessSeconds) {
        e.storage()
            .instance()
            .set(&DataKey::MaxStalenessSeconds, &seconds);
    }
}

fn require_admin_result(e: &Env, caller: &Address) -> Result<(), OracleError> {
    caller.require_auth();
    let admin = e
        .storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .ok_or(OracleError::NotInitialized)?;
    if *caller != admin {
        return Err(OracleError::Unauthorized);
    }
    Ok(())
}

fn require_valid_wasm_hash(e: &Env, wasm_hash: &BytesN<32>) -> Result<(), OracleError> {
    let zero = BytesN::from_array(e, &[0; 32]);
    if *wasm_hash == zero {
        return Err(OracleError::InvalidWasmHash);
    }
    Ok(())
}

#[contract]
/// CommitLabs price oracle contract.
///
/// This contract enforces write authorization with an admin-managed whitelist and
/// protects readers against stale or future-dated prices through `get_price_valid`.
/// It does not attempt to solve market manipulation on-chain beyond those controls.
///
/// Threat model reference:
/// [`docs/THREAT_MODEL.md#price-oracle-manipulation-resistance-assumptions`](../../../docs/THREAT_MODEL.md#price-oracle-manipulation-resistance-assumptions)
pub struct PriceOracleContract;

#[contractimpl]
impl PriceOracleContract {
    /// Initialize the oracle with an admin. Call once.
    ///
    /// Sets the initial trust root for oracle whitelisting and configures the default
    /// freshness window to one hour.
    pub fn initialize(e: Env, admin: Address) -> Result<(), OracleError> {
        if e.storage().instance().has(&DataKey::Admin) {
            return Err(OracleError::AlreadyInitialized);
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
        // Default: price valid for 1 hour
        let config = OracleConfig {
            max_staleness_seconds: 3600,
        };
        write_config(&e, &config);
        write_version(&e, CURRENT_VERSION);
        Ok(())
    }

    /// @notice Add an address to the oracle whitelist (can push prices).
    /// @dev Admin only. Whitelisted addresses are trusted publishers.
    /// @param e Contract environment.
    /// @param caller Must be the admin.
    /// @param oracle_address Address to add to the whitelist.
    /// @return Ok(()) on success, Err(Unauthorized) if not admin.
    /// @security Whitelisted oracles can overwrite the latest price for any asset. A compromised oracle key can replace the latest on-chain price for any asset it updates.
    pub fn add_oracle(e: Env, caller: Address, oracle_address: Address) -> Result<(), OracleError> {
        require_admin_result(&e, &caller)?;
        e.storage()
            .instance()
            .set(&DataKey::OracleWhitelist(oracle_address), &true);
        Ok(())
    }

    /// @notice Remove an address from the oracle whitelist.
    /// @dev Admin only.
    /// @param e Contract environment.
    /// @param caller Must be the admin.
    /// @param oracle_address Address to remove from the whitelist.
    /// @return Ok(()) on success, Err(Unauthorized) if not admin.
    /// @security Prevents further updates from the removed address.
    pub fn remove_oracle(
        e: Env,
        caller: Address,
        oracle_address: Address,
    ) -> Result<(), OracleError> {
        require_admin_result(&e, &caller)?;
        e.storage()
            .instance()
            .remove(&DataKey::OracleWhitelist(oracle_address));
        Ok(())
    }

    /// @notice Check if an address is whitelisted as an oracle.
    /// @dev View function.
    /// @param e Contract environment.
    /// @param address Address to check.
    /// @return True if whitelisted, false otherwise.
    pub fn is_oracle_whitelisted(e: Env, address: Address) -> bool {
        is_whitelisted(&e, &address)
    }

    /// Set price for an asset. Caller must be whitelisted. Validates price >= 0.
    ///
    /// This stores exactly one latest price per asset; it does not aggregate multiple
    /// submissions or compare against external references.
    pub fn set_price(
        e: Env,
        caller: Address,
        asset: Address,
        price: i128,
        decimals: u32,
    ) -> Result<(), OracleError> {
        require_whitelisted(&e, &caller);
        Validation::require_non_negative(price);
        let updated_at = e.ledger().timestamp();
        let data = PriceData {
            price,
            updated_at,
            decimals,
        };
        e.storage()
            .instance()
            .set(&DataKey::Price(asset.clone()), &data);
        e.events().publish(
            (symbol_short!("PriceSet"), asset),
            (price, updated_at, decimals),
        );
        Ok(())
    }

    /// Get last price and timestamp for an asset. Returns `(0, 0, 0)` if not set.
    ///
    /// This function does not enforce freshness. Contracts making security-sensitive
    /// decisions should prefer `get_price_valid`.
    pub fn get_price(e: Env, asset: Address) -> PriceData {
        e.storage()
            .instance()
            .get::<_, PriceData>(&DataKey::Price(asset))
            .unwrap_or(PriceData {
                price: 0,
                updated_at: 0,
                decimals: 0,
            })
    }

    /// Get price if it exists and is not stale; otherwise error.
    ///
    /// Returns `StalePrice` when:
    /// - the current ledger timestamp is later than `updated_at + max_staleness`, or
    /// - the stored `updated_at` is in the future relative to the current ledger
    ///
    /// `max_staleness_override`: if `Some(secs)`, use that instead of the contract default.
    ///
    /// This is the primary reader API for manipulation resistance. Integrators should
    /// choose a staleness window that matches the liquidity and operational assumptions
    /// of their downstream contract.
    pub fn get_price_valid(
        e: Env,
        asset: Address,
        max_staleness_override: Option<u64>,
    ) -> Result<PriceData, OracleError> {
        let data = e
            .storage()
            .instance()
            .get::<_, PriceData>(&DataKey::Price(asset))
            .ok_or(OracleError::PriceNotFound)?;
        if data.price < 0 {
            return Err(OracleError::InvalidPrice);
        }
        let max_staleness =
            max_staleness_override.unwrap_or_else(|| read_config(&e).max_staleness_seconds);
        let now = e.ledger().timestamp();
        if now < data.updated_at || now - data.updated_at > max_staleness {
            return Err(OracleError::StalePrice);
        }
        Ok(data)
    }

    /// @notice Set default max staleness (seconds).
    /// @dev Admin only. Lower values reduce the window in which stale or delayed updates are accepted, but increase the chance of rejecting otherwise usable data during oracle outages.
    /// @param e Contract environment.
    /// @param caller Must be the admin.
    /// @param seconds New staleness window in seconds.
    /// @return Ok(()) on success, Err(Unauthorized) if not admin.
    pub fn set_max_staleness(e: Env, caller: Address, seconds: u64) -> Result<(), OracleError> {
        require_admin_result(&e, &caller)?;
        set_max_staleness_internal(&e, seconds);
        Ok(())
    }

    /// Get max staleness setting used by `get_price_valid` when no override is supplied.
    pub fn get_max_staleness(e: Env) -> u64 {
        read_config(&e).max_staleness_seconds
    }

    /// @notice Get admin address.
    /// @dev View function.
    /// @param e Contract environment.
    /// @return Address of the current admin.
    pub fn get_admin(e: Env) -> Address {
        read_admin(&e)
    }

    /// Get current on-chain version (0 if legacy/uninitialized).
    pub fn get_version(e: Env) -> u32 {
        read_version(&e)
    }

    /// @notice Update admin address (admin-only).
    /// @dev Transfers control over whitelist management and configuration.
    /// @param e Contract environment.
    /// @param caller Must be the current admin.
    /// @param new_admin Address to set as new admin.
    /// @return Ok(()) on success, Err(Unauthorized) if not admin.
    /// @security Only the admin can transfer admin authority.
    pub fn set_admin(e: Env, caller: Address, new_admin: Address) -> Result<(), OracleError> {
        require_admin_result(&e, &caller)?;
        e.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    /// Upgrade contract WASM (admin-only).
    pub fn upgrade(e: Env, caller: Address, new_wasm_hash: BytesN<32>) -> Result<(), OracleError> {
        require_admin_result(&e, &caller)?;
        require_valid_wasm_hash(&e, &new_wasm_hash)?;
        e.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    /// Migrate storage from a previous version to `CURRENT_VERSION` (admin-only).
    pub fn migrate(e: Env, caller: Address, from_version: u32) -> Result<(), OracleError> {
        require_admin_result(&e, &caller)?;

        let stored_version = read_version(&e);
        if stored_version == CURRENT_VERSION {
            return Err(OracleError::AlreadyMigrated);
        }
        if from_version != stored_version || from_version > CURRENT_VERSION {
            return Err(OracleError::InvalidVersion);
        }

        if from_version == 0 {
            let existing = e
                .storage()
                .instance()
                .get::<_, OracleConfig>(&DataKey::OracleConfig);
            let max_staleness_seconds = if let Some(cfg) = existing {
                cfg.max_staleness_seconds
            } else {
                e.storage()
                    .instance()
                    .get::<_, u64>(&DataKey::MaxStalenessSeconds)
                    .unwrap_or(3600)
            };
            let config = OracleConfig {
                max_staleness_seconds,
            };
            write_config(&e, &config);
            e.storage().instance().remove(&DataKey::MaxStalenessSeconds);
        }

        write_version(&e, CURRENT_VERSION);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
