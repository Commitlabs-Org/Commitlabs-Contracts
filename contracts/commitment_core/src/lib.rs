#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String};
use access_control::{AccessControl, AccessControlError};

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

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidCommitment = 4,
    InvalidAmount = 5,
    CommitmentNotFound = 6,
    AccessControlError = 7,
}

impl From<AccessControlError> for Error {
    fn from(err: AccessControlError) -> Self {
        match err {
            AccessControlError::NotInitialized => Error::NotInitialized,
            AccessControlError::Unauthorized => Error::Unauthorized,
            AccessControlError::AlreadyAuthorized => Error::Unauthorized,
            AccessControlError::NotAuthorized => Error::Unauthorized,
            AccessControlError::InvalidAddress => Error::Unauthorized,
        }
    }
}

#[contracttype]
pub enum DataKey {
    NftContract,
    Commitment(String), // commitment_id -> Commitment
}

#[contract]
pub struct CommitmentCoreContract;

#[contractimpl]
impl CommitmentCoreContract {
    /// Initialize the core commitment contract
    pub fn initialize(e: Env, admin: Address, nft_contract: Address) -> Result<(), Error> {
        if e.storage().instance().has(&access_control::AccessControlKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        AccessControl::init_admin(&e, admin).map_err(|_| Error::AlreadyInitialized)?;
        e.storage().instance().set(&DataKey::NftContract, &nft_contract);
        Ok(())
    }

    /// Add an authorized allocator contract to the whitelist (admin only)
    pub fn add_authorized_allocator(
        e: Env,
        caller: Address,
        allocator_address: Address,
    ) -> Result<(), Error> {
        AccessControl::add_authorized_contract(&e, caller, allocator_address)
            .map_err(Error::from)
    }

    /// Remove an authorized allocator contract from the whitelist (admin only)
    pub fn remove_authorized_allocator(
        e: Env,
        caller: Address,
        allocator_address: Address,
    ) -> Result<(), Error> {
        AccessControl::remove_authorized_contract(&e, caller, allocator_address)
            .map_err(Error::from)
    }

    /// Check if a contract address is an authorized allocator
    pub fn is_authorized_allocator(e: Env, contract_address: Address) -> bool {
        AccessControl::is_authorized(&e, &contract_address)
    }

    /// Update admin (admin only)
    pub fn update_admin(e: Env, caller: Address, new_admin: Address) -> Result<(), Error> {
        AccessControl::update_admin(&e, caller, new_admin).map_err(Error::from)
    }

    /// Get the current admin address
    pub fn get_admin(e: Env) -> Result<Address, Error> {
        AccessControl::get_admin(&e).map_err(Error::from)
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
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        // TODO: Retrieve commitment from storage
        // For now, return placeholder data with valid addresses
        let dummy_address = Address::from_string(&String::from_str(&e, "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4"));
        Commitment {
            commitment_id,
            owner: dummy_address.clone(),
            nft_token_id: 0,
            rules: CommitmentRules {
                duration_days: 0,
                max_loss_percent: 0,
                commitment_type: String::from_str(&e, "placeholder"),
                early_exit_penalty: 0,
                min_fee_threshold: 0,
            },
            amount: 0,
            asset_address: dummy_address,
            created_at: 0,
            expires_at: 0,
            current_value: 0,
            status: String::from_str(&e, "active"),
        }
    }

    /// Update commitment value (called by allocation logic)
    pub fn update_value(
        e: Env,
        caller: Address,
        commitment_id: String,
        new_value: i128,
    ) -> Result<(), Error> {
        // Verify caller is authorized (admin or authorized allocator)
        AccessControl::require_authorized(&e, &caller)?;

        // TODO: Get commitment from storage
        // TODO: Update current_value
        // TODO: Check if max_loss_percent is violated
        // TODO: Emit value update event
        Ok(())
    }

    /// Check if commitment rules are violated
    pub fn check_violations(_e: Env, _commitment_id: String) -> bool {
        // TODO: Check if max_loss_percent exceeded
        // TODO: Check if duration expired
        // TODO: Check other rule violations
        false
    }

    /// Settle commitment at maturity
    pub fn settle(_e: Env, _commitment_id: String) {
        // TODO: Verify commitment is expired
        // TODO: Calculate final settlement amount
        // TODO: Transfer assets back to owner
        // TODO: Mark commitment as settled
        // TODO: Call NFT contract to mark NFT as settled
        // TODO: Emit settlement event
    }

    /// Early exit (with penalty)
    pub fn early_exit(e: Env, commitment_id: String, caller: Address) -> Result<(), Error> {
        caller.require_auth();

        // TODO: Get commitment from storage
        // TODO: Verify caller is owner of the commitment
        // TODO: Calculate penalty
        // TODO: Transfer remaining amount (after penalty) to owner
        // TODO: Mark commitment as early_exit
        // TODO: Emit early exit event
        Ok(())
    }

    /// Allocate liquidity (called by allocation strategy)
    pub fn allocate(
        e: Env,
        caller: Address,
        commitment_id: String,
        target_pool: Address,
        amount: i128,
    ) -> Result<(), Error> {
        // Verify caller is authorized (admin or authorized allocator)
        AccessControl::require_authorized(&e, &caller)?;

        // TODO: Verify commitment is active
        // TODO: Transfer assets to target pool
        // TODO: Record allocation
        // TODO: Emit allocation event
        Ok(())
    }
}

