#![cfg(test)]

use crate::{CommitmentNFTContract, CommitmentNFTContractClient, ContractError};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

/// Helper function to setup test environment with initialized contract
fn setup_env() -> (Env, Address, CommitmentNFTContractClient<'static>) {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    // Initialize the contract
    client.initialize(&admin);

    (e, admin, client)
}

/// Helper function to mint an NFT for testing
fn mint_test_nft(
    e: &Env,
    client: &CommitmentNFTContractClient,
    owner: &Address,
) -> u32 {
    let commitment_id = String::from_str(e, "test-commitment-1");
    let commitment_type = String::from_str(e, "safe");
    let asset_address = Address::generate(e);

    client.mint(
        owner,
        &commitment_id,
        &30u32,       // duration_days
        &10u32,       // max_loss_percent
        &commitment_type,
        &1000i128,    // initial_amount
        &asset_address,
    )
}

// ============================================
// Initialization Tests
// ============================================

// ============================================================================
// Helper Functions
// ============================================================================

fn setup_env() -> (Env, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    (e, contract_id, admin)
}

fn setup_contract<'a>(e: &'a Env) -> (Address, Address, Address, CommitmentNFTContractClient<'a>) {
    let admin = Address::generate(e);
    let core_contract = Address::generate(e);
    let owner = Address::generate(e);

    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    // Initialize should succeed
    client.initialize(&admin);
}

// ============================================================================
// Initialization Tests
// ============================================================================

#[test]
fn test_initialize_already_initialized() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    // First initialization should succeed
    client.initialize(&admin);

    // Second initialization should fail
    let result = client.try_initialize(&admin);
    assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
}

// ============================================
// Mint Tests
// ============================================

#[test]
fn test_mint() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);

    let token_id = mint_test_nft(&e, &client, &owner);

    // Verify token was minted with correct ID
    assert_eq!(token_id, 1);

    // Verify ownership
    let retrieved_owner = client.owner_of(&token_id);
    assert_eq!(retrieved_owner, owner);

    // Verify NFT is active (locked)
    let is_active = client.is_active(&token_id);
    assert!(is_active);

    // Verify owner's token list
    let tokens = client.get_tokens_by_owner(&owner);
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.get(0).unwrap(), token_id);
}

#[test]
fn test_mint_not_initialized() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    let owner = Address::generate(&e);
    let commitment_id = String::from_str(&e, "test");
    let commitment_type = String::from_str(&e, "safe");
    let asset_address = Address::generate(&e);

    // Minting without initialization should fail
    let result = client.try_mint(
        &owner,
        &commitment_id,
        &30u32,
        &10u32,
        &commitment_type,
        &1000i128,
        &asset_address,
    );
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn test_mint_multiple_tokens() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);

    // Mint multiple tokens
    let token_id_1 = mint_test_nft(&e, &client, &owner);
    let token_id_2 = mint_test_nft(&e, &client, &owner);
    let token_id_3 = mint_test_nft(&e, &client, &owner);

    // Verify unique IDs
    assert_eq!(token_id_1, 1);
    assert_eq!(token_id_2, 2);
    assert_eq!(token_id_3, 3);

    // Verify owner's token list contains all tokens
    let tokens = client.get_tokens_by_owner(&owner);
    assert_eq!(tokens.len(), 3);
}

// ============================================================================
// Transfer Tests - Success Cases
// ============================================================================

#[test]
fn test_transfer_success() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);
    let new_owner = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Settle the NFT first (to unlock it for transfer)
    client.settle(&token_id);

    // Verify NFT is now inactive (settled)
    assert!(!client.is_active(&token_id));

    // Transfer the token
    client.transfer(&owner, &new_owner, &token_id);

    // Verify new ownership
    let retrieved_owner = client.owner_of(&token_id);
    assert_eq!(retrieved_owner, new_owner);

    // Verify old owner's token list is empty
    let old_owner_tokens = client.get_tokens_by_owner(&owner);
    assert_eq!(old_owner_tokens.len(), 0);

    // Verify new owner's token list
    let new_owner_tokens = client.get_tokens_by_owner(&new_owner);
    assert_eq!(new_owner_tokens.len(), 1);
    assert_eq!(new_owner_tokens.get(0).unwrap(), token_id);
}

#[test]
fn test_transfer_multiple_tokens() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);
    let new_owner = Address::generate(&e);

    // Mint multiple tokens
    let token_id_1 = mint_test_nft(&e, &client, &owner);
    let token_id_2 = mint_test_nft(&e, &client, &owner);

    // Settle both NFTs
    client.settle(&token_id_1);
    client.settle(&token_id_2);

    // Transfer only the first token
    client.transfer(&owner, &new_owner, &token_id_1);

    // Verify ownership changes
    assert_eq!(client.owner_of(&token_id_1), new_owner);
    assert_eq!(client.owner_of(&token_id_2), owner);

    // Verify token lists
    let old_owner_tokens = client.get_tokens_by_owner(&owner);
    assert_eq!(old_owner_tokens.len(), 1);
    assert_eq!(old_owner_tokens.get(0).unwrap(), token_id_2);

    let new_owner_tokens = client.get_tokens_by_owner(&new_owner);
    assert_eq!(new_owner_tokens.len(), 1);
    assert_eq!(new_owner_tokens.get(0).unwrap(), token_id_1);
}

// ============================================================================
// Transfer Tests - Ownership Verification
// ============================================================================

#[test]
fn test_transfer_not_owner() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);
    let not_owner = Address::generate(&e);
    let recipient = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Settle the NFT
    client.settle(&token_id);

    // Attempt to transfer from non-owner should fail
    let result = client.try_transfer(&not_owner, &recipient, &token_id);
    assert_eq!(result, Err(Ok(ContractError::NotOwner)));

    // Verify ownership unchanged
    assert_eq!(client.owner_of(&token_id), owner);
}

#[test]
fn test_transfer_verifies_from_is_owner() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);
    let fake_owner = Address::generate(&e);
    let recipient = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Settle the NFT
    client.settle(&token_id);

    // Attempt transfer with wrong 'from' address
    let result = client.try_transfer(&fake_owner, &recipient, &token_id);
    assert_eq!(result, Err(Ok(ContractError::NotOwner)));
}

// ============================================================================
// Transfer Tests - Lock/Active State
// ============================================================================

#[test]
fn test_transfer_locked_nft_fails() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);
    let new_owner = Address::generate(&e);

    // Mint a token (it's active/locked by default)
    let token_id = mint_test_nft(&e, &client, &owner);

    // Verify NFT is active (locked)
    assert!(client.is_active(&token_id));

    // Attempt to transfer locked NFT should fail
    let result = client.try_transfer(&owner, &new_owner, &token_id);
    assert_eq!(result, Err(Ok(ContractError::TokenLocked)));

    // Verify ownership unchanged
    assert_eq!(client.owner_of(&token_id), owner);
}

#[test]
fn test_transfer_after_settle() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);
    let new_owner = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Verify locked
    assert!(client.is_active(&token_id));

    // Settle the NFT
    client.settle(&token_id);

    // Verify unlocked
    assert!(!client.is_active(&token_id));

    // Now transfer should succeed
    client.transfer(&owner, &new_owner, &token_id);
    assert_eq!(client.owner_of(&token_id), new_owner);
}

// ============================================
// Transfer Tests
// ============================================

#[test]
fn test_transfer_reactivated_nft_fails() {
    let (e, admin, client) = setup_env();
    let owner = Address::generate(&e);
    let new_owner = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Settle the NFT
    client.settle(&token_id);
    assert!(!client.is_active(&token_id));

    // Reactivate the NFT (admin only)
    client.activate(&token_id);
    assert!(client.is_active(&token_id));

    // Attempt to transfer should fail again
    let result = client.try_transfer(&owner, &new_owner, &token_id);
    assert_eq!(result, Err(Ok(ContractError::TokenLocked)));
}

// ============================================================================
// Transfer Tests - Invalid Transfers
// ============================================================================

#[test]
fn test_transfer_non_existent_token() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);
    let new_owner = Address::generate(&e);

    // Attempt to transfer a token that doesn't exist
    let result = client.try_transfer(&owner, &new_owner, &999u32);
    assert_eq!(result, Err(Ok(ContractError::TokenNotFound)));
}

#[test]
fn test_transfer_to_same_address() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Settle the NFT
    client.settle(&token_id);

    // Attempt to transfer to self should fail
    let result = client.try_transfer(&owner, &owner, &token_id);
    assert_eq!(result, Err(Ok(ContractError::InvalidRecipient)));
}

#[test]
fn test_transfer_not_initialized() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    let from = Address::generate(&e);
    let to = Address::generate(&e);

    // Transfer without initialization should fail
    let result = client.try_transfer(&from, &to, &1u32);
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

// ============================================================================
// Settle Tests
// ============================================================================

#[test]
fn test_settle() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Verify active
    assert!(client.is_active(&token_id));

    // Settle
    client.settle(&token_id);

    // Verify inactive
    assert!(!client.is_active(&token_id));
}

#[test]
fn test_settle_already_settled() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Settle once
    client.settle(&token_id);

    // Settle again (should be no-op, not error)
    client.settle(&token_id);

    // Still inactive
    assert!(!client.is_active(&token_id));
}

#[test]
fn test_settle_non_existent_token() {
    let (e, _admin, client) = setup_env();

    let result = client.try_settle(&999u32);
    assert_eq!(result, Err(Ok(ContractError::TokenNotFound)));
}

// ============================================================================
// Query Tests
// ============================================================================

#[test]
fn test_get_metadata() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Get metadata
    let metadata = client.get_metadata(&token_id);

    assert_eq!(metadata.duration_days, 30);
    assert_eq!(metadata.max_loss_percent, 10);
    assert_eq!(metadata.initial_amount, 1000);
}

// ============================================
// is_expired Tests
// ============================================

#[test]
fn test_get_metadata_non_existent() {
    let (_e, _admin, client) = setup_env();

    let result = client.try_get_metadata(&999u32);
    assert_eq!(result, Err(Ok(ContractError::TokenNotFound)));
}

#[test]
fn test_owner_of_non_existent() {
    let (_e, _admin, client) = setup_env();

    let result = client.try_owner_of(&999u32);
    assert_eq!(result, Err(Ok(ContractError::TokenNotFound)));
}

#[test]
fn test_is_active_non_existent() {
    let (_e, _admin, client) = setup_env();

    let result = client.try_is_active(&999u32);
    assert_eq!(result, Err(Ok(ContractError::TokenNotFound)));
}

#[test]
fn test_get_tokens_by_owner_empty() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);

    let tokens = client.get_tokens_by_owner(&owner);
    assert_eq!(tokens.len(), 0);
}

// ============================================================================
// Activate Tests (Admin Only)
// ============================================================================

#[test]
fn test_activate() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);

    // Mint and settle
    let token_id = mint_test_nft(&e, &client, &owner);
    client.settle(&token_id);
    assert!(!client.is_active(&token_id));

    // Reactivate
    client.activate(&token_id);
    assert!(client.is_active(&token_id));
}

#[test]
fn test_activate_non_existent() {
    let (_e, _admin, client) = setup_env();

    let result = client.try_activate(&999u32);
    assert_eq!(result, Err(Ok(ContractError::TokenNotFound)));
}

// ============================================================================
// Get NFT Data Tests
// ============================================================================

#[test]
fn test_get_nft_data() {
    let (e, _admin, client) = setup_env();
    let owner = Address::generate(&e);

    // Mint a token
    let token_id = mint_test_nft(&e, &client, &owner);

    // Get full NFT data
    let nft = client.get_nft_data(&token_id);

    assert_eq!(nft.owner, owner);
    assert_eq!(nft.token_id, token_id);
    assert!(nft.is_active);
    assert_eq!(nft.early_exit_penalty, 10);
}

#[test]
fn test_get_nft_data_non_existent() {
    let (_e, _admin, client) = setup_env();

    let result = client.try_get_nft_data(&999u32);
    assert_eq!(result, Err(Ok(ContractError::TokenNotFound)));
}
