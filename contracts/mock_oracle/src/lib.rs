#![no_std]

//! Mock Oracle Contract for Integration Testing
//!
//! This contract simulates an external oracle service for testing purposes.
//! It provides deterministic price feeds and allows test control over:
//! - Price values per asset
//! - Staleness simulation
//! - Error conditions

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

/// Storage keys for oracle contract
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Admin address
    Admin,
    /// Price data for an asset (Address -> PriceData)
    Price(Address),
    /// Staleness threshold in seconds
    StalenessThreshold,
    /// Whether oracle is paused (for testing error scenarios)
    Paused,
    /// Authorized price feeders
    Feeder(Address),
    /// Test configuration mode
    TestMode,
    /// Price override for specific asset (Address -> i128)
    PriceOverride(Address),
    /// Failure injection configuration (Symbol -> bool)
    FailureMode(Symbol),
    /// Per-asset failure injection configuration (Asset, Symbol) -> bool
    FailureModeForAsset(Address, Symbol),
    /// Simulated delay for price queries (u64 seconds)
    QueryDelay,
    /// Price volatility simulation (i128)
    VolatilityFactor,
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

        // Initialize test configuration
        e.storage().instance().set(&DataKey::TestMode, &false);
        e.storage().instance().set(&DataKey::QueryDelay, &0u64);
        e.storage().instance().set(&DataKey::VolatilityFactor, &0i128);

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

    /// Get the current price for an asset with configurable features
    ///
    /// # Arguments
    /// * `asset` - The asset address to get price for
    ///
    /// # Returns
    /// * The current price or an error
    pub fn get_price(e: Env, asset: Address) -> Result<i128, OracleError> {
        // Simulate query delay if configured
        let delay: u64 = e
            .storage()
            .instance()
            .get(&DataKey::QueryDelay)
            .unwrap_or(0);
        
        if delay > 0 {
            // In a real implementation, this would add delay
            // For testing, we just check that delay is configured
        }

        // Check for failure injection modes
        if Self::should_inject_failure(&e, &asset, "price_not_found")? {
            return Err(OracleError::PriceNotFound);
        }

        if Self::should_inject_failure(&e, &asset, "stale_price")? {
            return Err(OracleError::StalePrice);
        }

        if Self::should_inject_failure(&e, &asset, "oracle_paused")? {
            return Err(OracleError::NotInitialized);
        }

        if Self::should_inject_failure(&e, &asset, "invalid_price")? {
            return Err(OracleError::InvalidPrice);
        }

        // Check if oracle is paused
        let paused: bool = e
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(OracleError::NotInitialized); // Simulate unavailability
        }

        // Check for price override first (test mode)
        let test_mode: bool = e
            .storage()
            .instance()
            .get(&DataKey::TestMode)
            .unwrap_or(false);
        
        if test_mode {
            if let Some(override_price) = e.storage().instance().get(&DataKey::PriceOverride(asset.clone())) {
                return Ok(Self::apply_volatility(&e, override_price));
            }
        }

        // Get regular price data
        let mut price_data: PriceData = e
            .storage()
            .instance()
            .get(&DataKey::Price(asset))
            .ok_or(OracleError::PriceNotFound)?;

        // Apply volatility if configured
        price_data.price = Self::apply_volatility(&e, price_data.price);

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
        // Failure injection first so tests can deterministically fail reads.
        if Self::should_inject_failure(&e, &asset, "price_not_found")? {
            return Err(OracleError::PriceNotFound);
        }
        if Self::should_inject_failure(&e, &asset, "stale_price")? {
            return Err(OracleError::StalePrice);
        }
        if Self::should_inject_failure(&e, &asset, "oracle_paused")? {
            return Err(OracleError::NotInitialized);
        }
        if Self::should_inject_failure(&e, &asset, "invalid_price")? {
            return Err(OracleError::InvalidPrice);
        }

        let paused: bool = e
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(OracleError::NotInitialized);
        }

        // Check for test-mode override first.
        let test_mode: bool = e
            .storage()
            .instance()
            .get(&DataKey::TestMode)
            .unwrap_or(false);

        if test_mode {
            if let Some(override_price) = e.storage().instance().get(&DataKey::PriceOverride(asset.clone())) {
                // Preserve metadata when regular price exists; otherwise synthesize minimal metadata.
                let mut price_data: PriceData = e
                    .storage()
                    .instance()
                    .get(&DataKey::Price(asset.clone()))
                    .unwrap_or(PriceData {
                        price: 0,
                        timestamp: e.ledger().timestamp(),
                        decimals: 0,
                        confidence: 0,
                    });
                price_data.price = Self::apply_volatility(&e, override_price);
                return Ok(price_data);
            }
        }

        // Regular stored price.
        let mut price_data: PriceData = e
            .storage()
            .instance()
            .get(&DataKey::Price(asset))
            .ok_or(OracleError::PriceNotFound)?;
        price_data.price = Self::apply_volatility(&e, price_data.price);
        Ok(price_data)
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
        // Failure injection first so tests can deterministically fail reads.
        if Self::should_inject_failure(&e, &asset, "price_not_found")? {
            return Err(OracleError::PriceNotFound);
        }
        if Self::should_inject_failure(&e, &asset, "stale_price")? {
            return Err(OracleError::StalePrice);
        }
        if Self::should_inject_failure(&e, &asset, "oracle_paused")? {
            return Err(OracleError::NotInitialized);
        }
        if Self::should_inject_failure(&e, &asset, "invalid_price")? {
            return Err(OracleError::InvalidPrice);
        }

        let paused: bool = e
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(OracleError::NotInitialized);
        }

        // Override bypasses staleness check to keep tests deterministic.
        let test_mode: bool = e
            .storage()
            .instance()
            .get(&DataKey::TestMode)
            .unwrap_or(false);
        if test_mode {
            if let Some(override_price) = e.storage().instance().get(&DataKey::PriceOverride(asset.clone())) {
                return Ok(Self::apply_volatility(&e, override_price));
            }
        }

        let mut price_data: PriceData = e
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

        price_data.price = Self::apply_volatility(&e, price_data.price);
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

        e.storage()
            .instance()
            .remove(&DataKey::Price(asset.clone()));

        e.events()
            .publish((Symbol::new(&e, "PriceRemoved"),), asset);

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

        e.events()
            .publish((Symbol::new(&e, "FeederAdded"),), feeder);

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

        e.events()
            .publish((Symbol::new(&e, "FeederRemoved"),), feeder);

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

    // ========================================================================
    // CONFIGURABLE PRICE AND FAILURE INJECTION FUNCTIONS
    // ========================================================================

    /// Enable test mode for configurable prices and failure injection
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `enabled` - Whether to enable test mode
    pub fn set_test_mode(e: Env, caller: Address, enabled: bool) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage().instance().set(&DataKey::TestMode, &enabled);
        e.events()
            .publish((Symbol::new(&e, "TestModeChanged"),), enabled);
        Ok(())
    }

    /// Set a test price override for an asset
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `asset` - The asset address to override price for
    /// * `price` - The override price value
    pub fn set_test_price(
        e: Env,
        caller: Address,
        asset: Address,
        price: i128,
    ) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        if price < 0 {
            return Err(OracleError::InvalidPrice);
        }

        e.storage()
            .instance()
            .set(&DataKey::PriceOverride(asset.clone()), &price);
        e.events()
            .publish((Symbol::new(&e, "TestPriceSet"),), (asset, price));
        Ok(())
    }

    /// Configure failure injection mode
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `failure_type` - Type of failure to inject (global across all assets)
    /// * `enabled` - Whether to enable failure mode
    pub fn configure_failure_mode(
        e: Env,
        caller: Address,
        failure_type: Symbol,
        enabled: bool,
    ) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .set(&DataKey::FailureMode(failure_type.clone()), &enabled);
        e.events()
            .publish((Symbol::new(&e, "FailureModeConfigured"),), (failure_type, enabled));
        Ok(())
    }

    /// Configure failure injection mode for a specific asset
    ///
    /// When `set_test_mode(true)` is enabled, reads for the given `asset` will deterministically
    /// return the requested error.
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `asset` - Asset address to scope the failure injection to
    /// * `failure_type` - Type of failure to inject
    /// * `enabled` - Whether to enable failure mode for the given asset
    pub fn configure_failure_mode_for_asset(
        e: Env,
        caller: Address,
        asset: Address,
        failure_type: Symbol,
        enabled: bool,
    ) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .set(
                &DataKey::FailureModeForAsset(asset.clone(), failure_type.clone()),
                &enabled,
            );
        e.events().publish(
            (Symbol::new(&e, "FailureModeForAssetConfigured"),),
            (asset, failure_type, enabled),
        );
        Ok(())
    }

    /// Set artificial query delay for testing latency
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `delay_seconds` - Delay to add to price queries
    pub fn set_query_delay(
        e: Env,
        caller: Address,
        delay_seconds: u64,
    ) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .set(&DataKey::QueryDelay, &delay_seconds);
        e.events()
            .publish((Symbol::new(&e, "QueryDelaySet"),), (delay_seconds, ));
        Ok(())
    }

    /// Enable price volatility simulation
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    /// * `volatility_factor` - Factor for price variation (basis points)
    pub fn set_volatility_factor(
        e: Env,
        caller: Address,
        volatility_factor: i128,
    ) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage()
            .instance()
            .set(&DataKey::VolatilityFactor, &volatility_factor);
        e.events()
            .publish((Symbol::new(&e, "VolatilityFactorSet"),), (volatility_factor, ));
        Ok(())
    }

    /// Clear all test configurations
    ///
    /// # Arguments
    /// * `caller` - Must be admin
    pub fn clear_test_configurations(e: Env, caller: Address) -> Result<(), OracleError> {
        caller.require_auth();

        if !Self::is_admin(&e, &caller)? {
            return Err(OracleError::Unauthorized);
        }

        e.storage().instance().set(&DataKey::TestMode, &false);
        e.storage().instance().set(&DataKey::QueryDelay, &0u64);
        e.storage().instance().set(&DataKey::VolatilityFactor, &0i128);
        
        e.events()
            .publish((Symbol::new(&e, "TestConfigurationsCleared"),), ());
        Ok(())
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

    /// Check if failure injection should be applied
    fn should_inject_failure(
        e: &Env,
        asset: &Address,
        failure_type: &str,
    ) -> Result<bool, OracleError> {
        let test_mode: bool = e
            .storage()
            .instance()
            .get(&DataKey::TestMode)
            .unwrap_or(false);
        
        if !test_mode {
            return Ok(false);
        }

        let failure_symbol = Symbol::new(e, failure_type);

        // Per-asset failure injection has higher precedence than the global configuration.
        let per_asset_key = DataKey::FailureModeForAsset(asset.clone(), failure_symbol.clone());
        if e.storage().instance().has(&per_asset_key) {
            return Ok(e
                .storage()
                .instance()
                .get(&per_asset_key)
                .unwrap_or(false));
        }

        Ok(e.storage().instance().get(&DataKey::FailureMode(failure_symbol)).unwrap_or(false))
    }

    /// Apply volatility factor to price
    fn apply_volatility(e: &Env, base_price: i128) -> i128 {
        let volatility_factor: i128 = e
            .storage()
            .instance()
            .get(&DataKey::VolatilityFactor)
            .unwrap_or(0);
        
        if volatility_factor == 0 {
            return base_price;
        }

        // Apply volatility as percentage variation (basis points)
        // volatility_factor is in basis points (10000 = 100%)
        let variation = base_price
            .checked_mul(volatility_factor)
            .and_then(|x| x.checked_div(10000))
            .unwrap_or(0);
        
        base_price
            .checked_add(variation)
            .unwrap_or(base_price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn create_test_contract(e: &Env) -> (Address, Address) {
        let admin = Address::generate(e);
        let contract_id = e.register_contract(None, MockOracleContract);
        let _client = MockOracleContractClient::new(e, &contract_id);
        
        e.as_contract(&contract_id, || {
            MockOracleContract::initialize(e.clone(), admin.clone(), 3600).unwrap();
        });
        
        (admin, contract_id)
    }

    // ========================================================================
    // EXISTING TESTS (Preserved)
    // ========================================================================

    #[test]
    fn test_initialize() {
        let e = Env::default();
        let (admin, contract_id) = create_test_contract(&e);
        e.as_contract(&contract_id, || {
            assert_eq!(MockOracleContract::get_admin(e.clone()).unwrap(), admin);
        });
    }

    #[test]
    fn test_set_and_get_price() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

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

    // ========================================================================
    // CONFIGURABLE PRICE TESTS
    // ========================================================================

    #[test]
    fn test_set_test_mode() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });
    }

    #[test]
    fn test_set_test_mode_unauthorized() {
        let e = Env::default();
        e.mock_all_auths();
        let (_admin, contract_id) = create_test_contract(&e);
        let non_admin = Address::generate(&e);

        e.as_contract(&contract_id, || {
            let res = MockOracleContract::set_test_mode(e.clone(), non_admin.clone(), true);
            assert_eq!(res, Err(OracleError::Unauthorized));
        });
    }

    #[test]
    fn test_configurable_price_override() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });

        e.as_contract(&contract_id, || {
            // Set test price override (separate frame to avoid duplicate auth).
            MockOracleContract::set_test_price(e.clone(), admin.clone(), asset.clone(), 500_000_000)
                .unwrap();
        });

        let price = e.as_contract(&contract_id, || {
            MockOracleContract::get_price(e.clone(), asset.clone()).unwrap()
        });
        assert_eq!(price, 500_000_000);
    }

    #[test]
    fn test_configurable_price_override_applies_to_get_price_data_and_no_older_than() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });

        e.as_contract(&contract_id, || {
            // Set test price override (separate frame to avoid duplicate auth).
            MockOracleContract::set_test_price(e.clone(), admin.clone(), asset.clone(), 500_000_000)
                .unwrap();
        });

        let (price_data, price) = e.as_contract(&contract_id, || {
            let price_data = MockOracleContract::get_price_data(e.clone(), asset.clone()).unwrap();
            // Override bypasses staleness checks for deterministic tests.
            let price = MockOracleContract::get_price_no_older_than(e.clone(), asset.clone(), 1).unwrap();
            (price_data, price)
        });

        assert_eq!(price_data.price, 500_000_000);
        assert_eq!(price, 500_000_000);
    }

    #[test]
    fn test_configurable_price_override_preserves_metadata_in_get_price_data() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_price(
                e.clone(),
                admin.clone(),
                asset.clone(),
                100_000_000,
                8,
                1234,
            )
            .unwrap();
        });

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_price(e.clone(), admin.clone(), asset.clone(), 200_000_000)
                .unwrap();
        });

        let price_data = e.as_contract(&contract_id, || {
            MockOracleContract::get_price_data(e.clone(), asset.clone()).unwrap()
        });
        assert_eq!(price_data.price, 200_000_000);
        assert_eq!(price_data.decimals, 8);
        assert_eq!(price_data.confidence, 1234);
    }

    #[test]
    fn test_multiple_price_overrides() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset1 = Address::generate(&e);
        let asset2 = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_price(e.clone(), admin.clone(), asset1.clone(), 100_000_000).unwrap();
        });
        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_price(e.clone(), admin.clone(), asset2.clone(), 200_000_000).unwrap();
        });

        let (price1, price2) = e.as_contract(&contract_id, || {
            let price1 = MockOracleContract::get_price(e.clone(), asset1.clone()).unwrap();
            let price2 = MockOracleContract::get_price(e.clone(), asset2.clone()).unwrap();
            (price1, price2)
        });

        assert_eq!(price1, 100_000_000);
        assert_eq!(price2, 200_000_000);
    }

    #[test]
    fn test_set_test_price_invalid_price() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });

        let res = e.as_contract(&contract_id, || {
            // Negative price should be rejected
            MockOracleContract::set_test_price(e.clone(), admin.clone(), asset.clone(), -100)
        });
        assert_eq!(res, Err(OracleError::InvalidPrice));
    }

    // ========================================================================
    // FAILURE INJECTION TESTS
    // ========================================================================

    #[test]
    fn test_failure_injection_price_not_found() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });
        e.as_contract(&contract_id, || {
            // Configure price not found failure
            MockOracleContract::configure_failure_mode(
                e.clone(),
                admin.clone(),
                Symbol::new(&e, "price_not_found"),
                true,
            )
            .unwrap();
        });

        let result = e.as_contract(&contract_id, || MockOracleContract::get_price(e.clone(), asset.clone()));
        assert_eq!(result, Err(OracleError::PriceNotFound));
    }

    #[test]
    fn test_failure_injection_per_asset_affects_only_configured_asset() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset1 = Address::generate(&e);
        let asset2 = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });
        e.as_contract(&contract_id, || {
            MockOracleContract::set_price(
                e.clone(),
                admin.clone(),
                asset1.clone(),
                100_000_000,
                8,
                1000,
            )
            .unwrap();
        });
        e.as_contract(&contract_id, || {
            MockOracleContract::set_price(
                e.clone(),
                admin.clone(),
                asset2.clone(),
                200_000_000,
                8,
                1000,
            )
            .unwrap();
        });
        e.as_contract(&contract_id, || {
            // Inject stale_price only for asset1.
            MockOracleContract::configure_failure_mode_for_asset(
                e.clone(),
                admin.clone(),
                asset1.clone(),
                Symbol::new(&e, "stale_price"),
                true,
            )
            .unwrap();
        });

        let r1 = e.as_contract(&contract_id, || {
            MockOracleContract::get_price_data(e.clone(), asset1.clone())
        });
        assert_eq!(r1, Err(OracleError::StalePrice));

        let r2 = e.as_contract(&contract_id, || {
            MockOracleContract::get_price_data(e.clone(), asset2.clone()).unwrap()
        });
        assert_eq!(r2.price, 200_000_000);
    }

    #[test]
    fn test_failure_injection_global_still_affects_get_price_data() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset1 = Address::generate(&e);
        let asset2 = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });
        e.as_contract(&contract_id, || {
            MockOracleContract::set_price(
                e.clone(),
                admin.clone(),
                asset1.clone(),
                100_000_000,
                8,
                1000,
            )
            .unwrap();
        });
        e.as_contract(&contract_id, || {
            MockOracleContract::set_price(
                e.clone(),
                admin.clone(),
                asset2.clone(),
                200_000_000,
                8,
                1000,
            )
            .unwrap();
        });
        e.as_contract(&contract_id, || {
            // Global failure injection should apply to all assets.
            MockOracleContract::configure_failure_mode(
                e.clone(),
                admin.clone(),
                Symbol::new(&e, "stale_price"),
                true,
            )
            .unwrap();
        });

        let p1 = e.as_contract(&contract_id, || MockOracleContract::get_price_data(e.clone(), asset1.clone()));
        assert_eq!(p1, Err(OracleError::StalePrice));
        let p2 = e.as_contract(&contract_id, || MockOracleContract::get_price_data(e.clone(), asset2.clone()));
        assert_eq!(p2, Err(OracleError::StalePrice));
    }

    #[test]
    fn test_failure_injection_stale_price() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });
        e.as_contract(&contract_id, || {
            // Set a price first
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
            // Configure stale price failure
            MockOracleContract::configure_failure_mode(
                e.clone(),
                admin.clone(),
                Symbol::new(&e, "stale_price"),
                true,
            )
            .unwrap();
        });

        let result = e.as_contract(&contract_id, || MockOracleContract::get_price(e.clone(), asset.clone()));
        assert_eq!(result, Err(OracleError::StalePrice));
    }

    #[test]
    fn test_failure_injection_oracle_paused() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });
        e.as_contract(&contract_id, || {
            // Configure oracle paused failure
            MockOracleContract::configure_failure_mode(
                e.clone(),
                admin.clone(),
                Symbol::new(&e, "oracle_paused"),
                true,
            )
            .unwrap();
        });

        let result = e.as_contract(&contract_id, || MockOracleContract::get_price(e.clone(), asset.clone()));
        assert_eq!(result, Err(OracleError::NotInitialized));
    }

    #[test]
    fn test_failure_injection_invalid_price() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });
        e.as_contract(&contract_id, || {
            // Configure invalid price failure
            MockOracleContract::configure_failure_mode(
                e.clone(),
                admin.clone(),
                Symbol::new(&e, "invalid_price"),
                true,
            )
            .unwrap();
        });

        let result = e.as_contract(&contract_id, || MockOracleContract::get_price(e.clone(), asset.clone()));
        assert_eq!(result, Err(OracleError::InvalidPrice));
    }

    // ========================================================================
    // VOLATILITY AND ADVANCED TESTS
    // ========================================================================

    #[test]
    fn test_price_volatility_simulation() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });
        e.as_contract(&contract_id, || {
            // Set base price
            MockOracleContract::set_test_price(e.clone(), admin.clone(), asset.clone(), 100_000_000)
                .unwrap();
        });
        e.as_contract(&contract_id, || {
            // Set volatility factor (500 basis points = 5%)
            MockOracleContract::set_volatility_factor(e.clone(), admin.clone(), 500).unwrap();
        });

        let price = e
            .as_contract(&contract_id, || MockOracleContract::get_price(e.clone(), asset.clone()).unwrap());
        // Should be approximately 105,000,000 (100M + 5%)
        assert!(price > 100_000_000);
        assert!(price < 110_000_000);
    }

    #[test]
    fn test_query_delay_configuration() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);

        e.as_contract(&contract_id, || {
            // Set query delay
            MockOracleContract::set_query_delay(e.clone(), admin.clone(), 30).unwrap();
            
            // Delay should be stored (actual delay simulation would be more complex)
            // For this test, we just verify the function works
        });
    }

    #[test]
    fn test_clear_test_configurations() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_mode(e.clone(), admin.clone(), true).unwrap();
        });
        e.as_contract(&contract_id, || {
            MockOracleContract::set_test_price(e.clone(), admin.clone(), asset.clone(), 500_000_000)
                .unwrap();
        });
        e.as_contract(&contract_id, || {
            MockOracleContract::set_volatility_factor(e.clone(), admin.clone(), 1000).unwrap();
        });
        e.as_contract(&contract_id, || {
            // Clear configurations
            MockOracleContract::clear_test_configurations(e.clone(), admin.clone()).unwrap();
        });

        let result = e.as_contract(&contract_id, || MockOracleContract::get_price(e.clone(), asset.clone()));
        assert_eq!(result, Err(OracleError::PriceNotFound));
    }

    // ========================================================================
    // SECURITY AND BOUNDARY TESTS
    // ========================================================================

    #[test]
    fn test_configure_failure_mode_unauthorized() {
        let e = Env::default();
        e.mock_all_auths();
        let (_admin, contract_id) = create_test_contract(&e);
        let non_admin = Address::generate(&e);

        e.as_contract(&contract_id, || {
            let res = MockOracleContract::configure_failure_mode(
                e.clone(), 
                non_admin.clone(), 
                Symbol::new(&e, "test_failure"), 
                true
            );
            assert_eq!(res, Err(OracleError::Unauthorized));
        });
    }

    #[test]
    fn test_set_volatility_factor_unauthorized() {
        let e = Env::default();
        e.mock_all_auths();
        let (_admin, contract_id) = create_test_contract(&e);
        let non_admin = Address::generate(&e);

        e.as_contract(&contract_id, || {
            let res = MockOracleContract::set_volatility_factor(
                e.clone(),
                non_admin.clone(),
                1000,
            );
            assert_eq!(res, Err(OracleError::Unauthorized));
        });
    }

    #[test]
    fn test_test_mode_isolation() {
        let e = Env::default();
        e.mock_all_auths();
        let (admin, contract_id) = create_test_contract(&e);
        let asset = Address::generate(&e);

        e.as_contract(&contract_id, || {
            // Set price without test mode
            MockOracleContract::set_price(
                e.clone(),
                admin.clone(),
                asset.clone(),
                100_000_000,
                8,
                1000,
            ).unwrap();
            
            // Should get regular price when test mode is disabled
            let price = MockOracleContract::get_price(e.clone(), asset.clone()).unwrap();
            assert_eq!(price, 100_000_000);
        });
    }
}
