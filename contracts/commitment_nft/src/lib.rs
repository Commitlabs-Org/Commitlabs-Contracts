#![no_std]
use access_control::{AccessControl, AccessControlError};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol,
};
#[cfg(test)]
mod tests;

// ============================================================================
// Error Types
// ============================================================================

/// Contract errors for structured error handling
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract has not been initialized
    NotInitialized = 1,
    /// Contract has already been initialized
    AlreadyInitialized = 2,
    /// Caller is not authorized to perform this action
    Unauthorized = 3,
    /// Invalid duration (must be > 0)
    InvalidDuration = 4,
    /// Invalid max loss percent (must be 0–100)
    InvalidMaxLoss = 5,
    /// Invalid commitment type (must be safe, balanced, or aggressive)
    InvalidCommitmentType = 6,
    /// Invalid amount (must be > 0)
    InvalidAmount = 7,
    /// NFT with the given token_id does not exist
    TokenNotFound = 8,

    // ------------------------------------------------------------------------
    // Access control
    // ------------------------------------------------------------------------
    AccessControlError = 9,

    // ------------------------------------------------------------------------
    // Commitment / NFT lifecycle
    // ------------------------------------------------------------------------
    /// NFT has already been settled
    AlreadySettled = 10,
    /// Commitment has not expired yet
    NotExpired = 11,
    /// Caller is not the owner of the NFT
    NotOwner = 12,
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

// ============================================================================
// Data Types
// ============================================================================

/// Metadata associated with a commitment NFT
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

/// The Commitment NFT structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentNFT {
    pub owner: Address,
    pub token_id: u32,
    pub metadata: CommitmentMetadata,
    pub is_active: bool,
    pub early_exit_penalty: u32,
}

/// Storage keys for the contract
#[contracttype]
pub enum DataKey {
    /// Admin address (singleton)
    Admin,
    /// Counter for generating unique token IDs / Total supply
    TotalSupply,
    /// NFT data storage (token_id -> CommitmentNFT)
    Nft(u32),
    /// Owner mapping (token_id -> Address)
    Owner(u32),
    /// Authorized minter addresses (from upstream)
    AuthorizedMinter(Address),
    /// Authorized commitment_core contract address (for settlement)
    CoreContract,
    /// Active status (token_id -> bool)
    ActiveStatus(u32),
}

// Events
const MINT: soroban_sdk::Symbol = symbol_short!("mint");

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct CommitmentNFTContract;

#[contractimpl]
impl CommitmentNFTContract {
    // ========================================================================
    // Initialization
    // ========================================================================

    /// Initialize the NFT contract with an admin address
    pub fn initialize(e: Env, admin: Address) -> Result<(), Error> {
        if e.storage()
            .instance()
            .has(&access_control::AccessControlKey::Admin)
        {
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
        AccessControl::add_authorized_contract(&e, caller, contract_address).map_err(Error::from)
    }

    // ========================================================================
    // Access Control
    // ========================================================================

    /// Add an authorized minter (admin or commitment_core contract)
    pub fn add_authorized_minter(e: Env, caller: Address, minter: Address) -> Result<(), Error> {
        caller.require_auth();

        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;

        if caller != admin {
            return Err(Error::Unauthorized);
        }

        e.storage()
            .instance()
            .set(&DataKey::AuthorizedMinter(minter), &true);

        Ok(())
    }

    /// Remove an authorized contract from the whitelist (admin only)
    pub fn remove_authorized_contract(
        e: Env,
        caller: Address,
        contract_address: Address,
    ) -> Result<(), Error> {
        AccessControl::remove_authorized_contract(&e, caller, contract_address).map_err(Error::from)
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

    /// Set the authorized commitment_core contract address for settlement
    /// Only the admin can call this function
    pub fn set_core_contract(e: Env, core_contract: Address) -> Result<(), Error> {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();

        e.storage()
            .instance()
            .set(&DataKey::CoreContract, &core_contract);

        // Emit event for access control change
        e.events()
            .publish((Symbol::new(&e, "CoreContractSet"),), (core_contract,));

        Ok(())
    }

    /// Get the authorized commitment_core contract address
    pub fn get_core_contract(e: Env) -> Result<Address, Error> {
        e.storage()
            .instance()
            .get(&DataKey::CoreContract)
            .ok_or(Error::NotInitialized)
    }

    // ========================================================================
    // NFT Minting
    // ========================================================================

    /// Mint a new Commitment NFT
    ///
    /// # Arguments
    /// * `caller` - The address calling the mint function (must be authorized)
    /// * `owner` - The address that will own the NFT
    /// * `commitment_id` - Unique identifier for the commitment
    /// * `duration_days` - Duration of the commitment in days
    /// * `max_loss_percent` - Maximum allowed loss percentage (0-100)
    /// * `commitment_type` - Type of commitment ("safe", "balanced", "aggressive")
    /// * `initial_amount` - Initial amount committed
    /// * `asset_address` - Address of the asset contract
    ///
    /// # Returns
    /// The token_id of the newly minted NFT
    #[allow(clippy::too_many_arguments)]
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
        e.storage()
            .persistent()
            .set(&DataKey::ActiveStatus(token_id), &true);

        // Increment total supply
        e.storage().instance().set(&DataKey::TotalSupply, &token_id);

        // Emit mint event
        e.events()
            .publish((MINT, token_id), (owner, commitment_id, created_at));

        Ok(token_id)
    }

    // ========================================================================
    // NFT Query Functions
    // ========================================================================

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

    /// Get full NFT data
    pub fn get_nft(e: Env, token_id: u32) -> Result<CommitmentNFT, Error> {
        e.storage()
            .persistent()
            .get(&DataKey::Nft(token_id))
            .ok_or(Error::TokenNotFound)
    }

    // ========================================================================
    // NFT Transfer
    // ========================================================================

    /// Transfer NFT to new owner
    ///
    /// # Arguments
    /// * `from` - Current owner address
    /// * `to` - New owner address
    /// * `token_id` - Token ID to transfer
    ///
    /// # Errors
    /// * `TokenNotFound` - If the NFT does not exist
    /// * `NotOwner` - If the caller is not the owner
    pub fn transfer(e: Env, from: Address, to: Address, token_id: u32) -> Result<(), Error> {
        // Require authorization from the current owner
        from.require_auth();

        // Verify ownership
        let current_owner: Address = e
            .storage()
            .persistent()
            .get(&DataKey::Owner(token_id))
            .ok_or(Error::TokenNotFound)?;
        if current_owner != from {
            return Err(Error::NotOwner);
        }

        // Update owner in storage
        e.storage().persistent().set(&DataKey::Owner(token_id), &to);

        // Update NFT data to reflect new owner
        if let Some(mut nft) = e
            .storage()
            .persistent()
            .get::<DataKey, CommitmentNFT>(&DataKey::Nft(token_id))
        {
            nft.owner = to.clone();
            e.storage().persistent().set(&DataKey::Nft(token_id), &nft);
        }

        // Emit Transfer event
        e.events().publish(
            (Symbol::new(&e, "Transfer"), token_id),
            (from, to, e.ledger().timestamp()),
        );

        Ok(())
    }

    // ========================================================================
    // Settlement (Issue #5 - Main Implementation)
    // ========================================================================

    /// Mark NFT as settled (after maturity)
    ///
    /// This function can only be called by the authorized commitment_core contract.
    /// It marks the NFT as inactive and emits a Settle event.
    ///
    /// # Arguments
    /// * `caller` - The address of the caller (must be commitment_core contract)
    /// * `token_id` - The token ID to settle
    ///
    /// # Errors
    /// * `NotInitialized` - If the contract or core contract is not initialized
    /// * `Unauthorized` - If the caller is not the authorized core contract
    /// * `TokenNotFound` - If the NFT does not exist
    /// * `AlreadySettled` - If the NFT has already been settled
    ///
    /// # Events
    /// Emits a `Settle` event with:
    /// - token_id
    /// - timestamp
    /// - final_status ("settled")
    pub fn settle(e: Env, caller: Address, token_id: u32) -> Result<(), Error> {
        // 1. Access Control: Verify caller signed this transaction
        caller.require_auth();

        // 2. Access Control: Only commitment_core contract can call this
        let core_contract: Address = e
            .storage()
            .instance()
            .get(&DataKey::CoreContract)
            .ok_or(Error::NotInitialized)?;
        if caller != core_contract {
            return Err(Error::Unauthorized);
        }

        // 3. Verify NFT exists
        let mut nft: CommitmentNFT = e
            .storage()
            .persistent()
            .get(&DataKey::Nft(token_id))
            .ok_or(Error::TokenNotFound)?;

        // 4. Check if already settled
        if !nft.is_active {
            return Err(Error::AlreadySettled);
        }

        // 5. Check if commitment has expired (optional - may be handled by core contract)
        // The issue states this is optional since core contract may handle it
        // Uncomment if NFT contract should also verify expiration:
        // if e.ledger().timestamp() < nft.metadata.expires_at {
        //     return Err(Error::NotExpired);
        // }

        // 6. Mark NFT as inactive in storage
        nft.is_active = false;
        e.storage().persistent().set(&DataKey::Nft(token_id), &nft);
        e.storage()
            .persistent()
            .set(&DataKey::ActiveStatus(token_id), &false);

        // 7. Emit Settle event with: token_id, timestamp, final_status
        let timestamp = e.ledger().timestamp();
        let final_status = String::from_str(&e, "settled");
        e.events().publish(
            (Symbol::new(&e, "Settle"), token_id),
            (timestamp, final_status),
        );

        Ok(())
    }
}
