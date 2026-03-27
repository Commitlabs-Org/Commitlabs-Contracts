//! # Storage Utilities
//!
//! Provides standardized patterns for contract initialization, admin management,
//! and generic instance storage access.
//!
//! ### Patterns
//! * **Initialization**: Prevents multi-initialization of contracts.
//! * **Admin**: Centralizes the storage and retrieval of the contract owner/admin.
//! * **Generic Access**: Type-safe wrappers around Soroban's instance storage.


use soroban_sdk::{Address, Env, Symbol};

/// Constant keys used for well-known storage items.
pub mod keys {
    use soroban_sdk::{symbol_short, Symbol};

    /// Key for the contract's admin `Address`.
    pub const ADMIN: Symbol = symbol_short!("ADMIN");
    /// Key for the boolean initialization flag.
    pub const INITIALIZED: Symbol = symbol_short!("INIT");
}

/// Helper for managing a contract's instance storage state.
pub struct Storage;

impl Storage {
    /// Checks if the contract has been marked as initialized.
    pub fn is_initialized(e: &Env) -> bool {
        e.storage().instance().has(&keys::INITIALIZED)
    }

    /// Asserts that the contract has been initialized.
    ///
    /// ### Errors
    /// * Panics with "Contract not initialized" if `is_initialized` is false.
    pub fn require_initialized(e: &Env) {
        if !Self::is_initialized(e) {
            panic!("Contract not initialized");
        }
    }

    /// Sets the initialization flag to true.
    pub fn set_initialized(e: &Env) {
        e.storage().instance().set(&keys::INITIALIZED, &true);
    }

    /// Retrieves the stored administrator address.
    ///
    /// ### Returns
    /// * The `Address` currently stored under `ADMIN`.
    ///
    /// ### Errors
    /// * Panics if the contract is not initialized.
    /// * Panics if the admin has not been set.
    pub fn get_admin(e: &Env) -> Address {
        Self::require_initialized(e);
        e.storage()
            .instance()
            .get::<_, Address>(&keys::ADMIN)
            .unwrap_or_else(|| panic!("Admin not set"))
    }

    /// Updates the administrator address in storage.
    pub fn set_admin(e: &Env, admin: &Address) {
        e.storage().instance().set(&keys::ADMIN, admin);
    }

    /// Asserts that the contract has NOT yet been initialized.
    ///
    /// ### Errors
    /// * Panics with "Contract already initialized" if `is_initialized` is true.
    pub fn require_not_initialized(e: &Env) {
        if Self::is_initialized(e) {
            panic!("Contract already initialized");
        }
    }

    /// Retrieves a value from instance storage, falling back to a default if missing.
    ///
    /// ### Parameters
    /// * `key` - The storage key symbol.
    /// * `default` - The value to return if the key does not exist.
    pub fn get_or_default<T>(e: &Env, key: &Symbol, default: T) -> T
    where
        T: Clone + soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
    {
        e.storage().instance().get::<_, T>(key).unwrap_or(default)
    }

    /// Stores a value in the contract's instance storage.
    pub fn set<T>(e: &Env, key: &Symbol, value: &T)
    where
        T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
    {
        e.storage().instance().set(key, value);
    }

    /// Retrieves an optional value from instance storage.
    pub fn get<T>(e: &Env, key: &Symbol) -> Option<T>
    where
        T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>,
    {
        e.storage().instance().get::<_, T>(key)
    }

    /// Checks if a key exists in the contract's instance storage.
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
