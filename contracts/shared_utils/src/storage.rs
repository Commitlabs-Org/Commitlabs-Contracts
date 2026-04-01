//! Storage helper utilities for common storage patterns

use soroban_sdk::{Address, Env, Symbol};

/// Storage key constants
pub mod keys {
    use soroban_sdk::{symbol_short, Symbol};

    pub const ADMIN: Symbol = symbol_short!("ADMIN");
    pub const INITIALIZED: Symbol = symbol_short!("INIT");
}

/// Storage helper functions
pub struct Storage;

impl Storage {
    /// Check if a contract has been initialized
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Returns
    /// `true` if initialized, `false` otherwise
    pub fn is_initialized(e: &Env) -> bool {
        e.storage().instance().has(&keys::INITIALIZED)
    }

    /// Require that the contract is initialized, panic otherwise
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Panics
    /// Panics with "Contract not initialized" if not initialized
    pub fn require_initialized(e: &Env) {
        if !Self::is_initialized(e) {
            panic!("Contract not initialized");
        }
    }

    /// Mark contract as initialized
    ///
    /// # Arguments
    /// * `e` - The environment
    pub fn set_initialized(e: &Env) {
        e.storage().instance().set(&keys::INITIALIZED, &true);
    }

    /// Get admin address from storage
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Returns
    /// Admin address
    ///
    /// # Panics
    /// Panics if contract not initialized or admin not set
    pub fn get_admin(e: &Env) -> Address {
        Self::require_initialized(e);
        e.storage()
            .instance()
            .get::<_, Address>(&keys::ADMIN)
            .unwrap_or_else(|| panic!("Admin not set"))
    }

    /// Set admin address in storage
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `admin` - The admin address
    pub fn set_admin(e: &Env, admin: &Address) {
        e.storage().instance().set(&keys::ADMIN, admin);
    }

    /// Check if contract is already initialized and panic if so
    ///
    /// # Arguments
    /// * `e` - The environment
    ///
    /// # Panics
    /// Panics with "Contract already initialized" if already initialized
    pub fn require_not_initialized(e: &Env) {
        if Self::is_initialized(e) {
            panic!("Contract already initialized");
        }
    }

    /// Generic storage getter with default value
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `key` - The storage key
    /// * `default` - Default value if key doesn't exist
    ///
    /// # Returns
    /// The stored value or default
    pub fn get_or_default<T>(e: &Env, key: &Symbol, default: T) -> T
    where
        T: Clone + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
    {
        e.storage().instance().get::<_, T>(key).unwrap_or(default)
    }

    /// Generic storage setter
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `key` - The storage key
    /// * `value` - The value to store
    pub fn set<T>(e: &Env, key: &Symbol, value: &T)
    where
        T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        e.storage().instance().set(key, value);
    }

    /// Generic storage getter
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `key` - The storage key
    ///
    /// # Returns
    /// The stored value or None
    pub fn get<T>(e: &Env, key: &Symbol) -> Option<T>
    where
        T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
    {
        e.storage().instance().get::<_, T>(key)
    }

    /// Check if a key exists in storage
    ///
    /// # Arguments
    /// * `e` - The environment
    /// * `key` - The storage key
    ///
    /// # Returns
    /// `true` if key exists, `false` otherwise
    pub fn has(e: &Env, key: &Symbol) -> bool {
        e.storage().instance().has(key)
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, symbol_short};

    // Dummy contract used to provide a valid contract context for storage access
    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {
        pub fn stub() {}
    }

    // ========================================================================
    // Initialization Flag Tests
    // ========================================================================

    #[test]
    fn test_is_initialized_returns_false_by_default() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            assert!(!Storage::is_initialized(&env));
        });
    }

    #[test]
    fn test_set_initialized_marks_contract_as_initialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            assert!(!Storage::is_initialized(&env));

            Storage::set_initialized(&env);
            
            assert!(Storage::is_initialized(&env));
        });
    }

    #[test]
    fn test_set_initialized_is_idempotent() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            assert!(Storage::is_initialized(&env));

            // Setting initialized again should not cause issues
            Storage::set_initialized(&env);
            assert!(Storage::is_initialized(&env));
        });
    }

    #[test]
    fn test_require_initialized_succeeds_when_initialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            
            // Should not panic
            Storage::require_initialized(&env);
        });
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_require_initialized_panics_when_not_initialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::require_initialized(&env);
        });
    }

    #[test]
    fn test_require_not_initialized_succeeds_when_not_initialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            // Should not panic
            Storage::require_not_initialized(&env);
        });
    }

    #[test]
    #[should_panic(expected = "Contract already initialized")]
    fn test_require_not_initialized_panics_when_initialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            Storage::require_not_initialized(&env);
        });
    }

    #[test]
    fn test_initialization_flag_persists_across_calls() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        // First call: set initialized
        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
        });

        // Second call: verify it persists
        env.as_contract(&contract_id, || {
            assert!(Storage::is_initialized(&env));
        });
    }

    #[test]
    fn test_initialization_flag_uses_correct_storage_key() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            
            // Verify the key exists in storage
            assert!(env.storage().instance().has(&keys::INITIALIZED));
            
            // Verify the value is true
            let value: bool = env.storage().instance().get(&keys::INITIALIZED).unwrap();
            assert_eq!(value, true);
        });
    }

    // ========================================================================
    // Admin Storage Tests
    // ========================================================================

    #[test]
    fn test_set_and_get_admin() {
        let env = Env::default();
        let admin = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin);

            let stored_admin = Storage::get_admin(&env);
            assert_eq!(stored_admin, admin);
        });
    }

    #[test]
    fn test_admin_can_be_updated() {
        let env = Env::default();
        let admin1 = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let admin2 = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            
            // Set first admin
            Storage::set_admin(&env, &admin1);
            assert_eq!(Storage::get_admin(&env), admin1);

            // Update to second admin
            Storage::set_admin(&env, &admin2);
            assert_eq!(Storage::get_admin(&env), admin2);
        });
    }

    #[test]
    #[should_panic(expected = "Contract not initialized")]
    fn test_get_admin_panics_when_not_initialized() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::get_admin(&env);
        });
    }

    #[test]
    #[should_panic(expected = "Admin not set")]
    fn test_get_admin_panics_when_admin_not_set() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            // Admin not set yet
            Storage::get_admin(&env);
        });
    }

    #[test]
    fn test_admin_persists_across_calls() {
        let env = Env::default();
        let admin = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let contract_id = env.register_contract(None, TestContract);

        // First call: set admin
        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin);
        });

        // Second call: verify it persists
        env.as_contract(&contract_id, || {
            let stored_admin = Storage::get_admin(&env);
            assert_eq!(stored_admin, admin);
        });
    }

    #[test]
    fn test_admin_uses_correct_storage_key() {
        let env = Env::default();
        let admin = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin);
            
            // Verify the key exists in storage
            assert!(env.storage().instance().has(&keys::ADMIN));
            
            // Verify the value matches
            let stored: Address = env.storage().instance().get(&keys::ADMIN).unwrap();
            assert_eq!(stored, admin);
        });
    }

    #[test]
    fn test_multiple_admins_in_different_contracts() {
        let env = Env::default();
        let admin1 = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let admin2 = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        
        let contract_id1 = env.register_contract(None, TestContract);
        let contract_id2 = env.register_contract(None, TestContract);

        // Set different admins for different contracts
        env.as_contract(&contract_id1, || {
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin1);
        });

        env.as_contract(&contract_id2, || {
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin2);
        });

        // Verify each contract has its own admin
        env.as_contract(&contract_id1, || {
            assert_eq!(Storage::get_admin(&env), admin1);
        });

        env.as_contract(&contract_id2, || {
            assert_eq!(Storage::get_admin(&env), admin2);
        });
    }

    // ========================================================================
    // Generic Storage Helper Tests
    // ========================================================================

    #[test]
    fn test_get_or_default_returns_default_when_key_not_exists() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            let key = symbol_short!("TESTKEY");
            let default_value = 42i128;
            
            let result = Storage::get_or_default(&env, &key, default_value);
            assert_eq!(result, default_value);
        });
    }

    #[test]
    fn test_get_or_default_returns_stored_value_when_exists() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            let key = symbol_short!("TESTKEY");
            let stored_value = 100i128;
            let default_value = 42i128;
            
            Storage::set(&env, &key, &stored_value);
            
            let result = Storage::get_or_default(&env, &key, default_value);
            assert_eq!(result, stored_value);
        });
    }

    #[test]
    fn test_set_and_get_generic_value() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            let key = symbol_short!("TESTKEY");
            let value = 12345i128;
            
            Storage::set(&env, &key, &value);
            
            let result: Option<i128> = Storage::get(&env, &key);
            assert_eq!(result, Some(value));
        });
    }

    #[test]
    fn test_get_returns_none_when_key_not_exists() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            let key = symbol_short!("NOEXIST");
            
            let result: Option<i128> = Storage::get(&env, &key);
            assert_eq!(result, None);
        });
    }

    #[test]
    fn test_has_returns_false_when_key_not_exists() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            let key = symbol_short!("NOEXIST");
            
            assert!(!Storage::has(&env, &key));
        });
    }

    #[test]
    fn test_has_returns_true_when_key_exists() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            let key = symbol_short!("TESTKEY");
            let value = 999i128;
            
            Storage::set(&env, &key, &value);
            
            assert!(Storage::has(&env, &key));
        });
    }

    #[test]
    fn test_storage_with_different_types() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            // Test with i128
            let key_i128 = symbol_short!("INT");
            Storage::set(&env, &key_i128, &12345i128);
            assert_eq!(Storage::get::<i128>(&env, &key_i128), Some(12345i128));

            // Test with u64
            let key_u64 = symbol_short!("UINT");
            Storage::set(&env, &key_u64, &67890u64);
            assert_eq!(Storage::get::<u64>(&env, &key_u64), Some(67890u64));

            // Test with bool
            let key_bool = symbol_short!("BOOL");
            Storage::set(&env, &key_bool, &true);
            assert_eq!(Storage::get::<bool>(&env, &key_bool), Some(true));
        });
    }

    #[test]
    fn test_storage_key_isolation() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            let key1 = symbol_short!("KEY1");
            let key2 = symbol_short!("KEY2");
            
            Storage::set(&env, &key1, &100i128);
            Storage::set(&env, &key2, &200i128);
            
            // Verify keys are isolated
            assert_eq!(Storage::get::<i128>(&env, &key1), Some(100i128));
            assert_eq!(Storage::get::<i128>(&env, &key2), Some(200i128));
        });
    }

    #[test]
    fn test_storage_value_can_be_overwritten() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            let key = symbol_short!("TESTKEY");
            
            Storage::set(&env, &key, &100i128);
            assert_eq!(Storage::get::<i128>(&env, &key), Some(100i128));
            
            Storage::set(&env, &key, &200i128);
            assert_eq!(Storage::get::<i128>(&env, &key), Some(200i128));
        });
    }

    // ========================================================================
    // Storage Key Constants Tests
    // ========================================================================

    #[test]
    fn test_storage_key_constants_are_unique() {
        // Verify that ADMIN and INITIALIZED keys are different
        assert_ne!(keys::ADMIN, keys::INITIALIZED);
    }

    #[test]
    fn test_storage_keys_are_short_symbols() {
        // Verify keys are valid short symbols (max 9 characters)
        // This is implicit in the symbol_short! macro usage
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            // Should not panic - keys are valid
            env.storage().instance().set(&keys::ADMIN, &true);
            env.storage().instance().set(&keys::INITIALIZED, &true);
        });
    }

    // ========================================================================
    // Integration Tests: Initialization + Admin Flow
    // ========================================================================

    #[test]
    fn test_typical_initialization_flow() {
        let env = Env::default();
        let admin = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            // Step 1: Verify not initialized
            assert!(!Storage::is_initialized(&env));
            Storage::require_not_initialized(&env);

            // Step 2: Initialize and set admin
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin);

            // Step 3: Verify initialized
            assert!(Storage::is_initialized(&env));
            Storage::require_initialized(&env);

            // Step 4: Verify admin is set
            assert_eq!(Storage::get_admin(&env), admin);
        });
    }

    #[test]
    #[should_panic(expected = "Contract already initialized")]
    fn test_cannot_reinitialize_after_initialization() {
        let env = Env::default();
        let admin1 = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let admin2 = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            // First initialization
            Storage::require_not_initialized(&env);
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin1);

            // Attempt second initialization should fail
            Storage::require_not_initialized(&env);
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin2);
        });
    }

    #[test]
    fn test_admin_can_be_set_before_initialization() {
        let env = Env::default();
        let admin = <soroban_sdk::Address as soroban_sdk::testutils::Address>::generate(&env);
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            // set_admin doesn't require initialization
            Storage::set_admin(&env, &admin);
            
            // But get_admin does require initialization
            Storage::set_initialized(&env);
            assert_eq!(Storage::get_admin(&env), admin);
        });
    }
}
