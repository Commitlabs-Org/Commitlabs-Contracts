#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec, Map, i128, symbol_short, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentRules {
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String, // "safe", "balanced", "aggressive"
    pub early_exit_penalty: u32,
    pub min_fee_threshold: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commitment {
    pub commitment_id: String,
    pub owner: Address,
    pub nft_token_id: u32,
    pub rules: CommitmentRules,
    pub amount: i128,
    pub asset_address: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub current_value: i128,
    pub status: String, // "active", "settled", "violated", "early_exit"
}

// Storage keys for access control
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const NFT_CONTRACT_KEY: Symbol = symbol_short!("NFT_CT");
const AUTHORIZED_ALLOCATOR_KEY: Symbol = symbol_short!("AUTH_AL");
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
pub struct AuthorizedAllocatorAddedEvent {
    pub allocator_address: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizedAllocatorRemovedEvent {
    pub allocator_address: Address,
}

#[contract]
pub struct CommitmentCoreContract;

// Access control helper functions
impl CommitmentCoreContract {
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

    /// Get the NFT contract address
    fn get_nft_contract(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&NFT_CONTRACT_KEY)
            .expect("NFT contract not set")
    }

    /// Set the NFT contract address
    fn set_nft_contract(e: &Env, nft_contract: &Address) {
        e.storage().instance().set(&NFT_CONTRACT_KEY, nft_contract);
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

    /// Check if an address is authorized allocator
    fn is_authorized_allocator(e: &Env, address: &Address) -> bool {
        let admin = Self::get_admin(e);
        if *address == admin {
            return true;
        }
        
        // Check whitelist
        let key = (AUTHORIZED_ALLOCATOR_KEY, address);
        e.storage().instance().has(&key)
    }

    /// Require that caller is authorized allocator
    fn require_authorized_allocator(e: &Env) {
        let caller = e.invoker();
        if !Self::is_authorized_allocator(e, &caller) {
            panic!("Unauthorized: admin or authorized allocator access required");
        }
    }

    /// Add an authorized allocator to whitelist
    fn add_authorized_allocator(e: &Env, allocator_address: &Address) {
        let key = (AUTHORIZED_ALLOCATOR_KEY, allocator_address);
        e.storage().instance().set(&key, &true);
        
        // Emit event
        e.events().publish(
            (symbol_short!("alloc_add"), allocator_address),
            AuthorizedAllocatorAddedEvent {
                allocator_address: allocator_address.clone(),
            },
        );
    }

    /// Remove an authorized allocator from whitelist
    fn remove_authorized_allocator(e: &Env, allocator_address: &Address) {
        let key = (AUTHORIZED_ALLOCATOR_KEY, allocator_address);
        if e.storage().instance().has(&key) {
            e.storage().instance().remove(&key);
            
            // Emit event
            e.events().publish(
                (symbol_short!("alloc_rm"), allocator_address),
                AuthorizedAllocatorRemovedEvent {
                    allocator_address: allocator_address.clone(),
                },
            );
        }
    }

    /// Verify that caller is the owner of a commitment
    fn require_owner(e: &Env, owner: &Address) {
        let caller = e.invoker();
        if caller != *owner {
            panic!("Unauthorized: only commitment owner can perform this action");
        }
    }
}

#[contractimpl]
impl CommitmentCoreContract {
    /// Initialize the core commitment contract
    pub fn initialize(e: Env, admin: Address, nft_contract: Address) {
        if Self::is_initialized(&e) {
            panic!("Contract already initialized");
        }
        
        Self::set_admin(&e, &admin);
        Self::set_nft_contract(&e, &nft_contract);
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

    /// Add an authorized allocator to whitelist (admin-only)
    pub fn add_authorized_allocator(e: Env, allocator_address: Address) {
        Self::require_admin(&e);
        Self::add_authorized_allocator(&e, &allocator_address);
    }

    /// Remove an authorized allocator from whitelist (admin-only)
    pub fn remove_authorized_allocator(e: Env, allocator_address: Address) {
        Self::require_admin(&e);
        Self::remove_authorized_allocator(&e, &allocator_address);
    }

    /// Check if an address is an authorized allocator
    pub fn is_authorized_allocator(e: Env, allocator_address: Address) -> bool {
        Self::is_authorized_allocator(&e, &allocator_address)
    }

    /// Create a new commitment
    pub fn create_commitment(
        e: Env,
        _owner: Address,
        _amount: i128,
        _asset_address: Address,
        _rules: CommitmentRules,
    ) -> String {
        // TODO: Validate rules
        // TODO: Transfer assets from owner to contract
        // TODO: Call NFT contract to mint Commitment NFT
        // TODO: Store commitment data
        // TODO: Emit creation event
        String::from_str(&e, "commitment_id_placeholder")
    }

    /// Get commitment details
    pub fn get_commitment(e: Env, _commitment_id: String) -> Commitment {
        // TODO: Retrieve commitment from storage
        Commitment {
            commitment_id: String::from_str(&e, "placeholder"),
            owner: Address::from_string(&String::from_str(&e, "placeholder")),
            nft_token_id: 0,
            rules: CommitmentRules {
                duration_days: 0,
                max_loss_percent: 0,
                commitment_type: String::from_str(&e, "placeholder"),
                early_exit_penalty: 0,
                min_fee_threshold: 0,
            },
            amount: 0,
            asset_address: Address::from_string(&String::from_str(&e, "placeholder")),
            created_at: 0,
            expires_at: 0,
            current_value: 0,
            status: String::from_str(&e, "active"),
        }
    }

    /// Update commitment value (called by allocation logic) - authorized allocators only
    pub fn update_value(e: Env, _commitment_id: String, _new_value: i128) {
        Self::require_authorized_allocator(&e);
        
        // TODO: Update current_value
        // TODO: Check if max_loss_percent is violated
        // TODO: Emit value update event
    }

    /// Check if commitment rules are violated
    pub fn check_violations(_e: Env, _commitment_id: String) -> bool {
        // TODO: Check if max_loss_percent exceeded
        // TODO: Check if duration expired
        // TODO: Check other rule violations
        false
    }

    /// Settle commitment at maturity - authorized allocators only
    pub fn settle(e: Env, _commitment_id: String) {
        Self::require_authorized_allocator(&e);
        
        // TODO: Verify commitment is expired
        // TODO: Calculate final settlement amount
        // TODO: Transfer assets back to owner
        // TODO: Mark commitment as settled
        // TODO: Call NFT contract to mark NFT as settled
        // TODO: Emit settlement event
    }

    /// Early exit (with penalty)
    pub fn early_exit(_e: Env, _commitment_id: String, _caller: Address) {
        // TODO: Verify caller is owner
        // TODO: Calculate penalty
        // TODO: Transfer remaining amount (after penalty) to owner
        // TODO: Mark commitment as early_exit
        // TODO: Emit early exit event
    }

    /// Allocate liquidity (called by allocation strategy) - authorized allocators only
    pub fn allocate(e: Env, _commitment_id: String, _target_pool: Address, _amount: i128) {
        Self::require_authorized_allocator(&e);
        
        // TODO: Verify commitment is active
        // TODO: Transfer assets to target pool
        // TODO: Record allocation
        // TODO: Emit allocation event
    }
}

