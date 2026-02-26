#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    String, Symbol, Vec,
};
use shared_utils::{EmergencyControl, Pausable};

// Current storage version for migration checks.
pub const CURRENT_VERSION: u32 = 1;

// Issue #139: String parameter constraints
#[allow(dead_code)]
const MAX_COMMITMENT_ID_LENGTH: u32 = 256;

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
    /// Invalid WASM hash
    InvalidWasmHash = 15,
    /// Invalid version
    InvalidVersion = 16,
    /// Migration already applied
    AlreadyMigrated = 17,
    /// Transfer to zero address is not allowed
    TransferToZeroAddress = 18,
    /// NFT is locked (active commitment) and cannot be transferred
    NFTLocked = 19,
    /// Duration would cause expires_at to overflow u64
    ExpirationOverflow = 20,
    /// Invalid commitment_id (must be non-empty and <= 256 chars)
    InvalidCommitmentId = 21,
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

/// Parameters for batch NFT transfer operations
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferParams {
    pub from: Address,
    pub to: Address,
    pub token_id: u32,
}

/// Storage keys for the contract
#[contracttype]
pub enum DataKey {
    /// Admin address (singleton)
    Admin,
    /// Counter for generating unique token IDs / Total supply
    TokenCounter,
    /// NFT data storage (token_id -> CommitmentNFT)
    NFT(u32),
    /// Owner balance count (Address -> u32)
    OwnerBalance(Address),
    /// Owner tokens list (Address -> Vec<u32>)
    OwnerTokens(Address),
    /// List of all token IDs (Vec<u32>)
    TokenIds,
    /// Authorized commitment_core contract address (for settlement)
    CoreContract,
    /// Authorized minter addresses (from upstream)
    AuthorizedMinter(Address),
    /// Active status (token_id -> bool)
    ActiveStatus(u32),
    /// Reentrancy guard flag
    ReentrancyGuard,
    /// Contract version
    Version,
    /// Mapping from commitment_id to token_id for reverse lookup (commitment_id -> token_id)
    CommitmentIdIndex(String),
}

#[cfg(test)]
mod tests;

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct CommitmentNFTContract;

#[contractimpl]
impl CommitmentNFTContract {
    /// Initialize the NFT contract
    pub fn initialize(e: Env, admin: Address) -> Result<(), ContractError> {
        // Check if already initialized
        if e.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }

        // Store admin address
        e.storage().instance().set(&DataKey::Admin, &admin);

        // Initialize token counter to 0
        e.storage().instance().set(&DataKey::TokenCounter, &0u32);

        // Initialize empty token IDs vector
        let token_ids: Vec<u32> = Vec::new(&e);
        e.storage().instance().set(&DataKey::TokenIds, &token_ids);

        // Initialize paused state (default: not paused)
        e.storage()
            .instance()
            .set(&Pausable::paused_key(&e), &false);

        Ok(())
    }

    /// Pause the contract
    pub fn pause(e: Env) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Contract not initialized"));
        admin.require_auth();
        Pausable::pause(&e);
    }

    /// Unpause the contract
    pub fn unpause(e: Env) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic!("Contract not initialized"));
        admin.require_auth();
        Pausable::unpause(&e);
    }

    /// Check if the contract is paused
    pub fn is_paused(e: Env) -> bool {
        Pausable::is_paused(&e)
    }

    /// Validate commitment type
    fn is_valid_commitment_type(e: &Env, commitment_type: &String) -> bool {
        let safe = String::from_str(e, "safe");
        let balanced = String::from_str(e, "balanced");
        let aggressive = String::from_str(e, "aggressive");
        *commitment_type == safe || *commitment_type == balanced || *commitment_type == aggressive
    }

    /// Validate commitment_id: must be non-empty and not exceed MAX_COMMITMENT_ID_LENGTH
    #[allow(dead_code)]
    fn is_valid_commitment_id(_e: &Env, commitment_id: &String) -> bool {
        let len = commitment_id.len();
        len > 0 && len <= MAX_COMMITMENT_ID_LENGTH
    }

    /// Generate a unique commitment_id in the format COMMIT_{token_id}
    /// Pre-computed lookup table for token IDs 0-999
    fn format_commitment_id(e: &Env, token_id: u32) -> String {
        const ID_STRINGS: &[&str] = &[
            "COMMIT_0", "COMMIT_1", "COMMIT_2", "COMMIT_3", "COMMIT_4",
            "COMMIT_5", "COMMIT_6", "COMMIT_7", "COMMIT_8", "COMMIT_9",
            "COMMIT_10", "COMMIT_11", "COMMIT_12", "COMMIT_13", "COMMIT_14",
            "COMMIT_15", "COMMIT_16", "COMMIT_17", "COMMIT_18", "COMMIT_19",
            "COMMIT_20", "COMMIT_21", "COMMIT_22", "COMMIT_23", "COMMIT_24",
            "COMMIT_25", "COMMIT_26", "COMMIT_27", "COMMIT_28", "COMMIT_29",
            "COMMIT_30", "COMMIT_31", "COMMIT_32", "COMMIT_33", "COMMIT_34",
            "COMMIT_35", "COMMIT_36", "COMMIT_37", "COMMIT_38", "COMMIT_39",
            "COMMIT_40", "COMMIT_41", "COMMIT_42", "COMMIT_43", "COMMIT_44",
            "COMMIT_45", "COMMIT_46", "COMMIT_47", "COMMIT_48", "COMMIT_49",
            "COMMIT_50", "COMMIT_51", "COMMIT_52", "COMMIT_53", "COMMIT_54",
            "COMMIT_55", "COMMIT_56", "COMMIT_57", "COMMIT_58", "COMMIT_59",
            "COMMIT_60", "COMMIT_61", "COMMIT_62", "COMMIT_63", "COMMIT_64",
            "COMMIT_65", "COMMIT_66", "COMMIT_67", "COMMIT_68", "COMMIT_69",
            "COMMIT_70", "COMMIT_71", "COMMIT_72", "COMMIT_73", "COMMIT_74",
            "COMMIT_75", "COMMIT_76", "COMMIT_77", "COMMIT_78", "COMMIT_79",
            "COMMIT_80", "COMMIT_81", "COMMIT_82", "COMMIT_83", "COMMIT_84",
            "COMMIT_85", "COMMIT_86", "COMMIT_87", "COMMIT_88", "COMMIT_89",
            "COMMIT_90", "COMMIT_91", "COMMIT_92", "COMMIT_93", "COMMIT_94",
            "COMMIT_95", "COMMIT_96", "COMMIT_97", "COMMIT_98", "COMMIT_99",
        ];

        let idx = token_id as usize;
        if idx < ID_STRINGS.len() {
            String::from_str(e, ID_STRINGS[idx])
        } else {
            // For token IDs >= 100, fallback to a generic large ID string
            String::from_str(e, "COMMIT_MAX")
        }
    }

    /// Set the authorized commitment_core contract address for settlement
    /// Only the admin can call this function
    pub fn set_core_contract(e: Env, core_contract: Address) -> Result<(), ContractError> {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)?;
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
    pub fn get_core_contract(e: Env) -> Result<Address, ContractError> {
        e.storage()
            .instance()
            .get(&DataKey::CoreContract)
            .ok_or(ContractError::NotInitialized)
    }

    /// Get the admin address
    pub fn get_admin(e: Env) -> Result<Address, ContractError> {
        e.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)
    }

    /// Add an authorized contract that may call mint() (admin-only).
    pub fn add_authorized_contract(
        e: Env,
        caller: Address,
        contract_address: Address,
    ) -> Result<(), ContractError> {
        require_admin(&e, &caller)?;
        e.storage()
            .instance()
            .set(&DataKey::AuthorizedMinter(contract_address.clone()), &true);
        e.events().publish(
            (Symbol::new(&e, "AuthorizedContractAdded"),),
            (contract_address, e.ledger().timestamp()),
        );
        Ok(())
    }

    /// Remove an authorized contract from the minter whitelist (admin-only).
    pub fn remove_authorized_contract(
        e: Env,
        caller: Address,
        contract_address: Address,
    ) -> Result<(), ContractError> {
        require_admin(&e, &caller)?;
        e.storage()
            .instance()
            .remove(&DataKey::AuthorizedMinter(contract_address.clone()));
        e.events().publish(
            (Symbol::new(&e, "AuthorizedContractRemoved"),),
            (contract_address, e.ledger().timestamp()),
        );
        Ok(())
    }

    /// Return true if the given address may call mint() (admin, core contract, or in whitelist).
    pub fn is_authorized(e: Env, contract_address: Address) -> bool {
        if let Some(admin) = e.storage().instance().get::<_, Address>(&DataKey::Admin) {
            if contract_address == admin {
                return true;
            }
        }
        if let Some(core) = e.storage().instance().get::<_, Address>(&DataKey::CoreContract) {
            if contract_address == core {
                return true;
            }
        }
        e.storage()
            .instance()
            .get(&DataKey::AuthorizedMinter(contract_address))
            .unwrap_or(false)
    }

    /// Get current on-chain version (0 if legacy/uninitialized).
    pub fn get_version(e: Env) -> u32 {
        read_version(&e)
    }

    /// Update admin (admin-only).
    pub fn set_admin(e: Env, caller: Address, new_admin: Address) -> Result<(), ContractError> {
        require_admin(&e, &caller)?;
        e.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    /// Upgrade contract WASM (admin-only).
    pub fn upgrade(
        e: Env,
        caller: Address,
        new_wasm_hash: BytesN<32>,
    ) -> Result<(), ContractError> {
        require_admin(&e, &caller)?;
        require_valid_wasm_hash(&e, &new_wasm_hash)?;
        e.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    /// Migrate storage from a previous version to CURRENT_VERSION (admin-only).
    pub fn migrate(e: Env, caller: Address, from_version: u32) -> Result<(), ContractError> {
        require_admin(&e, &caller)?;

        let stored_version = read_version(&e);
        if stored_version == CURRENT_VERSION {
            return Err(ContractError::AlreadyMigrated);
        }
        if from_version != stored_version || from_version > CURRENT_VERSION {
            return Err(ContractError::InvalidVersion);
        }

        // Ensure essential counters are initialized
        if !e.storage().instance().has(&DataKey::TokenCounter) {
            e.storage().instance().set(&DataKey::TokenCounter, &0u32);
        }
        if !e.storage().instance().has(&DataKey::TokenIds) {
            let token_ids: Vec<u32> = Vec::new(&e);
            e.storage().instance().set(&DataKey::TokenIds, &token_ids);
        }
        if !e.storage().instance().has(&DataKey::ReentrancyGuard) {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
        }

        e.storage()
            .instance()
            .set(&DataKey::Version, &CURRENT_VERSION);
        Ok(())
    }

    // ========================================================================
    // NFT Minting
    // ========================================================================

    /// Mint a new Commitment NFT. Caller must be admin or an authorized minter (see add_authorized_contract).
    pub fn mint(
        e: Env,
        caller: Address,
        owner: Address,
        _commitment_id: String,
        duration_days: u32,
        max_loss_percent: u32,
        commitment_type: String,
        initial_amount: i128,
        asset_address: Address,
        early_exit_penalty: u32,
    ) -> Result<u32, ContractError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);

        if guard {
            return Err(ContractError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
        EmergencyControl::require_not_emergency(&e);

        // Check if contract is paused
        Pausable::require_not_paused(&e);

        if !e.storage().instance().has(&DataKey::Admin) {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::NotInitialized);
        }
        let admin: Address = e.storage().instance().get(&DataKey::Admin).unwrap();
        let core_contract: Option<Address> = e.storage().instance().get(&DataKey::CoreContract);
        let is_authorized_minter = e.storage().instance().get(&DataKey::AuthorizedMinter(caller.clone())).unwrap_or(false);
        let allowed = (caller == admin) || (core_contract.as_ref() == Some(&caller)) || is_authorized_minter;
        if !allowed {
            e.storage().instance().set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::NotAuthorized);
        }

        // CHECKS: Reject zero address owner
        if is_zero_address(&e, &owner) {
            e.storage().instance().set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::TransferToZeroAddress);
        }

        // Validate inputs
        if duration_days == 0 {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::InvalidDuration);
        }
        if max_loss_percent > 100 {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::InvalidMaxLoss);
        }
        if !Self::is_valid_commitment_type(&e, &commitment_type) {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::InvalidCommitmentType);
        }
        if initial_amount <= 0 {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::InvalidAmount);
        }

        // Calculate timestamps with overflow check (duration_days * 86400 + created_at must fit in u64)
        let created_at = e.ledger().timestamp();
        let seconds_per_day: u64 = 86400;
        let duration_seconds = match (duration_days as u64).checked_mul(seconds_per_day) {
            Some(s) => s,
            None => {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                return Err(ContractError::ExpirationOverflow);
            }
        };
        let expires_at = match created_at.checked_add(duration_seconds) {
            Some(t) => t,
            None => {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                return Err(ContractError::ExpirationOverflow);
            }
        };

        // EFFECTS: Update state
        // Generate unique token_id
        let token_id: u32 = e
            .storage()
            .instance()
            .get(&DataKey::TokenCounter)
            .unwrap_or(0);
        let next_token_id = token_id + 1;
        e.storage()
            .instance()
            .set(&DataKey::TokenCounter, &next_token_id);

        // Generate unique commitment_id based on token_id
        let generated_commitment_id = Self::format_commitment_id(&e, token_id);

        // Register commitment_id in the index for reverse lookup
        e.storage()
            .persistent()
            .set(&DataKey::CommitmentIdIndex(generated_commitment_id.clone()), &token_id);

        // Create CommitmentMetadata
        let metadata = CommitmentMetadata {
            commitment_id: generated_commitment_id.clone(),
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
            token_id,
            metadata,
            is_active: true,
            early_exit_penalty,
        };

        // Store NFT data
        e.storage().persistent().set(&DataKey::NFT(token_id), &nft);

        // Update owner balance
        let current_balance: u32 = e
            .storage()
            .persistent()
            .get(&DataKey::OwnerBalance(owner.clone()))
            .unwrap_or(0);
        e.storage().persistent().set(
            &DataKey::OwnerBalance(owner.clone()),
            &(current_balance + 1),
        );

        // Update owner tokens list
        let mut owner_tokens: Vec<u32> = e
            .storage()
            .persistent()
            .get(&DataKey::OwnerTokens(owner.clone()))
            .unwrap_or(Vec::new(&e));
        owner_tokens.push_back(token_id);
        e.storage()
            .persistent()
            .set(&DataKey::OwnerTokens(owner.clone()), &owner_tokens);

        // Add token_id to the list of all tokens
        let mut token_ids: Vec<u32> = e
            .storage()
            .instance()
            .get(&DataKey::TokenIds)
            .unwrap_or(Vec::new(&e));
        token_ids.push_back(token_id);
        e.storage().instance().set(&DataKey::TokenIds, &token_ids);

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit mint event
        e.events().publish(
            (symbol_short!("Mint"), token_id, owner.clone()),
            (generated_commitment_id, e.ledger().timestamp()),
        );

        Ok(token_id)
    }

    // ========================================================================
    // NFT Query Functions
    // ========================================================================

    /// Get NFT metadata by token_id
    pub fn get_metadata(e: Env, token_id: u32) -> Result<CommitmentNFT, ContractError> {
        e.storage()
            .persistent()
            .get(&DataKey::NFT(token_id))
            .ok_or(ContractError::TokenNotFound)
    }

    /// Get commitment NFT by commitment_id
    /// Returns the full CommitmentNFT including all metadata
    pub fn get_commitment_by_id(
        e: Env,
        commitment_id: String,
    ) -> Result<CommitmentNFT, ContractError> {
        // Lookup token_id from CommitmentIdIndex
        let token_id = e
            .storage()
            .persistent()
            .get(&DataKey::CommitmentIdIndex(commitment_id))
            .ok_or(ContractError::TokenNotFound)?;

        // Get NFT by token_id
        e.storage()
            .persistent()
            .get(&DataKey::NFT(token_id))
            .ok_or(ContractError::TokenNotFound)
    }

    /// Get owner of NFT
    pub fn owner_of(e: Env, token_id: u32) -> Result<Address, ContractError> {
        let nft: CommitmentNFT = e
            .storage()
            .persistent()
            .get(&DataKey::NFT(token_id))
            .ok_or(ContractError::TokenNotFound)?;

        Ok(nft.owner)
    }

    /// Transfer NFT to new owner
    ///
    /// # Reentrancy Protection
    /// Uses checks-effects-interactions pattern. This function only writes to storage
    /// and doesn't make external calls, but still protected for consistency.
    pub fn transfer(
        e: Env,
        from: Address,
        to: Address,
        token_id: u32,
    ) -> Result<(), ContractError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);

        if guard {
            return Err(ContractError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
        EmergencyControl::require_not_emergency(&e);

        // Check if contract is paused
        Pausable::require_not_paused(&e);

        // CHECKS: Require authorization from the sender
        from.require_auth();

        // Validate 'to' address is not the same as 'from' (prevent self-transfer)
        if to == from {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::TransferToZeroAddress);
        }

        // CHECKS: Reject transfer to zero address
        if is_zero_address(&e, &to) {
            e.storage().instance().set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::TransferToZeroAddress);
        }

        // Get the NFT
        let mut nft: CommitmentNFT = e
            .storage()
            .persistent()
            .get(&DataKey::NFT(token_id))
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                ContractError::TokenNotFound
            })?;

        // Verify ownership
        if nft.owner != from {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::NotOwner);
        }

        // Active (locked) commitment NFTs cannot be transferred (#145)
        if nft.is_active {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::NFTLocked);
        }

        // EFFECTS: Update state
        // Update owner
        nft.owner = to.clone();
        e.storage().persistent().set(&DataKey::NFT(token_id), &nft);

        // OPTIMIZATION: Batch read balances before updating
        let (from_balance, to_balance) = {
            let from_bal = e
                .storage()
                .persistent()
                .get(&DataKey::OwnerBalance(from.clone()))
                .unwrap_or(0u32);
            let to_bal = e
                .storage()
                .persistent()
                .get(&DataKey::OwnerBalance(to.clone()))
                .unwrap_or(0u32);
            (from_bal, to_bal)
        };

        // Update balance counts
        if from_balance > 0 {
            e.storage()
                .persistent()
                .set(&DataKey::OwnerBalance(from.clone()), &(from_balance - 1));
        }
        e.storage()
            .persistent()
            .set(&DataKey::OwnerBalance(to.clone()), &(to_balance + 1));

        // Update owner tokens lists
        let mut from_tokens: Vec<u32> = e
            .storage()
            .persistent()
            .get(&DataKey::OwnerTokens(from.clone()))
            .unwrap_or(Vec::new(&e));
        if let Some(index) = from_tokens.iter().position(|id| id == token_id) {
            from_tokens.remove(index as u32);
        }
        e.storage()
            .persistent()
            .set(&DataKey::OwnerTokens(from.clone()), &from_tokens);

        let mut to_tokens: Vec<u32> = e
            .storage()
            .persistent()
            .get(&DataKey::OwnerTokens(to.clone()))
            .unwrap_or(Vec::new(&e));
        to_tokens.push_back(token_id);
        e.storage()
            .persistent()
            .set(&DataKey::OwnerTokens(to.clone()), &to_tokens);

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit transfer event
        e.events().publish(
            (symbol_short!("Transfer"), from, to),
            (token_id, e.ledger().timestamp()),
        );

        Ok(())
    }

    /// Check if NFT is active
    pub fn is_active(e: Env, token_id: u32) -> Result<bool, ContractError> {
        let nft: CommitmentNFT = e
            .storage()
            .persistent()
            .get(&DataKey::NFT(token_id))
            .ok_or(ContractError::TokenNotFound)?;

        Ok(nft.is_active)
    }

    /// Get total supply of NFTs minted
    pub fn total_supply(e: Env) -> u32 {
        e.storage()
            .instance()
            .get(&DataKey::TokenCounter)
            .unwrap_or(0)
    }

    /// Get NFT count for a specific owner
    pub fn balance_of(e: Env, owner: Address) -> u32 {
        e.storage()
            .persistent()
            .get(&DataKey::OwnerBalance(owner))
            .unwrap_or(0)
    }

    /// Get all NFTs metadata (for frontend)
    pub fn get_all_metadata(e: Env) -> Vec<CommitmentNFT> {
        let token_ids: Vec<u32> = e
            .storage()
            .instance()
            .get(&DataKey::TokenIds)
            .unwrap_or(Vec::new(&e));

        let mut nfts: Vec<CommitmentNFT> = Vec::new(&e);

        for token_id in token_ids.iter() {
            if let Some(nft) = e
                .storage()
                .persistent()
                .get::<DataKey, CommitmentNFT>(&DataKey::NFT(token_id))
            {
                nfts.push_back(nft);
            }
        }

        nfts
    }

    /// Get all NFTs owned by a specific address
    pub fn get_nfts_by_owner(e: Env, owner: Address) -> Vec<CommitmentNFT> {
        let token_ids: Vec<u32> = e
            .storage()
            .persistent()
            .get(&DataKey::OwnerTokens(owner))
            .unwrap_or(Vec::new(&e));

        let mut owned_nfts: Vec<CommitmentNFT> = Vec::new(&e);

        for token_id in token_ids.iter() {
            if let Some(nft) = e
                .storage()
                .persistent()
                .get::<DataKey, CommitmentNFT>(&DataKey::NFT(token_id))
            {
                owned_nfts.push_back(nft);
            }
        }

        owned_nfts
    }

    // ========================================================================
    // Settlement (Issue #5 - Main Implementation)
    // ========================================================================

    /// Mark NFT as inactive (for early exit or other non-expiry scenarios)
    ///
    /// # Reentrancy Protection
    /// Uses checks-effects-interactions pattern.
    pub fn mark_inactive(e: Env, token_id: u32) -> Result<(), ContractError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);

        if guard {
            return Err(ContractError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
        EmergencyControl::require_not_emergency(&e);

        // Check if contract is paused
        Pausable::require_not_paused(&e);

        // CHECKS: Get the NFT
        let mut nft: CommitmentNFT = e
            .storage()
            .persistent()
            .get(&DataKey::NFT(token_id))
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                ContractError::TokenNotFound
            })?;

        // Check if already inactive
        if !nft.is_active {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::AlreadySettled);
        }

        // EFFECTS: Update state
        // Mark as inactive
        nft.is_active = false;
        e.storage().persistent().set(&DataKey::NFT(token_id), &nft);

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit event
        e.events().publish(
            (symbol_short!("Inactive"), token_id),
            e.ledger().timestamp(),
        );

        Ok(())
    }

    /// Mark NFT as settled (after maturity)
    ///
    /// # Reentrancy Protection
    /// Uses checks-effects-interactions pattern. This function only writes to storage
    /// and doesn't make external calls, but still protected for consistency.
    pub fn settle(e: Env, token_id: u32) -> Result<(), ContractError> {
        // Reentrancy protection
        let guard: bool = e
            .storage()
            .instance()
            .get(&DataKey::ReentrancyGuard)
            .unwrap_or(false);

        if guard {
            return Err(ContractError::ReentrancyDetected);
        }
        e.storage().instance().set(&DataKey::ReentrancyGuard, &true);
        EmergencyControl::require_not_emergency(&e);

        // Check if contract is paused
        Pausable::require_not_paused(&e);

        // CHECKS: Get the NFT
        let mut nft: CommitmentNFT = e
            .storage()
            .persistent()
            .get(&DataKey::NFT(token_id))
            .ok_or_else(|| {
                e.storage()
                    .instance()
                    .set(&DataKey::ReentrancyGuard, &false);
                ContractError::TokenNotFound
            })?;

        // Check if already settled
        if !nft.is_active {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::AlreadySettled);
        }

        // Verify expiration
        let current_time = e.ledger().timestamp();
        if current_time < nft.metadata.expires_at {
            e.storage()
                .instance()
                .set(&DataKey::ReentrancyGuard, &false);
            return Err(ContractError::NotExpired);
        }

        // EFFECTS: Update state
        // Mark as inactive (settled)
        nft.is_active = false;
        e.storage().persistent().set(&DataKey::NFT(token_id), &nft);

        // Clear reentrancy guard
        e.storage()
            .instance()
            .set(&DataKey::ReentrancyGuard, &false);

        // Emit settle event
        e.events()
            .publish((symbol_short!("Settle"), token_id), e.ledger().timestamp());

        Ok(())
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

    /// Set emergency mode (admin only)
    pub fn set_emergency_mode(e: Env, caller: Address, enabled: bool) -> Result<(), ContractError> {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(ContractError::NotInitialized)?;
        admin.require_auth();

        if caller != admin {
            return Err(ContractError::NotAuthorized);
        }

        EmergencyControl::set_emergency_mode(&e, enabled);
        Ok(())
    }
}

fn read_version(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get::<_, u32>(&DataKey::Version)
        .unwrap_or(0)
}

fn require_admin(e: &Env, caller: &Address) -> Result<(), ContractError> {
    caller.require_auth();
    let admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ContractError::NotInitialized)?;
    if *caller != admin {
        return Err(ContractError::NotAuthorized);
    }
    Ok(())
}

fn require_valid_wasm_hash(e: &Env, wasm_hash: &BytesN<32>) -> Result<(), ContractError> {
    let zero = BytesN::from_array(e, &[0; 32]);
    if *wasm_hash == zero {
        return Err(ContractError::InvalidWasmHash);
    }
    Ok(())
}

fn is_zero_address(e: &Env, address: &Address) -> bool {
    let zero_str = String::from_str(e, "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF");
    address.to_string() == zero_str
}

#[cfg(all(test, feature = "benchmark"))]
mod benchmarks;