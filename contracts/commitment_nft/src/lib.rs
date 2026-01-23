#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
};
use access_control::{AccessControl, AccessControlError};

// Storage keys
#[contracttype]
pub enum DataKey {
    TotalSupply,
    Nft(u32),   // token_id -> CommitmentNFT
    Owner(u32), // token_id -> Address
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidDuration = 4,
    InvalidMaxLoss = 5,
    InvalidCommitmentType = 6,
    InvalidAmount = 7,
    TokenNotFound = 8,
    AccessControlError = 9,
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

// Events
const MINT: soroban_sdk::Symbol = symbol_short!("mint");

#[contract]
pub struct CommitmentNFTContract;

#[contractimpl]
impl CommitmentNFTContract {
    /// Initialize the NFT contract
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage().instance().has(&access_control::AccessControlKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        AccessControl::init_admin(&e, admin).map_err(|_| Error::AlreadyInitialized)?;
        e.storage().instance().set(&DataKey::TotalSupply, &0u32);
        Ok(())
    }

    /// Add an authorized contract to the whitelist (admin only)
    pub fn add_authorized_contract(
        e: Env,
        caller: Address,
        contract_address: Address,
    ) -> Result<(), Error> {
        AccessControl::add_authorized_contract(&e, caller, contract_address)
            .map_err(Error::from)
    }

    /// Remove an authorized contract from the whitelist (admin only)
    pub fn remove_authorized_contract(
        e: Env,
        caller: Address,
        contract_address: Address,
    ) -> Result<(), Error> {
        AccessControl::remove_authorized_contract(&e, caller, contract_address)
            .map_err(Error::from)
    }

    /// Check if a contract address is authorized
    pub fn is_authorized(e: Env, contract_address: Address) -> bool {
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

    /// Validate commitment type
    fn is_valid_commitment_type(e: &Env, commitment_type: &String) -> bool {
        let safe = String::from_str(e, "safe");
        let balanced = String::from_str(e, "balanced");
        let aggressive = String::from_str(e, "aggressive");
        *commitment_type == safe || *commitment_type == balanced || *commitment_type == aggressive
    }

    /// Mint a new Commitment NFT
    pub fn mint(
        e: Env,
        caller: Address,
        owner: Address,
        commitment_id: String,
        duration_days: u32,
        max_loss_percent: u32,
        commitment_type: String,
        initial_amount: i128,
        asset_address: Address,
    ) -> Result<u32, Error> {
        // Access control: only authorized addresses (admin or whitelisted contracts) can mint
        AccessControl::require_authorized(&e, &caller)?;

        // Validate parameters
        if duration_days == 0 {
            return Err(Error::InvalidDuration);
        }
        if max_loss_percent > 100 {
            return Err(Error::InvalidMaxLoss);
        }
        if !Self::is_valid_commitment_type(&e, &commitment_type) {
            return Err(Error::InvalidCommitmentType);
        }
        if initial_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // Generate unique sequential token_id
        let total_supply: u32 = e
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .ok_or(Error::NotInitialized)?;
        let token_id = total_supply + 1;

        // Calculate timestamps
        let created_at = e.ledger().timestamp();
        let duration_seconds = (duration_days as u64) * 24 * 60 * 60;
        let expires_at = created_at + duration_seconds;

        // Create metadata
        let metadata = CommitmentMetadata {
            commitment_id: commitment_id.clone(),
            duration_days,
            max_loss_percent,
            commitment_type,
            created_at,
            expires_at,
            initial_amount,
            asset_address,
        };

        // Create NFT
        let nft = CommitmentNFT {
            owner: owner.clone(),
            token_id,
            metadata,
            is_active: true,
            early_exit_penalty: 0,
        };

        // Store NFT and ownership
        e.storage().persistent().set(&DataKey::Nft(token_id), &nft);
        e.storage()
            .persistent()
            .set(&DataKey::Owner(token_id), &owner);

        // Increment total supply
        e.storage().instance().set(&DataKey::TotalSupply, &token_id);

        // Emit mint event
        e.events()
            .publish((MINT, token_id), (owner, commitment_id, created_at));

        Ok(token_id)
    }

    /// Get NFT metadata by token_id
    pub fn get_metadata(e: Env, token_id: u32) -> Result<CommitmentMetadata, Error> {
        let nft: CommitmentNFT = e
            .storage()
            .persistent()
            .get(&DataKey::Nft(token_id))
            .ok_or(Error::TokenNotFound)?;
        Ok(nft.metadata)
    }

    /// Get owner of NFT
    pub fn owner_of(e: Env, token_id: u32) -> Result<Address, Error> {
        e.storage()
            .persistent()
            .get(&DataKey::Owner(token_id))
            .ok_or(Error::TokenNotFound)
    }

    /// Transfer NFT to new owner
    pub fn transfer(_e: Env, _from: Address, _to: Address, _token_id: u32) {
        // TODO: Verify ownership
        // TODO: Check if transfer is allowed (not locked)
        // TODO: Update owner
        // TODO: Emit transfer event
    }

    /// Check if NFT is active
    pub fn is_active(e: Env, token_id: u32) -> Result<bool, Error> {
        let nft: CommitmentNFT = e
            .storage()
            .persistent()
            .get(&DataKey::Nft(token_id))
            .ok_or(Error::TokenNotFound)?;
        Ok(nft.is_active)
    }

    /// Get total supply
    pub fn total_supply(e: Env) -> Result<u32, Error> {
        e.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .ok_or(Error::NotInitialized)
    }

    /// Mark NFT as settled (after maturity)
    pub fn settle(_e: Env, _token_id: u32) {
        // TODO: Verify expiration
        // TODO: Mark as inactive
        // TODO: Emit settle event
    }
}

#[cfg(test)]
mod tests;
