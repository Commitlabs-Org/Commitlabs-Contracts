//! # Access Control Utilities
//!
//! Provides standardized access control patterns for CommitLabs smart contracts.
//! Supports admin-only, owner-only, and authorized-list patterns with integrated
//! Soroban authentication checks.
//!
//! ### Security
//! * All `require_*` functions MUST call `require_auth()` on the relevant address.
//! * Admin storage is assumed to be initialized via `Storage::set_admin`.
//! * Authorized lists use composite keys `(authorized_key, address)` in instance storage.


use super::storage::Storage;
use soroban_sdk::{Address, Env, Symbol};

/// Access control helper functions for enforcing permission boundaries.
pub struct AccessControl;

impl AccessControl {
    /// Asserts that the caller is the registered administrator.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `caller` - The address to verify as admin.
    ///
    /// ### Errors
    /// * Panics if the caller has not provided valid authentication.
    /// * Panics with "Unauthorized: only admin" if the caller is not the registered admin.
    ///
    /// ### Security
    /// * Implementation calls `caller.require_auth()`, ensuring the transaction is
    ///   signed by the provided address.
    pub fn require_admin(e: &Env, caller: &Address) {
        caller.require_auth();
        let admin = Storage::get_admin(e);
        if *caller != admin {
            panic!("Unauthorized: only admin can perform this action");
        }
    }

    /// Asserts that the caller is either the admin or present in an authorized list.
    ///
    /// Useful for "manager" or "worker" roles where multiple addresses share permissions.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `caller` - The address to verify.
    /// * `authorized_key` - The storage key prefix identifying the specific authorized list.
    ///
    /// ### Errors
    /// * Panics if the caller has not provided valid authentication.
    /// * Panics with "Unauthorized" if the caller is neither the admin nor in the list.
    ///
    /// ### Security
    /// * Checks admin status first as a logical short-circuit.
    /// * Uses instance storage for the authorized list to ensure persistent permission state.
    pub fn require_admin_or_authorized(e: &Env, caller: &Address, authorized_key: &Symbol) {
        caller.require_auth();

        // Check if caller is admin
        let admin = Storage::get_admin(e);
        if *caller == admin {
            return;
        }

        // Check if caller is in authorized list using composite key
        let key = (authorized_key.clone(), caller.clone());
        let is_authorized: bool = e.storage().instance().get::<_, bool>(&key).unwrap_or(false);
        if !is_authorized {
            panic!("Unauthorized: caller is not admin or authorized");
        }
    }

    /// Checks if a given address is the registered administrator.
    ///
    /// Does not require authentication, purely a state check.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `address` - The address to check against the admin record.
    ///
    /// ### Returns
    /// * `true` if the address matches the registered admin, `false` otherwise.
    pub fn is_admin(e: &Env, address: &Address) -> bool {
        let admin = Storage::get_admin(e);
        *address == admin
    }

    /// Asserts that the caller matches a specific owner address.
    ///
    /// Typically used for user-owned resources (e.g., individual commitments).
    ///
    /// ### Parameters
    /// * `_e` - The Soroban environment (unused but kept for pattern consistency).
    /// * `caller` - The address attempting the action.
    /// * `owner` - The address that owns the resource.
    ///
    /// ### Errors
    /// * Panics if the caller has not provided valid authentication.
    /// * Panics with "Unauthorized: caller is not the owner" if addresses don't match.
    ///
    /// ### Security
    /// * Requires explicit authentication of the `caller` address.
    pub fn require_owner(_e: &Env, caller: &Address, owner: &Address) {
        caller.require_auth();
        if *caller != *owner {
            panic!("Unauthorized: caller is not the owner");
        }
    }

    /// Asserts that the caller is either the resource owner or the system admin.
    ///
    /// Allows administrative overrides for user-owned resources.
    ///
    /// ### Parameters
    /// * `e` - The Soroban environment.
    /// * `caller` - The address to verify.
    /// * `owner` - The specific resource owner.
    ///
    /// ### Errors
    /// * Panics if the caller has not provided valid authentication.
    /// * Panics with "Unauthorized" if the caller is neither the owner nor the admin.
    ///
    /// ### Security
    /// * `caller` auth is checked first.
    /// * Admin check allows system-level maintenance even if owner is unavailable.
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
    use super::super::storage::Storage;
    use super::*;
    use soroban_sdk::testutils::Address as TestAddress;
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
