#![no_std]

//! Mock Oracle Contract for Integration Testing
//!
//! This contract simulates an external oracle service for testing purposes.
//! It provides deterministic price feeds and allows test control over:
//! - Price values per asset
//! - Staleness simulation
//! - Error conditions
//!
//! ## Local Soroban test usage
//!
//! Typical unit/integration setup:
//! 1. Register the contract in `Env`.
//! 2. Initialize once with admin + staleness threshold.
//! 3. Seed prices with `set_price` or `set_price_with_timestamp`.
//! 4. Read with `get_price` / `get_price_data` and assert outcomes.
//!
//! For deterministic local tests with multiple authorized actors, use
//! `env.mock_all_auths_allowing_non_root_auth()` and explicit ledger timestamps
//! when freshness behavior is under test.
//!
//! ## CI usage pattern
//!
//! CI test runs should avoid wall-clock assumptions:
//! - Pin the ledger timestamp before writes.
//! - Use `set_price_with_timestamp` to model stale/fresh boundaries precisely.
//! - Use `pause`/`unpause` and `remove_price` to exercise failure branches in a
//!   deterministic way.
//!
//! See also: `docs/MOCK_ORACLE_TESTING.md`.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

/// Oracle-specific errors
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OracleError {
    /// Contract not initialized
    NotInitialized = 1,
    /// Contract already initialized
    AlreadyInitialized = 2,
    /// Caller is not authorized
    Unauthorized = 3,
    /// Price not found for asset
    PriceNotFound = 4,
    /// Price is stale (older than threshold)
    StalePrice = 5,
    /// Invalid price value
    InvalidPrice = 6,
    /// Asset not configured
    AssetNotConfigured = 7,
}

/// Price data structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceData {
    /// Price in base units (e.g., cents for USD)
    pub price: i128,
    /// Timestamp when price was last updated
    pub timestamp: u64,
    /// Number of decimal places for the price
    pub decimals: u32,
    /// Confidence interval (optional, for testing volatility)
    pub confidence: i128,
}

/// Storage keys for the oracle contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Admin address
    Admin,
    /// Price data for an asset (Address -> PriceData)
    Price(Address),
    /// Staleness threshold in seconds
    StalenessThreshold,
    /// Whether the oracle is paused (for testing error scenarios)
    Paused,
    /// Authorized price feeders
    Feeder(Address),
}

#[contract]
pub struct MockOracleContract;

#[contractimpl]
impl MockOracleContract {
    /// Initialize the mock oracle contract
    ///
    /// Summary:
    /// Sets admin authority, default staleness threshold, and paused state.
    ///
    /// Params:
    /// - `admin`: contract admin and initial feeder.
    /// - `staleness_threshold`: maximum allowed age (seconds) for `get_price`.
    ///
    /// Errors:
    /// - [`OracleError::AlreadyInitialized`] if called more than once.
    ///
    /// Security:
    /// - No `require_auth` because this is single-use bootstrap. Callers should only
    ///   invoke during test setup, immediately after deployment.
    pub fn initialize(e: Env, admin: Address, staleness_threshold: u64) -> Result<(), OracleError> {
        if e.storage().instance().has(&DataKey::Admin) {
            return Err(OracleError::AlreadyInitialized);
        }

        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage()
            .instance()
            .set(&DataKey::StalenessThreshold, &staleness_threshold);
        e.storage().instance().set(&DataKey::Paused, &false);

        // Admin is automatically a feeder
        e.storage()
            .instance()
            .set(&DataKey::Feeder(admin.clone()), &true);

        e.events().publish(
            (Symbol::new(&e, "OracleInitialized"),),
            (admin, staleness_threshold),
        );

        Ok(())
    }

    /// Set a price for an asset (admin/feeder only)
    ///
    /// Summary:
    /// Writes a price snapshot using the current ledger timestamp.
    ///
    /// Params:
    /// - `caller`: must auth and be admin or feeder.
    /// - `asset`: asset identifier.
    /// - `price`: non-negative price value.
    /// - `decimals`: decimal precision metadata.
    /// - `confidence`: confidence interval metadata.
    ///
    /// Errors:
    /// - [`OracleError::NotInitialized`] when admin state is missing.
    /// - [`OracleError::Unauthorized`] when caller is not admin/feeder.
    /// - [`OracleError::InvalidPrice`] when `price < 0`.
    ///
    /// Security:
    /// - Enforces `caller.require_auth()`.
    /// - Trust boundary is explicit: any authorized feeder can overwrite the latest
    ///   value for an asset.
    pub fn set_price(
        e: Env,
        caller: Address,
        asset: Address,
        price: i128,
        decimals: u32,
        confidence: i128,
    ) -> Result<(), OracleError> {
        caller.require_auth();

        // Check if caller is authorized
        if !Self::is_authorized(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        // Validate price
        if price < 0 {
            return Err(OracleError::InvalidPrice);
        }

        let price_data = PriceData {
            price,
            timestamp: e.ledger().timestamp(),
            decimals,
            confidence,
        };

        e.storage()
            .instance()
            .set(&DataKey::Price(asset.clone()), &price_data);

        e.events().publish(
            (Symbol::new(&e, "PriceUpdated"), asset.clone()),
            (price, e.ledger().timestamp()),
        );

        Ok(())
    }

    /// Set a price with a specific timestamp (for testing staleness)
    ///
    /// Summary:
    /// Writes a price snapshot using a caller-specified timestamp.
    ///
    /// Params:
    /// - `caller`: must auth and be admin or feeder.
    /// - `asset`: asset identifier.
    /// - `price`: non-negative price value.
    /// - `timestamp`: explicit timestamp used for deterministic stale/fresh tests.
    /// - `decimals`: decimal precision metadata.
    /// - `confidence`: confidence interval metadata.
    ///
    /// Errors:
    /// - [`OracleError::NotInitialized`] when admin state is missing.
    /// - [`OracleError::Unauthorized`] when caller is not admin/feeder.
    /// - [`OracleError::InvalidPrice`] when `price < 0`.
    ///
    /// Security:
    /// - Enforces `caller.require_auth()`.
    /// - This function is intended for test control; production-like consumers should
    ///   treat timestamp authority as trusted publisher input.
    pub fn set_price_with_timestamp(
        e: Env,
        caller: Address,
        asset: Address,
        price: i128,
        timestamp: u64,
        decimals: u32,
        confidence: i128,
    ) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_authorized(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        if price < 0 {
            return Err(OracleError::InvalidPrice);
        }

        let price_data = PriceData {
            price,
            timestamp,
            decimals,
            confidence,
        };

        e.storage()
            .instance()
            .set(&DataKey::Price(asset.clone()), &price_data);

        e.events().publish(
            (Symbol::new(&e, "PriceUpdated"), asset.clone()),
            (price, timestamp),
        );

        Ok(())
    }

    /// Get the current price for an asset
    ///
    /// Summary:
    /// Reads current price and enforces contract staleness threshold.
    ///
    /// Params:
    /// - `asset`: asset identifier.
    ///
    /// Returns:
    /// - `Ok(price)` when present and fresh.
    ///
    /// Errors:
    /// - [`OracleError::NotInitialized`] when paused (simulated unavailability).
    /// - [`OracleError::PriceNotFound`] when missing.
    /// - [`OracleError::StalePrice`] when older than configured threshold.
    ///
    /// Security:
    /// - Read-only path with no auth checks; freshness must be validated by callers
    ///   through this method or `get_price_no_older_than`.
    pub fn get_price(e: Env, asset: Address) -> Result<i128, OracleError> {
        // Check if oracle is paused
        let paused: bool = e
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(OracleError::NotInitialized); // Simulate unavailability
        }

        let price_data: PriceData = e
            .storage()
            .instance()
            .get(&DataKey::Price(asset))
            .ok_or(OracleError::PriceNotFound)?;

        // Check staleness
        let staleness_threshold: u64 = e
            .storage()
            .instance()
            .get(&DataKey::StalenessThreshold)
            .unwrap_or(3600); // Default 1 hour

        let current_time = e.ledger().timestamp();
        if current_time > price_data.timestamp
            && current_time - price_data.timestamp > staleness_threshold
        {
            return Err(OracleError::StalePrice);
        }

        Ok(price_data.price)
    }

    /// Get full price data for an asset
    ///
    /// Summary:
    /// Reads raw `PriceData` for an asset.
    ///
    /// Params:
    /// - `asset`: asset identifier.
    ///
    /// Returns:
    /// - `Ok(PriceData)` when present and unpaused.
    ///
    /// Errors:
    /// - [`OracleError::NotInitialized`] when paused.
    /// - [`OracleError::PriceNotFound`] when missing.
    pub fn get_price_data(e: Env, asset: Address) -> Result<PriceData, OracleError> {
        let paused: bool = e
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(OracleError::NotInitialized);
        }

        e.storage()
            .instance()
            .get(&DataKey::Price(asset))
            .ok_or(OracleError::PriceNotFound)
    }

    /// Get price with staleness check
    ///
    /// Summary:
    /// Reads price with caller-defined freshness window.
    ///
    /// Params:
    /// - `asset`: asset identifier.
    /// - `max_staleness`: maximum accepted age in seconds.
    ///
    /// Returns:
    /// - `Ok(price)` when present and fresh enough.
    ///
    /// Errors:
    /// - [`OracleError::PriceNotFound`] when missing.
    /// - [`OracleError::StalePrice`] when older than `max_staleness`.
    pub fn get_price_no_older_than(
        e: Env,
        asset: Address,
        max_staleness: u64,
    ) -> Result<i128, OracleError> {
        let price_data: PriceData = e
            .storage()
            .instance()
            .get(&DataKey::Price(asset))
            .ok_or(OracleError::PriceNotFound)?;

        let current_time = e.ledger().timestamp();
        if current_time > price_data.timestamp
            && current_time - price_data.timestamp > max_staleness
        {
            return Err(OracleError::StalePrice);
        }

        Ok(price_data.price)
    }

    /// Check if a price exists for an asset
    pub fn has_price(e: Env, asset: Address) -> bool {
        e.storage().instance().has(&DataKey::Price(asset))
    }

    /// Remove a price (for testing missing price scenarios)
    ///
    /// Security:
    /// - Admin-only (`require_auth` + admin check).
    pub fn remove_price(e: Env, caller: Address, asset: Address) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .remove(&DataKey::Price(asset.clone()));

        e.events()
            .publish((Symbol::new(&e, "PriceRemoved"),), asset);

        Ok(())
    }

    /// Pause the oracle (for testing unavailability)
    ///
    /// Security:
    /// - Admin-only (`require_auth` + admin check).
    /// - Paused state forces reads to return `NotInitialized` to simulate outage.
    pub fn pause(e: Env, caller: Address) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage().instance().set(&DataKey::Paused, &true);

        e.events().publish((symbol_short!("Paused"),), ());

        Ok(())
    }

    /// Unpause the oracle
    ///
    /// Security:
    /// - Admin-only (`require_auth` + admin check).
    pub fn unpause(e: Env, caller: Address) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage().instance().set(&DataKey::Paused, &false);

        e.events().publish((symbol_short!("Unpaused"),), ());

        Ok(())
    }

    /// Add an authorized price feeder
    ///
    /// Security:
    /// - Admin-only (`require_auth` + admin check).
    pub fn add_feeder(e: Env, caller: Address, feeder: Address) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .set(&DataKey::Feeder(feeder.clone()), &true);

        e.events()
            .publish((Symbol::new(&e, "FeederAdded"),), feeder);

        Ok(())
    }

    /// Remove an authorized price feeder
    ///
    /// Security:
    /// - Admin-only (`require_auth` + admin check).
    pub fn remove_feeder(e: Env, caller: Address, feeder: Address) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .remove(&DataKey::Feeder(feeder.clone()));

        e.events()
            .publish((Symbol::new(&e, "FeederRemoved"),), feeder);

        Ok(())
    }

    /// Update staleness threshold
    ///
    /// Security:
    /// - Admin-only (`require_auth` + admin check).
    pub fn set_staleness_threshold(
        e: Env,
        caller: Address,
        threshold: u64,
    ) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .set(&DataKey::StalenessThreshold, &threshold);

        e.events()
            .publish((Symbol::new(&e, "ThresholdUpdated"),), threshold);

        Ok(())
    }

    /// Get the admin address
    pub fn get_admin(e: Env) -> Result<Address, OracleError> {
        e.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(OracleError::NotInitialized)
    }

    /// Check if address is a feeder
    pub fn is_feeder(e: Env, address: Address) -> bool {
        e.storage()
            .instance()
            .get(&DataKey::Feeder(address))
            .unwrap_or(false)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn is_admin(e: &Env, address: &Address) -> Result<bool, OracleError> {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(OracleError::NotInitialized)?;
        Ok(*address == admin)
    }

    fn is_authorized(e: &Env, address: &Address) -> Result<bool, OracleError> {
        // Admin is always authorized
        if Self::is_admin(e, address)? {
            return Ok(true);
        }

        // Check if address is an authorized feeder
        Ok(e.storage()
            .instance()
            .get(&DataKey::Feeder(address.clone()))
            .unwrap_or(false))
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    const CI_FIXED_TIMESTAMP: u64 = 1_704_067_200; // Jan 1, 2024 UTC

    #[test]
    fn test_initialize() {
        let e = Env::default();
        let contract_id = e.register_contract(None, MockOracleContract);
        let admin = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::initialize(e.clone(), admin.clone(), 3600).unwrap();
            assert_eq!(MockOracleContract::get_admin(e.clone()).unwrap(), admin);
        });
    }

    #[test]
    fn test_set_and_get_price() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        let contract_id = e.register_contract(None, MockOracleContract);
        let admin = Address::generate(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::initialize(e.clone(), admin.clone(), 3600).unwrap();
            MockOracleContract::set_price(
                e.clone(),
                admin.clone(),
                asset.clone(),
                100_000_000,
                8,
                1000,
            )
            .unwrap();

            let price = MockOracleContract::get_price(e.clone(), asset.clone()).unwrap();
            assert_eq!(price, 100_000_000);
        });
    }

    #[test]
    fn test_price_not_found() {
        let e = Env::default();
        let contract_id = e.register_contract(None, MockOracleContract);
        let admin = Address::generate(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::initialize(e.clone(), admin.clone(), 3600).unwrap();
            let result = MockOracleContract::get_price(e.clone(), asset.clone());
            assert_eq!(result, Err(OracleError::PriceNotFound));
        });
    }

    #[test]
    fn test_local_usage_admin_and_feeder_flow() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        let contract_id = e.register_contract(None, MockOracleContract);
        let admin = Address::generate(&e);
        let feeder = Address::generate(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::initialize(e.clone(), admin.clone(), 3600).unwrap();
        });

        e.as_contract(&contract_id, || {
            // Local test pattern: admin is feeder by default, then explicit feeder onboarding.
            assert!(MockOracleContract::is_feeder(e.clone(), admin.clone()));
            assert!(!MockOracleContract::is_feeder(e.clone(), feeder.clone()));
        });

        e.as_contract(&contract_id, || {
            MockOracleContract::add_feeder(e.clone(), admin.clone(), feeder.clone()).unwrap();
        });

        e.as_contract(&contract_id, || {
            assert!(MockOracleContract::is_feeder(e.clone(), feeder.clone()));
        });

        e.as_contract(&contract_id, || {
            // Feeder writes deterministic price data for test assertions.
            MockOracleContract::set_price(
                e.clone(),
                feeder.clone(),
                asset.clone(),
                150_000_000,
                8,
                500,
            )
            .unwrap();
        });

        e.as_contract(&contract_id, || {
            assert!(MockOracleContract::has_price(e.clone(), asset.clone()));
            assert_eq!(
                MockOracleContract::get_price(e.clone(), asset.clone()).unwrap(),
                150_000_000
            );

            let data = MockOracleContract::get_price_data(e.clone(), asset.clone()).unwrap();
            assert_eq!(data.price, 150_000_000);
            assert_eq!(data.decimals, 8);
            assert_eq!(data.confidence, 500);
        });

        e.as_contract(&contract_id, || {
            // Removing feeder should immediately revoke write privileges.
            MockOracleContract::remove_feeder(e.clone(), admin.clone(), feeder.clone()).unwrap();
        });

        e.as_contract(&contract_id, || {
            assert!(!MockOracleContract::is_feeder(e.clone(), feeder.clone()));
            assert_eq!(
                MockOracleContract::set_price(
                    e.clone(),
                    feeder.clone(),
                    asset,
                    151_000_000,
                    8,
                    500,
                ),
                Err(OracleError::Unauthorized)
            );
        });
    }

    #[test]
    fn test_ci_usage_deterministic_staleness_boundaries() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        let contract_id = e.register_contract(None, MockOracleContract);
        let admin = Address::generate(&e);
        let asset = Address::generate(&e);

        e.ledger().with_mut(|ledger| {
            ledger.timestamp = CI_FIXED_TIMESTAMP;
        });

        e.as_contract(&contract_id, || {
            MockOracleContract::initialize(e.clone(), admin.clone(), 60).unwrap();
        });

        e.as_contract(&contract_id, || {
            // CI pattern: force stale data using explicit old timestamp.
            MockOracleContract::set_price_with_timestamp(
                e.clone(),
                admin.clone(),
                asset.clone(),
                90_000_000,
                CI_FIXED_TIMESTAMP - 61,
                8,
                1000,
            )
            .unwrap();
        });

        e.as_contract(&contract_id, || {
            assert_eq!(
                MockOracleContract::get_price(e.clone(), asset.clone()),
                Err(OracleError::StalePrice)
            );
        });

        e.as_contract(&contract_id, || {
            // Exactly-at-threshold boundary should be accepted (strictly greater is stale).
            MockOracleContract::set_price_with_timestamp(
                e.clone(),
                admin.clone(),
                asset.clone(),
                91_000_000,
                CI_FIXED_TIMESTAMP - 60,
                8,
                1000,
            )
            .unwrap();
        });

        e.as_contract(&contract_id, || {
            assert_eq!(
                MockOracleContract::get_price(e.clone(), asset.clone()).unwrap(),
                91_000_000
            );
            assert_eq!(
                MockOracleContract::get_price_no_older_than(e.clone(), asset.clone(), 59),
                Err(OracleError::StalePrice)
            );
            assert_eq!(
                MockOracleContract::get_price_no_older_than(e.clone(), asset, 60).unwrap(),
                91_000_000
            );
        });
    }

    #[test]
    fn test_ci_usage_error_simulation_pause_and_missing_price() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        let contract_id = e.register_contract(None, MockOracleContract);
        let admin = Address::generate(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::initialize(e.clone(), admin.clone(), 3600).unwrap();
        });

        e.as_contract(&contract_id, || {
            MockOracleContract::set_price(
                e.clone(),
                admin.clone(),
                asset.clone(),
                100_000_000,
                8,
                1000,
            )
            .unwrap();
        });

        e.as_contract(&contract_id, || {
            // Simulate upstream outage in CI suites.
            MockOracleContract::pause(e.clone(), admin.clone()).unwrap();
        });

        e.as_contract(&contract_id, || {
            assert_eq!(
                MockOracleContract::get_price(e.clone(), asset.clone()),
                Err(OracleError::NotInitialized)
            );
            assert_eq!(
                MockOracleContract::get_price_data(e.clone(), asset.clone()),
                Err(OracleError::NotInitialized)
            );
        });

        e.as_contract(&contract_id, || {
            MockOracleContract::unpause(e.clone(), admin.clone()).unwrap();
        });

        e.as_contract(&contract_id, || {
            assert_eq!(
                MockOracleContract::get_price(e.clone(), asset.clone()).unwrap(),
                100_000_000
            );
        });

        e.as_contract(&contract_id, || {
            // Simulate missing feed after removal.
            MockOracleContract::remove_price(e.clone(), admin.clone(), asset.clone()).unwrap();
        });

        e.as_contract(&contract_id, || {
            assert!(!MockOracleContract::has_price(e.clone(), asset.clone()));
            assert_eq!(
                MockOracleContract::get_price(e.clone(), asset),
                Err(OracleError::PriceNotFound)
            );
        });
    }

    #[test]
    fn test_unauthorized_admin_paths_rejected() {
        let e = Env::default();
        e.mock_all_auths_allowing_non_root_auth();
        let contract_id = e.register_contract(None, MockOracleContract);
        let admin = Address::generate(&e);
        let attacker = Address::generate(&e);
        let feeder = Address::generate(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::initialize(e.clone(), admin.clone(), 3600).unwrap();
        });

        e.as_contract(&contract_id, || {
            assert_eq!(
                MockOracleContract::add_feeder(e.clone(), attacker.clone(), feeder.clone()),
                Err(OracleError::Unauthorized)
            );
        });

        e.as_contract(&contract_id, || {
            assert_eq!(
                MockOracleContract::set_staleness_threshold(e.clone(), attacker.clone(), 30),
                Err(OracleError::Unauthorized)
            );
        });

        e.as_contract(&contract_id, || {
            assert_eq!(
                MockOracleContract::remove_price(e.clone(), attacker.clone(), asset.clone()),
                Err(OracleError::Unauthorized)
            );
        });

        e.as_contract(&contract_id, || {
            assert_eq!(
                MockOracleContract::pause(e.clone(), attacker.clone()),
                Err(OracleError::Unauthorized)
            );
        });

        e.as_contract(&contract_id, || {
            assert_eq!(
                MockOracleContract::unpause(e.clone(), attacker),
                Err(OracleError::Unauthorized)
            );
        });
    }
}
