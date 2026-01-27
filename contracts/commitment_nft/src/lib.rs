#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String, Vec,
};

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum DataKey {
    Admin,
    TokenCounter,
    Token(u32),
    OwnerTokens(Address),
}

// ============================================================================
// Error Types
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    TokenNotFound = 3,
    NotOwner = 4,
    TokenLocked = 5,
    InvalidRecipient = 6,
    Unauthorized = 7,
}

// ============================================================================
// Data Structures
// ============================================================================

#[cfg(test)]
mod tests;

// ============================================================================
// Error Types
// ============================================================================

/// Contract errors for structured error handling
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    /// Contract has not been initialized
    NotInitialized = 1,
    /// Contract has already been initialized
    AlreadyInitialized = 2,
    /// NFT with the given token_id does not exist
    TokenNotFound = 3,
    /// Invalid token_id
    InvalidTokenId = 4,
    /// Caller is not the owner of the NFT
    NotOwner = 5,
    /// Caller is not authorized to perform this action
    NotAuthorized = 6,
    /// Transfer is not allowed (e.g. restricted)
    TransferNotAllowed = 7,
    /// NFT has already been settled
    AlreadySettled = 8,
    /// Commitment has not expired yet
    NotExpired = 9,
    /// Invalid duration (must be > 0)
    InvalidDuration = 10,
    /// Invalid max loss percent (must be 0-100)
    InvalidMaxLoss = 11,
    /// Invalid commitment type (must be safe, balanced, or aggressive)
    InvalidCommitmentType = 12,
    /// Invalid amount (must be > 0)
    InvalidAmount = 13,
    /// Reentrancy detected
    ReentrancyDetected = 14,
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
    pub fn initialize(e: Env, admin: Address) -> Result<(), ContractError> {
        // Check if already initialized
        if e.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }

        // Require admin authorization
        admin.require_auth();

        // Store admin address
        e.storage().instance().set(&DataKey::Admin, &admin);

        // Initialize token counter to 0
        e.storage().instance().set(&DataKey::TokenCounter, &0u32);

        Ok(())
    }

    // ========================================================================
    // Minting
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
    /// 
    /// # Reentrancy Protection
    /// Uses checks-effects-interactions pattern. This function only writes to storage
    /// and doesn't make external calls, but still protected for consistency.
    pub fn mint(
        e: Env,
        owner: Address,
        commitment_id: String,
        duration_days: u32,
        max_loss_percent: u32,
        commitment_type: String,
        initial_amount: i128,
        asset_address: Address,
    ) -> Result<u32, ContractError> {
        // Verify contract is initialized
        Self::require_initialized(&e)?;

        // Require owner authorization for minting
        owner.require_auth();

        // Generate unique token_id (increment counter)
        let token_id: u32 = e
            .storage()
            .instance()
            .get(&DataKey::TokenCounter)
            .unwrap_or(0);
        let new_token_id = token_id + 1;
        e.storage()
            .instance()
            .set(&DataKey::TokenCounter, &new_token_id);

        // Calculate timestamps
        let created_at = e.ledger().timestamp();
        let expires_at = created_at + (duration_days as u64 * 24 * 60 * 60);

        // Create CommitmentMetadata
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

        // Create CommitmentNFT
        let nft = CommitmentNFT {
            owner: owner.clone(),
            token_id: new_token_id,
            metadata,
            is_active: true,
            early_exit_penalty: 10, // Default 10% penalty
        };

        // Store NFT
        e.storage()
            .persistent()
            .set(&DataKey::Token(new_token_id), &nft);

        // Add to owner's token list
        Self::add_token_to_owner(&e, &owner, new_token_id);

        // Clear reentrancy guard
        e.storage().instance().set(&DataKey::ReentrancyGuard, &false);

        // Emit mint event
        e.events()
            .publish((symbol_short!("mint"), owner, new_token_id), created_at);

        Ok(new_token_id)
    }

    // ========================================================================
    // Transfer Functions
    // ========================================================================

    /// Transfer NFT to new owner
    ///
    /// # Arguments
    /// * `from` - Current owner address
    /// * `to` - New owner address
    /// * `token_id` - Token to transfer
    ///
    /// # Errors
    /// * `TokenNotFound` - Token does not exist
    /// * `NotOwner` - Caller is not the owner
    /// * `TokenLocked` - Token has an active commitment and cannot be transferred
    /// * `InvalidRecipient` - Cannot transfer to zero/invalid address
    pub fn transfer(
        e: Env,
        from: Address,
        to: Address,
        token_id: u32,
    ) -> Result<(), ContractError> {
        // Verify contract is initialized
        Self::require_initialized(&e)?;

        // Require authorization from the 'from' address
        from.require_auth();

        // Verify recipient is not the same as sender
        if from == to {
            return Err(ContractError::InvalidRecipient);
        }

        // Get the NFT (verifies existence)
        let mut nft = Self::get_nft(&e, token_id)?;

        // Verify ownership - from address must be the current owner
        if nft.owner != from {
            e.storage().instance().set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::NotOwner);
        }

        // Check if NFT is locked (active commitment cannot be transferred)
        if nft.is_active {
            return Err(ContractError::TokenLocked);
        }

        // Update ownership
        nft.owner = to.clone();

        // Store updated NFT
        e.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &nft);

        // Update owner token lists
        Self::remove_token_from_owner(&e, &from, token_id);
        Self::add_token_to_owner(&e, &to, token_id);

        // Emit transfer event with from, to, token_id, and timestamp
        let timestamp = e.ledger().timestamp();
        e.events()
            .publish((symbol_short!("transfer"), from, to, token_id), timestamp);

        Ok(())
    }

    // ========================================================================
    // Query Functions
    // ========================================================================

    /// Get NFT metadata by token_id
    pub fn get_metadata(e: Env, token_id: u32) -> Result<CommitmentMetadata, ContractError> {
        let nft = Self::get_nft(&e, token_id)?;
        Ok(nft.metadata)
    }

    /// Get the full NFT data
    pub fn get_nft_data(e: Env, token_id: u32) -> Result<CommitmentNFT, ContractError> {
        Self::get_nft(&e, token_id)
    }

    /// Get owner of NFT
    pub fn owner_of(e: Env, token_id: u32) -> Result<Address, ContractError> {
        let nft = Self::get_nft(&e, token_id)?;
        Ok(nft.owner)
    }

    /// Check if NFT is active (locked)
    pub fn is_active(e: Env, token_id: u32) -> Result<bool, ContractError> {
        let nft = Self::get_nft(&e, token_id)?;
        Ok(nft.is_active)
    }

    /// Get all tokens owned by an address
    pub fn get_tokens_by_owner(e: Env, owner: Address) -> Vec<u32> {
        e.storage()
            .persistent()
            .get(&DataKey::OwnerTokens(owner))
            .unwrap_or(Vec::new(&e))
    }

    // ========================================================================
    // State Management
    // ========================================================================

    /// Mark NFT as settled (after maturity or early exit)
    /// This deactivates the NFT, allowing it to be transferred
    pub fn settle(e: Env, token_id: u32) -> Result<(), ContractError> {
        // Verify contract is initialized
        Self::require_initialized(&e)?;

        // Get the NFT
        let mut nft = Self::get_nft(&e, token_id)?;

        // Require authorization from owner
        nft.owner.require_auth();

        // Check if already settled
        if !nft.is_active {
            return Ok(()); // Already settled, no-op
        }

        // Mark as inactive (settled)
        nft.is_active = false;

        // Store updated NFT
        e.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &nft);

        // Emit settle event
        let timestamp = e.ledger().timestamp();
        e.events()
            .publish((symbol_short!("settle"), nft.owner, token_id), timestamp);

        for token_id in token_ids.iter() {
            if let Some(nft) = e.storage().persistent().get::<DataKey, CommitmentNFT>(&DataKey::NFT(token_id)) {
                owned_nfts.push_back(nft);
            }
        }

        owned_nfts
    }

    /// Activate an NFT (for new commitments)
    /// Only admin can activate tokens
    pub fn activate(e: Env, token_id: u32) -> Result<(), ContractError> {
        // Verify contract is initialized
        Self::require_initialized(&e)?;

        // Get admin and require authorization
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)?;
        admin.require_auth();

        // Get the NFT
        let mut nft = Self::get_nft(&e, token_id)?;

        // Mark as active
        nft.is_active = true;

        // Store updated NFT
        e.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &nft);

        Ok(())
    }

    // ========================================================================
    // Internal Helper Functions
    // ========================================================================

    /// Check if contract is initialized
    fn require_initialized(e: &Env) -> Result<(), ContractError> {
        if !e.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::NotInitialized);
        }
        Ok(())
    }

    /// Get NFT by token_id, returns error if not found
    fn get_nft(e: &Env, token_id: u32) -> Result<CommitmentNFT, ContractError> {
        e.storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .ok_or(ContractError::TokenNotFound)
    }

    /// Add token to owner's token list
    fn add_token_to_owner(e: &Env, owner: &Address, token_id: u32) {
        let mut tokens: Vec<u32> = e
            .storage()
            .persistent()
            .get(&DataKey::OwnerTokens(owner.clone()))
            .unwrap_or(Vec::new(e));

        tokens.push_back(token_id);

        e.storage()
            .persistent()
            .set(&DataKey::OwnerTokens(owner.clone()), &tokens);
    }

    /// Remove token from owner's token list
    fn remove_token_from_owner(e: &Env, owner: &Address, token_id: u32) {
        let tokens: Vec<u32> = e
            .storage()
            .persistent()
            .get(&DataKey::OwnerTokens(owner.clone()))
            .unwrap_or(Vec::new(e));

        let mut new_tokens = Vec::new(e);
        for t in tokens.iter() {
            if t != token_id {
                new_tokens.push_back(t);
            }
        }

        e.storage()
            .persistent()
            .set(&DataKey::OwnerTokens(owner.clone()), &new_tokens);
    }

    /// Check if an NFT has expired (based on time)
    pub fn is_expired(e: Env, token_id: u32) -> Result<bool, ContractError> {
        let nft: CommitmentNFT = e
            .storage()
            .persistent()
            .get(&DataKey::NFT(token_id))
            .ok_or(ContractError::TokenNotFound)?;

        let current_time = e.ledger().timestamp();
        Ok(current_time >= nft.metadata.expires_at)
    }

    /// Check if a token exists
    pub fn token_exists(e: Env, token_id: u32) -> bool {
        e.storage().persistent().has(&DataKey::NFT(token_id))
    }
}

#[cfg(test)]
mod tests;
