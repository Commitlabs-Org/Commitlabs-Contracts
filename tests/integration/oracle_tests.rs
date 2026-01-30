//! Oracle Integration Tests
//!
//! These tests verify oracle behavior:
//! - Valid price reads
//! - Missing price handling
//! - Stale price detection
//! - Unauthorized update attempts
//! - Price volatility simulation

use crate::harness::{TestHarness, DEFAULT_STALENESS_THRESHOLD};
use soroban_sdk::{testutils::Address as _, Address, Env};

use mock_oracle::{MockOracleContract, OracleError, PriceData};

/// Test: Read valid oracle price
#[test]
fn test_oracle_valid_price_read() {
    let harness = TestHarness::new();
    let asset = Address::generate(&harness.env);
    let price = 100_000_000i128; // $1.00 with 8 decimals

    // Set price
    harness.set_oracle_price(&asset, price, 8);

    // Read price
    let read_price = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone()).unwrap()
        });

    assert_eq!(read_price, price);
}

/// Test: Read full price data with metadata
#[test]
fn test_oracle_full_price_data() {
    let harness = TestHarness::new();
    let asset = Address::generate(&harness.env);
    let price = 250_000_000i128;
    let decimals = 8u32;
    let confidence = 5000i128;

    // Set price with full data
    harness.env.as_contract(&harness.contracts.mock_oracle, || {
        MockOracleContract::set_price(
            harness.env.clone(),
            harness.accounts.admin.clone(),
            asset.clone(),
            price,
            decimals,
            confidence,
        )
        .unwrap();
    });

    // Read full price data
    let price_data = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price_data(harness.env.clone(), asset.clone()).unwrap()
        });

    assert_eq!(price_data.price, price);
    assert_eq!(price_data.decimals, decimals);
    assert_eq!(price_data.confidence, confidence);
    assert!(price_data.timestamp > 0);
}

/// Test: Missing price returns error
#[test]
fn test_oracle_missing_price() {
    let harness = TestHarness::new();
    let unknown_asset = Address::generate(&harness.env);

    // Try to read price for asset that has no price set
    let result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), unknown_asset.clone())
        });

    assert_eq!(result, Err(OracleError::PriceNotFound));
}

/// Test: Stale price detection
#[test]
fn test_oracle_stale_price_detection() {
    let harness = TestHarness::new();
    let asset = Address::generate(&harness.env);
    let price = 100_000_000i128;

    // Set a price with current timestamp
    harness.set_oracle_price(&asset, price, 8);

    // Verify price is fresh
    let fresh_result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone())
        });
    assert!(fresh_result.is_ok());

    // Advance time beyond staleness threshold
    harness.advance_time(DEFAULT_STALENESS_THRESHOLD + 1);

    // Now the price should be stale
    let stale_result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone())
        });
    assert_eq!(stale_result, Err(OracleError::StalePrice));
}

/// Test: Custom staleness check
#[test]
fn test_oracle_custom_staleness_check() {
    let harness = TestHarness::new();
    let asset = Address::generate(&harness.env);
    let price = 100_000_000i128;

    harness.set_oracle_price(&asset, price, 8);

    // Advance time by 100 seconds
    harness.advance_time(100);

    // Check with 50 second max staleness - should be stale
    let stale_result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price_no_older_than(harness.env.clone(), asset.clone(), 50)
        });
    assert_eq!(stale_result, Err(OracleError::StalePrice));

    // Check with 200 second max staleness - should be fresh
    let fresh_result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price_no_older_than(harness.env.clone(), asset.clone(), 200)
        });
    assert!(fresh_result.is_ok());
}

/// Test: Set stale price directly (for testing)
#[test]
fn test_oracle_set_stale_price() {
    let harness = TestHarness::new();
    let asset = Address::generate(&harness.env);
    let price = 100_000_000i128;
    let age = DEFAULT_STALENESS_THRESHOLD + 1000; // Very stale

    // Set stale price using test helper
    harness.set_stale_oracle_price(&asset, price, age);

    // Price should be detected as stale
    let result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone())
        });
    assert_eq!(result, Err(OracleError::StalePrice));
}

/// Test: Unauthorized price update attempt
#[test]
fn test_oracle_unauthorized_update_attempt() {
    let harness = TestHarness::new();
    let asset = Address::generate(&harness.env);
    let attacker = &harness.accounts.attacker;

    // Attacker (not admin or feeder) tries to set price
    let result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::set_price(
                harness.env.clone(),
                attacker.clone(),
                asset.clone(),
                999_999_999,
                8,
                0,
            )
        });

    assert_eq!(result, Err(OracleError::Unauthorized));
}

/// Test: Authorized feeder can update price
#[test]
fn test_oracle_authorized_feeder_update() {
    let harness = TestHarness::new();
    let admin = &harness.accounts.admin;
    let feeder = Address::generate(&harness.env);
    let asset = Address::generate(&harness.env);
    let price = 123_456_789i128;

    // Admin adds feeder
    harness.env.as_contract(&harness.contracts.mock_oracle, || {
        MockOracleContract::add_feeder(harness.env.clone(), admin.clone(), feeder.clone()).unwrap();
    });

    // Verify feeder is authorized
    let is_feeder = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::is_feeder(harness.env.clone(), feeder.clone())
        });
    assert!(is_feeder);

    // Feeder sets price
    harness.env.as_contract(&harness.contracts.mock_oracle, || {
        MockOracleContract::set_price(
            harness.env.clone(),
            feeder.clone(),
            asset.clone(),
            price,
            8,
            1000,
        )
        .unwrap();
    });

    // Verify price was set
    let read_price = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone()).unwrap()
        });
    assert_eq!(read_price, price);
}

/// Test: Remove feeder authorization
#[test]
fn test_oracle_remove_feeder() {
    let harness = TestHarness::new();
    let admin = &harness.accounts.admin;
    let feeder = Address::generate(&harness.env);
    let asset = Address::generate(&harness.env);

    // Add and then remove feeder
    harness.env.as_contract(&harness.contracts.mock_oracle, || {
        MockOracleContract::add_feeder(harness.env.clone(), admin.clone(), feeder.clone()).unwrap();
        MockOracleContract::remove_feeder(harness.env.clone(), admin.clone(), feeder.clone())
            .unwrap();
    });

    // Feeder should no longer be authorized
    let is_feeder = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::is_feeder(harness.env.clone(), feeder.clone())
        });
    assert!(!is_feeder);

    // Feeder can't set price anymore
    let result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::set_price(
                harness.env.clone(),
                feeder.clone(),
                asset.clone(),
                100_000_000,
                8,
                1000,
            )
        });
    assert_eq!(result, Err(OracleError::Unauthorized));
}

/// Test: Oracle pause/unpause
#[test]
fn test_oracle_pause_unpause() {
    let harness = TestHarness::new();
    let asset = Address::generate(&harness.env);
    let price = 100_000_000i128;

    // Set price
    harness.set_oracle_price(&asset, price, 8);

    // Verify price can be read
    let result_before = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone())
        });
    assert!(result_before.is_ok());

    // Pause oracle
    harness.pause_oracle();

    // Price read should fail
    let result_paused = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone())
        });
    assert!(result_paused.is_err());

    // Unpause oracle
    harness.unpause_oracle();

    // Price read should work again
    let result_after = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone())
        });
    assert!(result_after.is_ok());
}

/// Test: Remove price (for testing missing price scenarios)
#[test]
fn test_oracle_remove_price() {
    let harness = TestHarness::new();
    let asset = Address::generate(&harness.env);

    // Set price
    harness.set_oracle_price(&asset, 100_000_000, 8);

    // Verify price exists
    let has_price_before = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::has_price(harness.env.clone(), asset.clone())
        });
    assert!(has_price_before);

    // Remove price
    harness.remove_oracle_price(&asset);

    // Price should be gone
    let has_price_after = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::has_price(harness.env.clone(), asset.clone())
        });
    assert!(!has_price_after);

    // Reading should return error
    let result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone())
        });
    assert_eq!(result, Err(OracleError::PriceNotFound));
}

/// Test: Price volatility simulation
#[test]
fn test_oracle_price_volatility_simulation() {
    let harness = TestHarness::new();
    let asset = Address::generate(&harness.env);

    // Simulate price changes over time
    let prices = vec![
        100_000_000i128, // $1.00
        102_000_000,     // $1.02 (+2%)
        98_000_000,      // $0.98 (-4%)
        105_000_000,     // $1.05 (+7%)
        95_000_000,      // $0.95 (-10%)
    ];

    for (i, &price) in prices.iter().enumerate() {
        harness.advance_time(60); // 1 minute between updates
        harness.set_oracle_price(&asset, price, 8);

        // Verify price is updated
        let read_price = harness
            .env
            .as_contract(&harness.contracts.mock_oracle, || {
                MockOracleContract::get_price(harness.env.clone(), asset.clone()).unwrap()
            });
        assert_eq!(read_price, price, "Price mismatch at iteration {}", i);
    }
}

/// Test: Multiple assets with different prices
#[test]
fn test_oracle_multiple_assets() {
    let harness = TestHarness::new();

    let asset1 = Address::generate(&harness.env);
    let asset2 = Address::generate(&harness.env);
    let asset3 = Address::generate(&harness.env);

    let price1 = 100_000_000i128;
    let price2 = 50_000_000i128;
    let price3 = 200_000_000i128;

    // Set prices for all assets
    harness.set_oracle_price(&asset1, price1, 8);
    harness.set_oracle_price(&asset2, price2, 8);
    harness.set_oracle_price(&asset3, price3, 8);

    // Verify each price
    let read1 = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset1.clone()).unwrap()
        });
    let read2 = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset2.clone()).unwrap()
        });
    let read3 = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset3.clone()).unwrap()
        });

    assert_eq!(read1, price1);
    assert_eq!(read2, price2);
    assert_eq!(read3, price3);
}

/// Test: Update staleness threshold
#[test]
fn test_oracle_staleness_threshold_update() {
    let harness = TestHarness::new();
    let admin = &harness.accounts.admin;
    let asset = Address::generate(&harness.env);

    // Set price
    harness.set_oracle_price(&asset, 100_000_000, 8);

    // Advance time to make price stale under default threshold
    harness.advance_time(DEFAULT_STALENESS_THRESHOLD + 100);

    // Price should be stale
    let stale_result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone())
        });
    assert_eq!(stale_result, Err(OracleError::StalePrice));

    // Increase staleness threshold
    let new_threshold = DEFAULT_STALENESS_THRESHOLD * 2;
    harness.env.as_contract(&harness.contracts.mock_oracle, || {
        MockOracleContract::set_staleness_threshold(
            harness.env.clone(),
            admin.clone(),
            new_threshold,
        )
        .unwrap();
    });

    // Now price should be fresh again
    let fresh_result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_price(harness.env.clone(), asset.clone())
        });
    assert!(fresh_result.is_ok());
}

/// Test: Invalid price rejection
#[test]
fn test_oracle_invalid_price_rejection() {
    let harness = TestHarness::new();
    let admin = &harness.accounts.admin;
    let asset = Address::generate(&harness.env);

    // Try to set negative price
    let result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::set_price(
                harness.env.clone(),
                admin.clone(),
                asset.clone(),
                -100_000_000,
                8,
                1000,
            )
        });

    assert_eq!(result, Err(OracleError::InvalidPrice));
}

/// Test: Oracle initialization
#[test]
fn test_oracle_initialization() {
    let harness = TestHarness::minimal();
    let admin = &harness.accounts.admin;

    // Initialize oracle
    harness.env.as_contract(&harness.contracts.mock_oracle, || {
        MockOracleContract::initialize(harness.env.clone(), admin.clone(), 7200).unwrap();
    });

    // Verify admin
    let stored_admin = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::get_admin(harness.env.clone()).unwrap()
        });
    assert_eq!(stored_admin, *admin);

    // Admin should be a feeder by default
    let is_admin_feeder = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::is_feeder(harness.env.clone(), admin.clone())
        });
    assert!(is_admin_feeder);
}

/// Test: Double initialization fails
#[test]
fn test_oracle_double_initialization_fails() {
    let harness = TestHarness::new();
    let admin = &harness.accounts.admin;

    // Try to initialize again (already initialized in TestHarness::new())
    let result = harness
        .env
        .as_contract(&harness.contracts.mock_oracle, || {
            MockOracleContract::initialize(harness.env.clone(), admin.clone(), 7200)
        });

    assert_eq!(result, Err(OracleError::AlreadyInitialized));
}
