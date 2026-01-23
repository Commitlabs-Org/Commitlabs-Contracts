#![no_std]
use soroban_sdk::{contracttype, symbol_short, Address, Env};

/// Storage keys for access control
#[contracttype]
#[derive(Clone)]
pub enum AccessControlKey {
    Admin,
    AuthorizedContract(Address),
    Owner,
}

/// Access control errors
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AccessControlError {
    NotInitialized = 1,
    Unauthorized = 2,
    AlreadyAuthorized = 3,
    NotAuthorized = 4,
    InvalidAddress = 5,
}

/// Access control module providing admin, owner, and whitelist management
pub struct AccessControl;

impl AccessControl {
    /// Initialize admin (should only be called once during contract initialization)
    pub fn init_admin(e: &Env, admin: Address) -> Result<(), AccessControlError> {
        if e.storage().instance().has(&AccessControlKey::Admin) {
            return Err(AccessControlError::NotInitialized);
        }
        e.storage().instance().set(&AccessControlKey::Admin, &admin);
        Ok(())
    }

    /// Get the admin address
    pub fn get_admin(e: &Env) -> Result<Address, AccessControlError> {
        e.storage()
            .instance()
            .get(&AccessControlKey::Admin)
            .ok_or(AccessControlError::NotInitialized)
    }

    /// Check if an address is the admin
    pub fn is_admin(e: &Env, address: &Address) -> bool {
        Self::get_admin(e)
            .map(|admin| admin == *address)
            .unwrap_or(false)
    }

    /// Require that the caller is the admin
    pub fn require_admin(e: &Env, caller: &Address) -> Result<(), AccessControlError> {
        caller.require_auth();
        if !Self::is_admin(e, caller) {
            return Err(AccessControlError::Unauthorized);
        }
        Ok(())
    }

    /// Initialize owner (optional, for owner-only functions)
    pub fn init_owner(e: &Env, owner: Address) {
        e.storage().instance().set(&AccessControlKey::Owner, &owner);
    }

    /// Get the owner address
    pub fn get_owner(e: &Env) -> Option<Address> {
        e.storage().instance().get(&AccessControlKey::Owner)
    }

    /// Check if an address is the owner
    pub fn is_owner(e: &Env, address: &Address) -> bool {
        Self::get_owner(e)
            .map(|owner| owner == *address)
            .unwrap_or(false)
    }

    /// Require that the caller is the owner
    pub fn require_owner(e: &Env, caller: &Address) -> Result<(), AccessControlError> {
        caller.require_auth();
        if !Self::is_owner(e, caller) {
            return Err(AccessControlError::Unauthorized);
        }
        Ok(())
    }

    /// Add an authorized contract to the whitelist (admin only)
    pub fn add_authorized_contract(
        e: &Env,
        caller: Address,
        contract_address: Address,
    ) -> Result<(), AccessControlError> {
        Self::require_admin(e, &caller)?;

        // Check if already authorized
        if Self::is_authorized(e, &contract_address) {
            return Err(AccessControlError::AlreadyAuthorized);
        }

        e.storage()
            .instance()
            .set(&AccessControlKey::AuthorizedContract(contract_address.clone()), &true);

        // Emit event
        e.events().publish(
            (symbol_short!("auth_add"), caller),
            contract_address,
        );

        Ok(())
    }

    /// Remove an authorized contract from the whitelist (admin only)
    pub fn remove_authorized_contract(
        e: &Env,
        caller: Address,
        contract_address: Address,
    ) -> Result<(), AccessControlError> {
        Self::require_admin(e, &caller)?;

        // Check if authorized
        if !Self::is_authorized(e, &contract_address) {
            return Err(AccessControlError::NotAuthorized);
        }

        e.storage()
            .instance()
            .remove(&AccessControlKey::AuthorizedContract(contract_address.clone()));

        // Emit event
        e.events().publish(
            (symbol_short!("auth_rm"), caller),
            contract_address,
        );

        Ok(())
    }

    /// Check if a contract address is authorized
    pub fn is_authorized(e: &Env, contract_address: &Address) -> bool {
        // Admin is always authorized
        if Self::is_admin(e, contract_address) {
            return true;
        }

        // Check whitelist
        e.storage()
            .instance()
            .get(&AccessControlKey::AuthorizedContract(contract_address.clone()))
            .unwrap_or(false)
    }

    /// Require that the caller is authorized (admin or whitelisted)
    pub fn require_authorized(e: &Env, caller: &Address) -> Result<(), AccessControlError> {
        caller.require_auth();
        if !Self::is_authorized(e, caller) {
            return Err(AccessControlError::Unauthorized);
        }
        Ok(())
    }

    /// Update admin (admin only)
    pub fn update_admin(
        e: &Env,
        caller: Address,
        new_admin: Address,
    ) -> Result<(), AccessControlError> {
        Self::require_admin(e, &caller)?;

        let old_admin = Self::get_admin(e)?;
        e.storage().instance().set(&AccessControlKey::Admin, &new_admin);

        // Emit event
        e.events().publish(
            (symbol_short!("admin_upd"), caller),
            (old_admin, new_admin),
        );

        Ok(())
    }

    /// Update owner (owner only)
    pub fn update_owner(
        e: &Env,
        caller: Address,
        new_owner: Address,
    ) -> Result<(), AccessControlError> {
        Self::require_owner(e, &caller)?;

        let old_owner = Self::get_owner(e).ok_or(AccessControlError::NotInitialized)?;
        e.storage().instance().set(&AccessControlKey::Owner, &new_owner);

        // Emit event
        e.events().publish(
            (symbol_short!("owner_upd"), caller),
            (old_owner, new_owner),
        );

        Ok(())
    }
}
