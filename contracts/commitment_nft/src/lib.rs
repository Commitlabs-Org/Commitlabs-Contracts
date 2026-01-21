#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec, Map, i128, symbol_short, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentMetadata {
    pub commitment_id: String,
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String, // "safe", "balanced", "aggressive"
    pub created_at: u64,
    pub expires_at: u64,
    pub initial_amount: i128,
    pub asset_address: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentNFT {
    pub owner: Address,
    pub token_id: u32,
    pub metadata: CommitmentMetadata,
    pub is_active: bool,
    pub early_exit_penalty: u32,
}

// Storage keys for access control
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const AUTHORIZED_KEY: Symbol = symbol_short!("AUTH");
const INITIALIZED_KEY: Symbol = symbol_short!("INIT");

// Events
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminChangedEvent {
    pub old_admin: Address,
    pub new_admin: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedContractAddedEvent {
    pub contract_address: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedContractRemovedEvent {
    pub contract_address: Address,
}

#[contract]
pub struct CommitmentNFTContract;

// Access control helper functions
impl CommitmentNFTContract {
    /// Get the admin address from storage
    fn get_admin(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("Contract not initialized")
    }

    /// Set the admin address in storage
    fn set_admin(e: &Env, admin: &Address) {
        e.storage().instance().set(&ADMIN_KEY, admin);
    }

    /// Check if contract is initialized
    fn is_initialized(e: &Env) -> bool {
        e.storage().instance().has(&INITIALIZED_KEY)
    }

    /// Mark contract as initialized
    fn set_initialized(e: &Env) {
        e.storage().instance().set(&INITIALIZED_KEY, &true);
    }

    /// Check if caller is admin
    fn require_admin(e: &Env) {
        let admin = Self::get_admin(e);
        let caller = e.invoker();
        if caller != admin {
            panic!("Unauthorized: admin access required");
        }
    }

    /// Check if an address is authorized (admin or whitelisted contract)
    fn is_authorized(e: &Env, address: &Address) -> bool {
        let admin = Self::get_admin(e);
        if *address == admin {
            return true;
        }
        
        // Check whitelist
        let key = (AUTHORIZED_KEY, address);
        e.storage().instance().has(&key)
    }

    /// Require that caller is authorized (admin or whitelisted)
    fn require_authorized(e: &Env) {
        let caller = e.invoker();
        if !Self::is_authorized(e, &caller) {
            panic!("Unauthorized: admin or authorized contract access required");
        }
    }

    /// Add an authorized contract to whitelist
    fn add_authorized_contract(e: &Env, contract_address: &Address) {
        let key = (AUTHORIZED_KEY, contract_address);
        e.storage().instance().set(&key, &true);
        
        // Emit event
        e.events().publish(
            (symbol_short!("auth_add"), contract_address),
            AuthorizedContractAddedEvent {
                contract_address: contract_address.clone(),
            },
        );
    }

    /// Remove an authorized contract from whitelist
    fn remove_authorized_contract(e: &Env, contract_address: &Address) {
        let key = (AUTHORIZED_KEY, contract_address);
        if e.storage().instance().has(&key) {
            e.storage().instance().remove(&key);
            
            // Emit event
            e.events().publish(
                (symbol_short!("auth_rm"), contract_address),
                AuthorizedContractRemovedEvent {
                    contract_address: contract_address.clone(),
                },
            );
        }
    }
}

#[contractimpl]
impl CommitmentNFTContract {
    /// Initialize the NFT contract
    pub fn initialize(e: Env, admin: Address) {
        if Self::is_initialized(&e) {
            panic!("Contract already initialized");
        }
        
        Self::set_admin(&e, &admin);
        Self::set_initialized(&e);
    }

    /// Transfer admin role to a new address (admin-only)
    pub fn transfer_admin(e: Env, new_admin: Address) {
        Self::require_admin(&e);
        
        let old_admin = Self::get_admin(&e);
        Self::set_admin(&e, &new_admin);
        
        // Emit event
        e.events().publish(
            symbol_short!("admin_chg"),
            AdminChangedEvent {
                old_admin,
                new_admin: new_admin.clone(),
            },
        );
    }

    /// Get the current admin address
    pub fn get_admin(e: Env) -> Address {
        Self::get_admin(&e)
    }

    /// Add an authorized contract to whitelist (admin-only)
    pub fn add_authorized_contract(e: Env, contract_address: Address) {
        Self::require_admin(&e);
        Self::add_authorized_contract(&e, &contract_address);
    }

    /// Remove an authorized contract from whitelist (admin-only)
    pub fn remove_authorized_contract(e: Env, contract_address: Address) {
        Self::require_admin(&e);
        Self::remove_authorized_contract(&e, &contract_address);
    }

    /// Check if an address is authorized
    pub fn is_authorized(e: Env, contract_address: Address) -> bool {
        Self::is_authorized(&e, &contract_address)
    }

    /// Mint a new Commitment NFT (admin-only)
    pub fn mint(
        e: Env,
        owner: Address,
        commitment_id: String,
        duration_days: u32,
        max_loss_percent: u32,
        commitment_type: String,
        initial_amount: i128,
        asset_address: Address,
    ) -> u32 {
        Self::require_admin(&e);
        
        // TODO: Generate unique token_id
        // TODO: Calculate expires_at from duration_days
        // TODO: Create CommitmentMetadata
        // TODO: Store NFT data
        // TODO: Emit mint event
        0 // Placeholder token_id
    }

    /// Get NFT metadata by token_id
    pub fn get_metadata(e: Env, token_id: u32) -> CommitmentMetadata {
        // TODO: Retrieve and return metadata
        CommitmentMetadata {
            commitment_id: String::from_str(&e, "placeholder"),
            duration_days: 0,
            max_loss_percent: 0,
            commitment_type: String::from_str(&e, "placeholder"),
            created_at: 0,
            expires_at: 0,
            initial_amount: 0,
            asset_address: Address::from_string(&String::from_str(&e, "placeholder")),
        }
    }

    /// Get owner of NFT
    pub fn owner_of(e: Env, token_id: u32) -> Address {
        // TODO: Retrieve owner from storage
        Address::from_string(&String::from_str(&e, "placeholder"))
    }

    /// Transfer NFT to new owner
    pub fn transfer(e: Env, from: Address, to: Address, token_id: u32) {
        // Verify caller is the owner
        let caller = e.invoker();
        if caller != from {
            panic!("Unauthorized: only owner can transfer");
        }
        
        // TODO: Verify ownership
        // TODO: Check if transfer is allowed (not locked)
        // TODO: Update owner
        // TODO: Emit transfer event
    }

    /// Check if NFT is active
    pub fn is_active(e: Env, token_id: u32) -> bool {
        // TODO: Check if commitment is still active
        false
    }

    /// Mark NFT as settled (after maturity) - authorized contracts only
    pub fn settle(e: Env, token_id: u32) {
        Self::require_authorized(&e);
        
        // TODO: Verify expiration
        // TODO: Mark as inactive
        // TODO: Emit settle event
    }
}

