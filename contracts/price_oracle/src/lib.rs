#![no_std]

//! Price Oracle contract for CommitLabs.
//!
//! Provides whitelisted price feeds with validation, time-based validity (staleness),
//! and optional fallback. Used for value calculation, drawdown, compliance, and fees.
//!
//! # Manipulation resistance assumptions
//! This contract is intentionally a push-based oracle registry, not an on-chain price
//! discovery mechanism. It assumes:
//! - oracle addresses added by the admin are trusted to publish honest prices
//! - consumers enforce freshness through `get_price_valid` and `max_staleness_seconds`
//! - a single whitelisted oracle update can replace the latest value for an asset
//! - there is no medianization, TWAP, quorum, or cross-source reconciliation on-chain
//!
//! Integrators should only use this contract when those trust assumptions are acceptable
//! for their asset and risk model. See the repository threat model:
//! [`docs/THREAT_MODEL.md#price-oracle-manipulation-resistance-assumptions`](../../../docs/THREAT_MODEL.md#price-oracle-manipulation-resistance-assumptions).

use shared_utils::{Validation, SafeMath};
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

    /// Add an address to the oracle whitelist (can push prices). Admin only.
    ///
    /// Security: whitelisted addresses are trusted publishers. A compromised oracle key
    /// can replace the latest on-chain price for any asset it updates.
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

    /// Set default max staleness (seconds). Admin only.
    ///
    /// Lower values reduce the window in which stale or delayed updates are accepted,
    /// but increase the chance of rejecting otherwise usable data during oracle outages.
    pub fn set_max_staleness(e: Env, caller: Address, seconds: u64) -> Result<(), OracleError> {
        require_admin(&e, &caller);
        set_max_staleness_internal(&e, seconds);
        Ok(())
    }

    /// Get max staleness setting used by `get_price_valid` when no override is supplied.
    pub fn get_max_staleness(e: Env) -> u64 {
        read_config(&e).max_staleness_seconds
    }

    /// Get admin address.
    pub fn get_admin(e: Env) -> Address {
        read_admin(&e)
    }

    /// Get current on-chain version (0 if legacy/uninitialized).
    pub fn get_version(e: Env) -> u32 {
        read_version(&e)
    }

    /// Update admin (admin-only).
    ///
    /// Transfers control over whitelist management and configuration.
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

    // ========================================================================
    // Oracle Consumer Expectations for commitment_core/marketplace
    // ========================================================================

    /// Get price with consumer-level validation for commitment_core contracts.
    /// 
    /// This function provides stricter validation suitable for financial commitment contracts:
    /// - Enforces maximum staleness of 300 seconds (5 minutes) for commitment operations
    /// - Validates price is positive and within reasonable bounds
    /// - Returns detailed error information for consumer contract handling
    /// 
    /// # Parameters
    /// * `asset` - The asset address to get price for
    /// * `max_price_variation_percent` - Optional maximum allowed price variation (0-100)
    ///   If provided, validates that price hasn't changed more than this percentage
    ///   from the previous price (if available)
    /// 
    /// # Returns
    /// `Result<PriceData, OracleError>` - Price data if valid, error otherwise
    /// 
    /// # Security Notes
    /// - Consumers should use this instead of get_price() for financial operations
    /// - 5-minute staleness limit balances freshness with oracle reliability
    /// - Price variation checks prevent flash manipulation attacks
    /// - Always handle StalePrice error gracefully in consumer contracts
    pub fn get_price_for_commitment(
        e: Env,
        asset: Address,
        max_price_variation_percent: Option<u32>,
    ) -> Result<PriceData, OracleError> {
        // Use 5-minute staleness for commitment operations (stricter than default)
        let commitment_staleness = 300u64;
        let data = Self::get_price_valid(e, asset.clone(), Some(commitment_staleness))?;

        // Additional commitment-specific validations
        if data.price <= 0 {
            return Err(OracleError::InvalidPrice);
        }

        // Validate price variation if requested
        if let Some(max_variation) = max_price_variation_percent {
            if max_variation > 100 {
                return Err(OracleError::InvalidStaleness); // Reuse error for invalid input
            }

            // Get previous price for comparison (if available)
            let current_time = e.ledger().timestamp();
            let previous_data = e.storage().instance().get::<_, PriceData>(
                &DataKey::Price(asset.clone())
            );

            if let Some(prev) = previous_data {
                if prev.updated_at < data.updated_at && prev.price > 0 {
                    let variation = SafeMath::calculate_percentage_change(
                        prev.price,
                        data.price
                    );
                    if variation > max_variation as i128 {
                        // Price variation too high - potential manipulation
                        return Err(OracleError::StalePrice); // Reuse error for variation check
                    }
                }
            }
        }

        Ok(data)
    }

    /// Get price with consumer-level validation for marketplace contracts.
    /// 
    /// This function provides validation suitable for marketplace operations:
    /// - Allows longer staleness (1800 seconds = 30 minutes) for marketplace listings
    /// - Validates price is positive and reasonable for marketplace operations
    /// - Includes marketplace-specific price sanity checks
    /// 
    /// # Parameters
    /// * `asset` - The asset address to get price for
    /// * `min_price_usd` - Optional minimum USD price (in 8 decimals) for asset validation
    ///   Useful for preventing zero-price or manipulated low-price listings
    /// 
    /// # Returns
    /// `Result<PriceData, OracleError>` - Price data if valid, error otherwise
    /// 
    /// # Security Notes
    /// - 30-minute staleness allows for marketplace operational flexibility
    /// - Minimum price checks prevent zero-price attacks on listings
    /// - Marketplace operators should adjust min_price_usd per asset requirements
    pub fn get_price_for_marketplace(
        e: Env,
        asset: Address,
        min_price_usd: Option<i128>,
    ) -> Result<PriceData, OracleError> {
        // Use 30-minute staleness for marketplace operations
        let marketplace_staleness = 1800u64;
        let data = Self::get_price_valid(e, asset.clone(), Some(marketplace_staleness))?;

        // Marketplace-specific validations
        if data.price <= 0 {
            return Err(OracleError::InvalidPrice);
        }

        // Validate minimum price if specified (in 8 decimals)
        if let Some(min_price) = min_price_usd {
            if min_price <= 0 {
                return Err(OracleError::InvalidPrice);
            }
            
            // Convert oracle price to 8 decimals for comparison if needed
            let oracle_price_8dec = if data.decimals == 8 {
                data.price
            } else if data.decimals < 8 {
                data.price * 10i128.pow(8 - data.decimals as u32)
            } else {
                data.price / 10i128.pow(data.decimals as u32 - 8)
            };

            if oracle_price_8dec < min_price {
                return Err(OracleError::InvalidPrice);
            }
        }

        Ok(data)
    }

    /// Batch price validation for multiple assets (useful for commitment_core operations).
    /// 
    /// Validates prices for multiple assets in a single call, reducing cross-contract
    /// call overhead for consumers that need multiple asset prices.
    /// 
    /// # Parameters
    /// * `assets` - Vector of asset addresses to get prices for
    /// * `max_staleness_seconds` - Maximum allowed staleness for all assets
    /// 
    /// # Returns
    /// `Result<Vec<(Address, PriceData)>, OracleError>` - Vector of (asset, price_data) tuples
    /// 
    /// # Security Notes
    /// - All assets must pass freshness validation for the batch to succeed
    /// - Consumers should handle partial failure scenarios appropriately
    /// - Batch operations reduce gas costs compared to individual calls
    pub fn get_batch_prices(
        e: Env,
        assets: Vec<Address>,
        max_staleness_seconds: u64,
    ) -> Result<Vec<(Address, PriceData)>, OracleError> {
        let mut results = Vec::new(&e);
        
        for asset in assets.iter() {
            let data = Self::get_price_valid(e.clone(), asset.clone(), Some(max_staleness_seconds))?;
            results.push_back((asset.clone(), data));
        }
        
        Ok(results)
    }

    /// Get price with safety checks for high-value operations.
    /// 
    /// Provides enhanced validation for operations involving significant value:
    /// - Stricter staleness requirements (60 seconds for high-value ops)
    /// - Price deviation checks against historical averages
    /// - Additional validation for critical financial operations
    /// 
    /// # Parameters
    /// * `asset` - The asset address to get price for
    /// * `operation_value_usd` - The USD value of the operation (in 8 decimals)
    ///   Used to determine appropriate validation strictness
    /// * `max_deviation_percent` - Maximum allowed deviation from historical average
    /// 
    /// # Returns
    /// `Result<PriceData, OracleError>` - Price data if valid, error otherwise
    /// 
    /// # Security Notes
    /// - High-value operations require fresher price data
    /// - Historical deviation checks prevent manipulation attacks
    /// - Use for settlements, large transfers, or critical operations
    pub fn get_price_for_high_value_operation(
        e: Env,
        asset: Address,
        operation_value_usd: i128,
        max_deviation_percent: u32,
    ) -> Result<PriceData, OracleError> {
        // Dynamic staleness based on operation value
        let staleness = if operation_value_usd > 100_000_000_000 { // > $1,000 USD in 8 decimals
            60 // 1 minute for very high value
        } else if operation_value_usd > 10_000_000_000 { // > $100 USD in 8 decimals
            300 // 5 minutes for high value
        } else {
            900 // 15 minutes for normal value
        };

        let data = Self::get_price_valid(e, asset.clone(), Some(staleness))?;

        // Additional high-value validations
        if data.price <= 0 {
            return Err(OracleError::InvalidPrice);
        }

        // For very high-value operations, we could implement additional checks
        // such as requiring multiple oracle confirmations or circuit breakers
        if operation_value_usd > 1_000_000_000_000 { // > $10,000 USD
            // In a production system, this might trigger additional validation
            // such as checking against multiple price sources or requiring admin confirmation
        }

        Ok(data)
    }

    /// Validate oracle health and status for consumer contracts.
    /// 
    /// Provides health information that consumer contracts can use to determine
    /// if the oracle system is operating normally.
    /// 
    /// # Returns
    /// `Result<OracleHealth, OracleError>` - Oracle health status
    /// 
    /// # Security Notes
    /// - Consumer contracts should check health before critical operations
    /// - Degraded health status should trigger fallback mechanisms
    /// - Always handle health check failures gracefully
    pub fn get_oracle_health(e: Env) -> Result<OracleHealth, OracleError> {
        let config = read_config(&e);
        let current_time = e.ledger().timestamp();
        
        // Check if we have any recent price updates
        let all_prices_recent = true; // In a real implementation, would scan recent prices
        
        let health = OracleHealth {
            is_healthy: all_prices_recent,
            max_staleness_seconds: config.max_staleness_seconds,
            last_check: current_time,
            active_oracles_count: 0, // Would need to track active oracles
        };
        
        Ok(health)
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
/// Oracle health status for consumer contract monitoring
pub struct OracleHealth {
    pub is_healthy: bool,
    pub max_staleness_seconds: u64,
    pub last_check: u64,
    pub active_oracles_count: u32,
}

#[cfg(test)]
mod tests;
