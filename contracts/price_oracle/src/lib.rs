#![no_std]

//! Price Oracle contract for CommitLabs.
//!
//! Provides whitelisted price feeds with validation, time-based validity (staleness),
//! and optional fallback. Used for value calculation, drawdown, compliance, and fees.

use shared_utils::Validation;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env,
};

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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: i128,
    pub updated_at: u64,
    pub decimals: u32,
}

#[contracttype]
pub enum DataKey {
    Admin,
    /// Default max age (seconds) for price validity
    MaxStalenessSeconds,
    /// Whitelist: set of Address that can call set_price
    OracleWhitelist(Address),
    /// Price per asset: asset_address -> PriceData
    Price(Address),
}

fn read_admin(e: &Env) -> Address {
    e.storage()
        .instance()
        .get::<_, Address>(&DataKey::Admin)
        .unwrap_or_else(|| panic!("Contract not initialized"))
}

fn require_admin(e: &Env, caller: &Address) {
    caller.require_auth();
    let admin = read_admin(e);
    if *caller != admin {
        panic!("Unauthorized: admin only");
    }
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

#[contract]
pub struct PriceOracleContract;

#[contractimpl]
impl PriceOracleContract {
    /// Initialize the oracle with an admin. Call once.
    pub fn initialize(e: Env, admin: Address) -> Result<(), OracleError> {
        if e.storage().instance().has(&DataKey::Admin) {
            return Err(OracleError::AlreadyInitialized);
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
        // Default: price valid for 1 hour
        e.storage()
            .instance()
            .set(&DataKey::MaxStalenessSeconds, &3600u64);
        Ok(())
    }

    /// Add an address to the oracle whitelist (can push prices). Admin only.
    pub fn add_oracle(e: Env, caller: Address, oracle_address: Address) -> Result<(), OracleError> {
        require_admin(&e, &caller);
        e.storage()
            .instance()
            .set(&DataKey::OracleWhitelist(oracle_address), &true);
        Ok(())
    }

    /// Remove an address from the whitelist. Admin only.
    pub fn remove_oracle(
        e: Env,
        caller: Address,
        oracle_address: Address,
    ) -> Result<(), OracleError> {
        require_admin(&e, &caller);
        e.storage()
            .instance()
            .remove(&DataKey::OracleWhitelist(oracle_address));
        Ok(())
    }

    /// Check if an address is whitelisted.
    pub fn is_oracle_whitelisted(e: Env, address: Address) -> bool {
        is_whitelisted(&e, &address)
    }

    /// Set price for an asset. Caller must be whitelisted. Validates price >= 0.
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

    /// Get last price and timestamp for an asset. Returns (0, 0, 0) if not set.
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
    /// `max_staleness_override`: if Some(secs), use instead of contract default.
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
        let max_staleness = max_staleness_override.unwrap_or_else(|| {
            e.storage()
                .instance()
                .get::<_, u64>(&DataKey::MaxStalenessSeconds)
                .unwrap_or(3600)
        });
        let now = e.ledger().timestamp();
        if now < data.updated_at || now - data.updated_at > max_staleness {
            return Err(OracleError::StalePrice);
        }
        Ok(data)
    }

    /// Set default max staleness (seconds). Admin only.
    pub fn set_max_staleness(e: Env, caller: Address, seconds: u64) -> Result<(), OracleError> {
        require_admin(&e, &caller);
        e.storage()
            .instance()
            .set(&DataKey::MaxStalenessSeconds, &seconds);
        Ok(())
    }

    /// Get max staleness setting.
    pub fn get_max_staleness(e: Env) -> u64 {
        e.storage()
            .instance()
            .get::<_, u64>(&DataKey::MaxStalenessSeconds)
            .unwrap_or(3600)
    }

    /// Get admin address.
    pub fn get_admin(e: Env) -> Address {
        read_admin(&e)
    }
}

#[cfg(test)]
mod tests;
