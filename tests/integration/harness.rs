//! Integration Test Harness
//!
//! This module provides a reusable test harness that:
//! - Boots a Soroban Env
//! - Deploys all necessary contracts
//! - Creates test accounts (admin/user/attacker)
//! - Seeds token balances/allowances
//! - Provides typed contract clients
//! - Supports deterministic time advancement and ledger simulation

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    token::{StellarAssetClient, Client as TokenClient},
    Address, Env, String, Map,
};

use commitment_core::{CommitmentCoreContract, CommitmentRules};
use commitment_nft::CommitmentNFTContract;
use attestation_engine::AttestationEngineContract;
use allocation_logic::{AllocationStrategiesContract, RiskLevel, Strategy};
use mock_oracle::MockOracleContract;

/// Default staleness threshold for oracle (1 hour in seconds)
pub const DEFAULT_STALENESS_THRESHOLD: u64 = 3600;

/// Default test token decimals
pub const TOKEN_DECIMALS: u32 = 7;

/// Default test token supply
pub const INITIAL_TOKEN_SUPPLY: i128 = 1_000_000_000_000_000; // 100M tokens with 7 decimals

/// Default user initial balance
pub const DEFAULT_USER_BALANCE: i128 = 10_000_000_000_000; // 1M tokens with 7 decimals

/// One day in seconds
pub const SECONDS_PER_DAY: u64 = 86400;

/// Test accounts container
pub struct TestAccounts {
    pub admin: Address,
    pub user1: Address,
    pub user2: Address,
    pub attacker: Address,
    pub verifier: Address,
}

impl TestAccounts {
    /// Create new test accounts
    pub fn new(e: &Env) -> Self {
        Self {
            admin: Address::generate(e),
            user1: Address::generate(e),
            user2: Address::generate(e),
            attacker: Address::generate(e),
            verifier: Address::generate(e),
        }
    }
}

/// Deployed contract addresses
pub struct DeployedContracts {
    pub commitment_core: Address,
    pub commitment_nft: Address,
    pub attestation_engine: Address,
    pub allocation_logic: Address,
    pub mock_oracle: Address,
    pub token: Address,
}

/// Main test harness structure
pub struct TestHarness {
    pub env: Env,
    pub accounts: TestAccounts,
    pub contracts: DeployedContracts,
}

impl TestHarness {
    /// Create a new test harness with all contracts deployed and initialized
    pub fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        // Set initial ledger state
        env.ledger().set(LedgerInfo {
            timestamp: 1704067200, // Jan 1, 2024 00:00:00 UTC
            protocol_version: 21,
            sequence_number: 1,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 1000,
            min_persistent_entry_ttl: 1000,
            max_entry_ttl: 10000,
        });

        let accounts = TestAccounts::new(&env);

        // Deploy token contract (Stellar Asset Contract)
        let token_admin = Address::generate(&env);
        let token = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_address = token.address();

        // Deploy all contracts
        let commitment_nft = env.register_contract(None, CommitmentNFTContract);
        let commitment_core = env.register_contract(None, CommitmentCoreContract);
        let attestation_engine = env.register_contract(None, AttestationEngineContract);
        let allocation_logic = env.register_contract(None, AllocationStrategiesContract);
        let mock_oracle = env.register_contract(None, MockOracleContract);

        // Initialize NFT contract first (needed by commitment_core)
        env.as_contract(&commitment_nft, || {
            CommitmentNFTContract::initialize(env.clone(), accounts.admin.clone()).unwrap();
            // Set core contract for NFT
            CommitmentNFTContract::set_core_contract(env.clone(), commitment_core.clone()).unwrap();
        });

        // Initialize commitment_core
        env.as_contract(&commitment_core, || {
            CommitmentCoreContract::initialize(
                env.clone(),
                accounts.admin.clone(),
                commitment_nft.clone(),
            );
        });

        // Initialize attestation_engine
        env.as_contract(&attestation_engine, || {
            AttestationEngineContract::initialize(
                env.clone(),
                accounts.admin.clone(),
                commitment_core.clone(),
            )
            .unwrap();
            // Add verifier
            AttestationEngineContract::add_verifier(
                env.clone(),
                accounts.admin.clone(),
                accounts.verifier.clone(),
            )
            .unwrap();
        });

        // Initialize allocation_logic
        env.as_contract(&allocation_logic, || {
            AllocationStrategiesContract::initialize(
                env.clone(),
                accounts.admin.clone(),
                commitment_core.clone(),
            )
            .unwrap();
        });

        // Initialize mock_oracle
        env.as_contract(&mock_oracle, || {
            MockOracleContract::initialize(
                env.clone(),
                accounts.admin.clone(),
                DEFAULT_STALENESS_THRESHOLD,
            )
            .unwrap();
        });

        // Mint tokens to users
        let token_client = StellarAssetClient::new(&env, &token_address);
        token_client.mint(&accounts.user1, &DEFAULT_USER_BALANCE);
        token_client.mint(&accounts.user2, &DEFAULT_USER_BALANCE);
        token_client.mint(&accounts.attacker, &DEFAULT_USER_BALANCE);

        let contracts = DeployedContracts {
            commitment_core,
            commitment_nft,
            attestation_engine,
            allocation_logic,
            mock_oracle,
            token: token_address,
        };

        Self {
            env,
            accounts,
            contracts,
        }
    }

    /// Create a minimal harness with just token and oracle (for simpler tests)
    pub fn minimal() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        env.ledger().set(LedgerInfo {
            timestamp: 1704067200,
            protocol_version: 21,
            sequence_number: 1,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 1000,
            min_persistent_entry_ttl: 1000,
            max_entry_ttl: 10000,
        });

        let accounts = TestAccounts::new(&env);

        // Deploy token
        let token_admin = Address::generate(&env);
        let token = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_address = token.address();

        // Deploy contracts without full initialization
        let commitment_nft = env.register_contract(None, CommitmentNFTContract);
        let commitment_core = env.register_contract(None, CommitmentCoreContract);
        let attestation_engine = env.register_contract(None, AttestationEngineContract);
        let allocation_logic = env.register_contract(None, AllocationStrategiesContract);
        let mock_oracle = env.register_contract(None, MockOracleContract);

        // Mint tokens
        let token_client = StellarAssetClient::new(&env, &token_address);
        token_client.mint(&accounts.user1, &DEFAULT_USER_BALANCE);
        token_client.mint(&accounts.user2, &DEFAULT_USER_BALANCE);

        let contracts = DeployedContracts {
            commitment_core,
            commitment_nft,
            attestation_engine,
            allocation_logic,
            mock_oracle,
            token: token_address,
        };

        Self {
            env,
            accounts,
            contracts,
        }
    }

    // ========================================================================
    // Time Management Helpers
    // ========================================================================

    /// Advance time by a specified number of seconds
    pub fn advance_time(&self, seconds: u64) {
        let mut ledger = self.env.ledger().get();
        ledger.timestamp += seconds;
        ledger.sequence_number += 1;
        self.env.ledger().set(ledger);
    }

    /// Advance time by a specified number of days
    pub fn advance_days(&self, days: u64) {
        self.advance_time(days * SECONDS_PER_DAY);
    }

    /// Set timestamp to a specific value
    pub fn set_timestamp(&self, timestamp: u64) {
        let mut ledger = self.env.ledger().get();
        ledger.timestamp = timestamp;
        self.env.ledger().set(ledger);
    }

    /// Get current timestamp
    pub fn current_timestamp(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    // ========================================================================
    // Contract Interaction Helpers
    // ========================================================================

    /// Create default commitment rules
    pub fn default_rules(&self) -> CommitmentRules {
        CommitmentRules {
            duration_days: 30,
            max_loss_percent: 10,
            commitment_type: String::from_str(&self.env, "balanced"),
            early_exit_penalty: 5,
            min_fee_threshold: 1000,
        }
    }

    /// Create safe commitment rules
    pub fn safe_rules(&self) -> CommitmentRules {
        CommitmentRules {
            duration_days: 90,
            max_loss_percent: 5,
            commitment_type: String::from_str(&self.env, "safe"),
            early_exit_penalty: 3,
            min_fee_threshold: 500,
        }
    }

    /// Create aggressive commitment rules
    pub fn aggressive_rules(&self) -> CommitmentRules {
        CommitmentRules {
            duration_days: 7,
            max_loss_percent: 25,
            commitment_type: String::from_str(&self.env, "aggressive"),
            early_exit_penalty: 10,
            min_fee_threshold: 2000,
        }
    }

    /// Get token client
    pub fn token_client(&self) -> TokenClient {
        TokenClient::new(&self.env, &self.contracts.token)
    }

    /// Get stellar asset client for minting
    pub fn token_admin_client(&self) -> StellarAssetClient {
        StellarAssetClient::new(&self.env, &self.contracts.token)
    }

    /// Check user balance
    pub fn balance(&self, user: &Address) -> i128 {
        self.token_client().balance(user)
    }

    /// Approve token spending
    pub fn approve_tokens(&self, owner: &Address, spender: &Address, amount: i128) {
        let expiration = self.current_timestamp() + SECONDS_PER_DAY * 365;
        self.token_client()
            .approve(owner, spender, &amount, &(expiration as u32));
    }

    // ========================================================================
    // Oracle Helpers
    // ========================================================================

    /// Set oracle price for an asset
    pub fn set_oracle_price(&self, asset: &Address, price: i128, decimals: u32) {
        self.env.as_contract(&self.contracts.mock_oracle, || {
            MockOracleContract::set_price(
                self.env.clone(),
                self.accounts.admin.clone(),
                asset.clone(),
                price,
                decimals,
                1000, // confidence
            )
            .unwrap();
        });
    }

    /// Set stale oracle price (with old timestamp)
    pub fn set_stale_oracle_price(&self, asset: &Address, price: i128, age_seconds: u64) {
        let stale_timestamp = self.current_timestamp().saturating_sub(age_seconds);
        self.env.as_contract(&self.contracts.mock_oracle, || {
            MockOracleContract::set_price_with_timestamp(
                self.env.clone(),
                self.accounts.admin.clone(),
                asset.clone(),
                price,
                stale_timestamp,
                8,
                1000,
            )
            .unwrap();
        });
    }

    /// Remove oracle price (for testing missing price)
    pub fn remove_oracle_price(&self, asset: &Address) {
        self.env.as_contract(&self.contracts.mock_oracle, || {
            MockOracleContract::remove_price(
                self.env.clone(),
                self.accounts.admin.clone(),
                asset.clone(),
            )
            .unwrap();
        });
    }

    /// Pause oracle (for testing unavailability)
    pub fn pause_oracle(&self) {
        self.env.as_contract(&self.contracts.mock_oracle, || {
            MockOracleContract::pause(self.env.clone(), self.accounts.admin.clone()).unwrap();
        });
    }

    /// Unpause oracle
    pub fn unpause_oracle(&self) {
        self.env.as_contract(&self.contracts.mock_oracle, || {
            MockOracleContract::unpause(self.env.clone(), self.accounts.admin.clone()).unwrap();
        });
    }

    // ========================================================================
    // Allocation Helpers
    // ========================================================================

    /// Register a pool in allocation logic
    pub fn register_pool(&self, pool_id: u32, risk_level: RiskLevel, apy: u32, max_capacity: i128) {
        self.env.as_contract(&self.contracts.allocation_logic, || {
            AllocationStrategiesContract::register_pool(
                self.env.clone(),
                self.accounts.admin.clone(),
                pool_id,
                risk_level,
                apy,
                max_capacity,
            )
            .unwrap();
        });
    }

    /// Setup default pools (one of each risk level)
    pub fn setup_default_pools(&self) {
        self.register_pool(1, RiskLevel::Low, 500, 1_000_000_000_000_000);
        self.register_pool(2, RiskLevel::Medium, 1000, 1_000_000_000_000_000);
        self.register_pool(3, RiskLevel::High, 2000, 1_000_000_000_000_000);
    }

    // ========================================================================
    // Attestation Helpers
    // ========================================================================

    /// Create health check attestation data
    pub fn health_check_data(&self) -> Map<String, String> {
        let mut data = Map::new(&self.env);
        data.set(
            String::from_str(&self.env, "current_value"),
            String::from_str(&self.env, "100000"),
        );
        data.set(
            String::from_str(&self.env, "health_status"),
            String::from_str(&self.env, "healthy"),
        );
        data
    }

    /// Create violation attestation data
    pub fn violation_data(&self, violation_type: &str, severity: &str) -> Map<String, String> {
        let mut data = Map::new(&self.env);
        data.set(
            String::from_str(&self.env, "violation_type"),
            String::from_str(&self.env, violation_type),
        );
        data.set(
            String::from_str(&self.env, "severity"),
            String::from_str(&self.env, severity),
        );
        data
    }

    /// Create fee generation attestation data
    pub fn fee_generation_data(&self, fee_amount: i128) -> Map<String, String> {
        let mut data = Map::new(&self.env);
        let fee_str = Self::i128_to_string(&self.env, fee_amount);
        data.set(String::from_str(&self.env, "fee_amount"), fee_str);
        data
    }

    /// Create drawdown attestation data
    pub fn drawdown_data(&self, drawdown_percent: i128) -> Map<String, String> {
        let mut data = Map::new(&self.env);
        let drawdown_str = Self::i128_to_string(&self.env, drawdown_percent);
        data.set(
            String::from_str(&self.env, "drawdown_percent"),
            drawdown_str,
        );
        data
    }

    /// Convert i128 to String (helper)
    fn i128_to_string(e: &Env, value: i128) -> String {
        // Simple conversion for positive numbers
        if value == 0 {
            return String::from_str(e, "0");
        }

        let mut result = [0u8; 40];
        let mut idx = 39;
        let mut val = if value < 0 { -value } else { value } as u128;

        while val > 0 {
            result[idx] = b'0' + (val % 10) as u8;
            val /= 10;
            if idx > 0 {
                idx -= 1;
            }
        }

        if value < 0 {
            result[idx] = b'-';
        } else {
            idx += 1;
        }

        // Convert slice to String
        let slice = &result[idx..40];
        let s = core::str::from_utf8(slice).unwrap_or("0");
        String::from_str(e, s)
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper macro to assert an error result matches expected error
#[macro_export]
macro_rules! assert_err {
    ($result:expr, $expected:pat) => {
        match $result {
            Err($expected) => (),
            Err(e) => panic!("Expected error {:?}, got {:?}", stringify!($expected), e),
            Ok(_) => panic!("Expected error {:?}, got Ok", stringify!($expected)),
        }
    };
}

/// Helper macro to assert success and extract value
#[macro_export]
macro_rules! assert_ok {
    ($result:expr) => {
        match $result {
            Ok(val) => val,
            Err(e) => panic!("Expected Ok, got Err({:?})", e),
        }
    };
}

#[cfg(test)]
mod harness_tests {
    use super::*;

    #[test]
    fn test_harness_creation() {
        let harness = TestHarness::new();

        // Verify accounts are created
        assert_ne!(harness.accounts.admin, harness.accounts.user1);
        assert_ne!(harness.accounts.user1, harness.accounts.user2);

        // Verify contracts are deployed
        assert_ne!(
            harness.contracts.commitment_core,
            harness.contracts.commitment_nft
        );
    }

    #[test]
    fn test_time_advancement() {
        let harness = TestHarness::new();
        let initial_time = harness.current_timestamp();

        harness.advance_time(100);
        assert_eq!(harness.current_timestamp(), initial_time + 100);

        harness.advance_days(1);
        assert_eq!(
            harness.current_timestamp(),
            initial_time + 100 + SECONDS_PER_DAY
        );
    }

    #[test]
    fn test_token_balances() {
        let harness = TestHarness::new();

        // Verify initial balances
        assert_eq!(harness.balance(&harness.accounts.user1), DEFAULT_USER_BALANCE);
        assert_eq!(harness.balance(&harness.accounts.user2), DEFAULT_USER_BALANCE);
    }

    #[test]
    fn test_oracle_price_setting() {
        let harness = TestHarness::new();
        let asset = Address::generate(&harness.env);

        harness.set_oracle_price(&asset, 100_000_000, 8);

        harness.env.as_contract(&harness.contracts.mock_oracle, || {
            let price =
                MockOracleContract::get_price(harness.env.clone(), asset.clone()).unwrap();
            assert_eq!(price, 100_000_000);
        });
    }
}
