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

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl};

    // Dummy contract used to provide a valid contract context for storage access
    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {
        pub fn stub() {}
    }

    #[test]
    fn test_initialization() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            assert!(!Storage::is_initialized(&env));

            Storage::set_initialized(&env);
            assert!(Storage::is_initialized(&env));
        });
    }

    #[test]
    fn test_admin_storage() {
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
    #[should_panic(expected = "Contract not initialized")]
    fn test_require_initialized_fails() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::require_initialized(&env);
        });
    }

    #[test]
    #[should_panic(expected = "Contract already initialized")]
    fn test_require_not_initialized_fails() {
        let env = Env::default();
        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            Storage::require_not_initialized(&env);
        });
    }
}
