#![no_std]

//! Mock Oracle Contract for Integration Testing
//!
//! This contract simulates an external oracle service for testing purposes.
//! It provides deterministic price feeds and allows test control over:
//! - Price values per asset
//! - Staleness simulation
//! - Error conditions

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Env, Map, Symbol, symbol_short,
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
    /// # Arguments
    /// * `admin` - The admin address for the contract
    /// * `staleness_threshold` - Maximum age of price data in seconds before considered stale
    pub fn initialize(
        e: Env,
        admin: Address,
        staleness_threshold: u64,
    ) -> Result<(), OracleError> {
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
    /// # Arguments
    /// * `caller` - Must be admin or authorized feeder
    /// * `asset` - The asset address to set price for
    /// * `price` - The price value
    /// * `decimals` - Number of decimal places
    /// * `confidence` - Confidence interval for the price
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
    /// # Arguments
    /// * `caller` - Must be admin or authorized feeder
    /// * `asset` - The asset address
    /// * `price` - The price value
    /// * `timestamp` - Custom timestamp for the price
    /// * `decimals` - Number of decimal places
    /// * `confidence` - Confidence interval
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
    /// # Arguments
    /// * `asset` - The asset address to get price for
    ///
    /// # Returns
    /// * The current price or an error
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
    /// # Arguments
    /// * `asset` - The asset address
    ///
    /// # Returns
    /// * Full PriceData struct or error
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
    /// # Arguments
    /// * `asset` - The asset address
    /// * `max_staleness` - Maximum acceptable age in seconds
    ///
    /// # Returns
    /// * Price if fresh enough, error otherwise
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
    pub fn remove_price(e: Env, caller: Address, asset: Address) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage().instance().remove(&DataKey::Price(asset.clone()));

        e.events().publish(
            (Symbol::new(&e, "PriceRemoved"),),
            asset,
        );

        Ok(())
    }

    /// Pause the oracle (for testing unavailability)
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
    pub fn add_feeder(e: Env, caller: Address, feeder: Address) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .set(&DataKey::Feeder(feeder.clone()), &true);

        e.events().publish(
            (Symbol::new(&e, "FeederAdded"),),
            feeder,
        );

        Ok(())
    }

    /// Remove an authorized price feeder
    pub fn remove_feeder(e: Env, caller: Address, feeder: Address) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .remove(&DataKey::Feeder(feeder.clone()));

        e.events().publish(
            (Symbol::new(&e, "FeederRemoved"),),
            feeder,
        );

        Ok(())
    }

    /// Update staleness threshold
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

        e.events().publish(
            (Symbol::new(&e, "ThresholdUpdated"),),
            threshold,
        );

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

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

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
        e.mock_all_auths();
        let contract_id = e.register_contract(None, MockOracleContract);
        let admin = Address::generate(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::initialize(e.clone(), admin.clone(), 3600).unwrap();
            MockOracleContract::set_price(e.clone(), admin.clone(), asset.clone(), 100_000_000, 8, 1000)
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
}
