//! Access control patterns and utilities

use soroban_sdk::{Env, Address, Symbol};
use super::storage::Storage;

/// Access control helper functions
pub struct AccessControl;

impl AccessControl {
    /// Require that the caller is the admin
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `caller` - The caller address
    /// 
    /// # Panics
    /// Panics with "Unauthorized: only admin" if caller is not admin
    pub fn require_admin(e: &Env, caller: &Address) {
        caller.require_auth();
        let admin = Storage::get_admin(e);
        if *caller != admin {
            panic!("Unauthorized: only admin can perform this action");
        }
    }

    /// Require that the caller is authorized (either admin or in authorized list)
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `caller` - The caller address
    /// * `authorized_key` - The storage key prefix for the authorized list
    /// 
    /// # Panics
    /// Panics with "Unauthorized" if caller is not admin or authorized
    pub fn require_admin_or_authorized(e: &Env, caller: &Address, authorized_key: &Symbol) {
        caller.require_auth();
        
        // Check if caller is admin
        let admin = Storage::get_admin(e);
        if *caller == admin {
            return;
        }
        
        // Check if caller is in authorized list using composite key
        let key = (authorized_key.clone(), caller.clone());
        let is_authorized: bool = e.storage()
            .instance()
            .get::<_, bool>(&key)
            .unwrap_or(false);
        if !is_authorized {
            panic!("Unauthorized: caller is not admin or authorized");
        }
    }

    /// Check if an address is the admin
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `address` - The address to check
    /// 
    /// # Returns
    /// `true` if address is admin, `false` otherwise
    pub fn is_admin(e: &Env, address: &Address) -> bool {
        let admin = Storage::get_admin(e);
        *address == admin
    }

    /// Require that the caller is the owner
    /// 
    /// # Arguments
    /// * `_e` - The environment
    /// * `caller` - The caller address
    /// * `owner` - The owner address
    /// 
    /// # Panics
    /// Panics with "Unauthorized: caller is not the owner" if caller != owner
    pub fn require_owner(_e: &Env, caller: &Address, owner: &Address) {
        caller.require_auth();
        if *caller != *owner {
            panic!("Unauthorized: caller is not the owner");
        }
    }

    /// Require that the caller is either the owner or admin
    /// 
    /// # Arguments
    /// * `e` - The environment
    /// * `caller` - The caller address
    /// * `owner` - The owner address
    /// 
    /// # Panics
    /// Panics with "Unauthorized" if caller is neither owner nor admin
    pub fn require_owner_or_admin(e: &Env, caller: &Address, owner: &Address) {
        caller.require_auth();
        
        if *caller == *owner {
            return;
        }
        
        if Self::is_admin(e, caller) {
            return;
        }
        
        panic!("Unauthorized: caller is not the owner or admin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;
    use super::super::storage::Storage;
    use soroban_sdk::{contract, contractimpl};

    // Dummy contract used to provide a valid contract context for access control tests
    #[contract]
    pub struct TestContract;

    #[contractimpl]
    impl TestContract {
        pub fn stub() {}
    }

    #[test]
    fn test_is_admin() {
        let env = Env::default();
        let admin = <soroban_sdk::Address as TestAddress>::generate(&env);

        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            Storage::set_initialized(&env);
            Storage::set_admin(&env, &admin);

            assert!(AccessControl::is_admin(&env, &admin));

            let other = <soroban_sdk::Address as TestAddress>::generate(&env);
            assert!(!AccessControl::is_admin(&env, &other));
        });
    }

    #[test]
    #[should_panic(expected = "Unauthorized function call for address")]
    fn test_require_owner() {
        let env = Env::default();
        let owner = <soroban_sdk::Address as TestAddress>::generate(&env);

        let contract_id = env.register_contract(None, TestContract);

        env.as_contract(&contract_id, || {
            // In a real contract, the host would be provided with proper auth
            // context for `owner`. In this unit test we don't set up auth
            // simulation, so `require_auth` will cause an auth error panic.
            // We assert that this auth check is actually happening.
            AccessControl::require_owner(&env, &owner, &owner);
        });
    }
}
