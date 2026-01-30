#![cfg(test)]

extern crate std;

use crate::*;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger},
    vec, Address, Env, IntoVal, String,
};

fn setup_contract(e: &Env) -> (Address, CommitmentNFTContractClient<'_>) {
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(e, &contract_id);
    let admin = Address::generate(e);
    (admin, client)
}

fn create_test_metadata(
    e: &Env,
    asset_address: &Address,
) -> (String, u32, u32, String, i128, Address, u32) {
    (
        String::from_str(e, "commitment_001"),
        30, // duration_days
        10, // max_loss_percent
        String::from_str(e, "balanced"),
        1000, // initial_amount
        asset_address.clone(),
        5, // early_exit_penalty
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
    let (admin, contract_id) = {
        let (admin, client) = setup_contract(&e);

        // Initialize should succeed
        client.initialize(&admin);

        // Verify admin is set
        let stored_admin = client.get_admin();
        assert_eq!(stored_admin, admin);

        // Verify total supply is 0
        assert_eq!(client.total_supply(), 0);

        (admin, client.address)
    };

    (e, contract_id, admin)
}

// ============================================================================
// Initialization Tests
// ============================================================================

#[test]
#[should_panic(expected = "Error(Contract, #2)")] // AlreadyInitialized
fn test_initialize_twice_fails() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);
    client.initialize(&admin); // Should panic
}

// ============================================
// Mint Tests
// ============================================

#[test]
fn test_mint() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    assert_eq!(token_id, 0);
    assert_eq!(client.total_supply(), 1);
    assert_eq!(client.balance_of(&owner), 1);

    // Verify Mint event
    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, client.address);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("Mint").into_val(&e),
            token_id.into_val(&e),
            owner.into_val(&e)
        ]
    );
    let data: (String, u64) = last_event.2.into_val(&e);
    assert_eq!(data.0, commitment_id);
}

#[test]
fn test_mint_multiple() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint 3 NFTs
    let token_id_0 = client.mint(
        &owner,
        &String::from_str(&e, "commitment_0"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(token_id_0, 0);

    let token_id_1 = client.mint(
        &owner,
        &String::from_str(&e, "commitment_1"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(token_id_1, 1);

    let token_id_2 = client.mint(
        &owner,
        &String::from_str(&e, "commitment_2"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );
    assert_eq!(token_id_2, 2);

    assert_eq!(client.total_supply(), 3);
    assert_eq!(client.balance_of(&owner), 3);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")] // NotInitialized
fn test_mint_without_initialize_fails() {
    let e = Env::default();
    let (_admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    client.mint(
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );
}

// ============================================
// get_metadata Tests
// ============================================

#[test]
fn test_get_metadata() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let commitment_id = String::from_str(&e, "test_commitment");
    let duration = 30u32;
    let max_loss = 15u32;
    let commitment_type = String::from_str(&e, "aggressive");
    let amount = 5000i128;

    let token_id = client.mint(
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset_address,
        &10,
    );

    let nft = client.get_metadata(&token_id);

    assert_eq!(nft.metadata.commitment_id, commitment_id);
    assert_eq!(nft.metadata.duration_days, duration);
    assert_eq!(nft.metadata.max_loss_percent, max_loss);
    assert_eq!(nft.metadata.commitment_type, commitment_type);
    assert_eq!(nft.metadata.initial_amount, amount);
    assert_eq!(nft.metadata.asset_address, asset_address);
    assert_eq!(nft.owner, owner);
    assert_eq!(nft.token_id, token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_get_metadata_nonexistent_token() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    // Try to get metadata for non-existent token
    client.get_metadata(&999);
}

// ============================================
// owner_of Tests
// ============================================

#[test]
fn test_owner_of() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    let retrieved_owner = client.owner_of(&token_id);
    assert_eq!(retrieved_owner, owner);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_owner_of_nonexistent_token() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    client.owner_of(&999);
}

// ============================================
// is_active Tests
// ============================================

#[test]
fn test_is_active() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Newly minted NFT should be active
    assert_eq!(client.is_active(&token_id), true);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_is_active_nonexistent_token() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    client.is_active(&999);
}

// ============================================
// total_supply Tests
// ============================================

#[test]
fn test_total_supply_initial() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    assert_eq!(client.total_supply(), 0);
}

#[test]
fn test_total_supply_after_minting() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint 5 NFTs
    for _ in 0..5 {
        client.mint(
            &owner,
            &String::from_str(&e, "commitment"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    assert_eq!(client.total_supply(), 5);
}

// ============================================
// balance_of Tests
// ============================================

#[test]
fn test_balance_of_initial() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);

    client.initialize(&admin);

    // Owner with no NFTs should have balance 0
    assert_eq!(client.balance_of(&owner), 0);
}

#[test]
fn test_balance_of_after_minting() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint 3 NFTs for owner1
    for _ in 0..3 {
        client.mint(
            &owner1,
            &String::from_str(&e, "owner1_commitment"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    // Mint 2 NFTs for owner2
    for _ in 0..2 {
        client.mint(
            &owner2,
            &String::from_str(&e, "owner2_commitment"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    assert_eq!(client.balance_of(&owner1), 3);
    assert_eq!(client.balance_of(&owner2), 2);
}

// ============================================
// get_all_metadata Tests
// ============================================

#[test]
fn test_get_all_metadata_empty() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    let all_nfts = client.get_all_metadata();
    assert_eq!(all_nfts.len(), 0);
}

#[test]
fn test_get_all_metadata() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint 3 NFTs
    for _ in 0..3 {
        client.mint(
            &owner,
            &String::from_str(&e, "commitment"),
            &30,
            &10,
            &String::from_str(&e, "balanced"),
            &1000,
            &asset_address,
            &5,
        );
    }

    let all_nfts = client.get_all_metadata();
    assert_eq!(all_nfts.len(), 3);

    // Verify each NFT owner
    for nft in all_nfts.iter() {
        assert_eq!(nft.owner, owner);
    }
}

// ============================================
// get_nfts_by_owner Tests
// ============================================

#[test]
fn test_get_nfts_by_owner_empty() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);

    client.initialize(&admin);

    let nfts = client.get_nfts_by_owner(&owner);
    assert_eq!(nfts.len(), 0);
}

#[test]
fn test_get_nfts_by_owner() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint 2 NFTs for owner1
    for _ in 0..2 {
        client.mint(
            &owner1,
            &String::from_str(&e, "owner1"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    // Mint 3 NFTs for owner2
    for _ in 0..3 {
        client.mint(
            &owner2,
            &String::from_str(&e, "owner2"),
            &30,
            &10,
            &String::from_str(&e, "safe"),
            &1000,
            &asset_address,
            &5,
        );
    }

    let owner1_nfts = client.get_nfts_by_owner(&owner1);
    let owner2_nfts = client.get_nfts_by_owner(&owner2);

    assert_eq!(owner1_nfts.len(), 2);
    assert_eq!(owner2_nfts.len(), 3);

    // Verify all owner1 NFTs belong to owner1
    for nft in owner1_nfts.iter() {
        assert_eq!(nft.owner, owner1);
    }
}

// ============================================
// Transfer Tests
// ============================================

#[test]
fn test_owner_of_not_found() {
    let (e, contract_id, _admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    let result = client.try_owner_of(&999);
    assert!(result.is_err());
}

// ============================================================================
// Transfer Tests
// ============================================================================

#[test]
fn test_transfer() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &owner1,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Verify initial state
    assert_eq!(client.owner_of(&token_id), owner1);
    assert_eq!(client.balance_of(&owner1), 1);
    assert_eq!(client.balance_of(&owner2), 0);

    // Transfer NFT
    client.transfer(&owner1, &owner2, &token_id);

    // Verify transfer
    assert_eq!(client.owner_of(&token_id), owner2);
    assert_eq!(client.balance_of(&owner1), 0);
    assert_eq!(client.balance_of(&owner2), 1);

    // Verify Transfer event
    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, client.address);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("Transfer").into_val(&e),
            owner1.into_val(&e),
            owner2.into_val(&e)
        ]
    );
    let data: (u32, u64) = last_event.2.into_val(&e);
    assert_eq!(data.0, token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")] // NotOwner
fn test_transfer_not_owner() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let not_owner = Address::generate(&e);
    let recipient = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Try to transfer from non-owner (should fail)
    client.transfer(&not_owner, &recipient, &token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_transfer_nonexistent_token() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let recipient = Address::generate(&e);

    client.initialize(&admin);

    client.transfer(&owner, &recipient, &999);
}

// ============================================
// Settle Tests
// ============================================

#[test]
fn test_settle() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint with 1 day duration
    let token_id = client.mint(
        &owner,
        &String::from_str(&e, "test_commitment"),
        &1, // 1 day duration
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // NFT should be active initially
    assert_eq!(client.is_active(&token_id), true);

    // Fast forward time past expiration (2 days = 172800 seconds)
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    // Verify it's expired
    assert_eq!(client.is_expired(&token_id), true);

    // Settle the NFT
    client.settle(&token_id);

    // NFT should now be inactive
    assert_eq!(client.is_active(&token_id), false);

    // Verify Settle event
    let events = e.events().all();
    let last_event = events.last().unwrap();

    assert_eq!(last_event.0, client.address);
    assert_eq!(
        last_event.1,
        vec![
            &e,
            symbol_short!("Settle").into_val(&e),
            token_id.into_val(&e)
        ]
    );
    let data: u64 = last_event.2.into_val(&e);
    assert_eq!(data, e.ledger().timestamp());
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")] // NotExpired
fn test_settle_not_expired() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &owner,
        &String::from_str(&e, "test_commitment"),
        &30, // 30 days duration
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Try to settle before expiration (should fail)
    client.settle(&token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")] // AlreadySettled
fn test_settle_already_settled() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &owner,
        &String::from_str(&e, "test_commitment"),
        &1,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Fast forward time
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    client.settle(&token_id);
    client.settle(&token_id); // Should fail
}

// ============================================
// is_expired Tests
// ============================================

#[test]
fn test_is_expired() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &owner,
        &String::from_str(&e, "test_commitment"),
        &1, // 1 day
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    // Should not be expired initially
    assert_eq!(client.is_expired(&token_id), false);

    // Fast forward 2 days
    e.ledger().with_mut(|li| {
        li.timestamp = 172800;
    });

    // Should now be expired
    assert_eq!(client.is_expired(&token_id), true);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")] // TokenNotFound
fn test_is_expired_nonexistent_token() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    client.is_expired(&999);
}

// ============================================
// token_exists Tests
// ============================================

#[test]
fn test_token_exists() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Token 0 should not exist yet
    assert_eq!(client.token_exists(&0), false);

    let (commitment_id, duration, max_loss, commitment_type, amount, asset, penalty) =
        create_test_metadata(&e, &asset_address);

    let token_id = client.mint(
        &owner,
        &commitment_id,
        &duration,
        &max_loss,
        &commitment_type,
        &amount,
        &asset,
        &penalty,
    );

    // Token should now exist
    assert_eq!(client.token_exists(&token_id), true);

    // Non-existent token should return false
    assert_eq!(client.token_exists(&999), false);
}

// ============================================
// get_admin Tests
// ============================================

#[test]
fn test_get_admin() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")] // NotInitialized
fn test_get_admin_not_initialized() {
    let e = Env::default();
    let (_admin, client) = setup_contract(&e);

    client.get_admin();
}

// ============================================
// Edge Cases
// ============================================

#[test]
fn test_metadata_timestamps() {
    let e = Env::default();

    // Set initial ledger timestamp
    e.ledger().with_mut(|li| {
        li.timestamp = 1000;
    });

    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &owner,
        &String::from_str(&e, "test"),
        &30, // 30 days
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    let metadata = client.get_metadata(&token_id);

    // Verify timestamps
    assert_eq!(metadata.metadata.created_at, 1000);
    // expires_at should be created_at + (30 days * 86400 seconds)
    assert_eq!(metadata.metadata.expires_at, 1000 + (30 * 86400));
}

#[test]
fn test_balance_updates_after_transfer() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, client) = setup_contract(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint multiple NFTs for owner1
    client.mint(
        &owner1,
        &String::from_str(&e, "commitment_0"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    client.mint(
        &owner1,
        &String::from_str(&e, "commitment_1"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );
    client.mint(
        &owner1,
        &String::from_str(&e, "commitment_2"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset_address,
        &5,
    );

    assert_eq!(client.balance_of(&owner1), 3);
    assert_eq!(client.balance_of(&owner2), 0);

    // Transfer one NFT
    client.transfer(&owner1, &owner2, &0);

    assert_eq!(client.balance_of(&owner1), 2);
    assert_eq!(client.balance_of(&owner2), 1);

    // Transfer another
    client.transfer(&owner1, &owner2, &1);

    assert_eq!(client.balance_of(&owner1), 1);
    assert_eq!(client.balance_of(&owner2), 2);

    // Verify get_nfts_by_owner reflects the transfers
    let owner1_nfts = client.get_nfts_by_owner(&owner1);
    let owner2_nfts = client.get_nfts_by_owner(&owner2);

    assert_eq!(owner1_nfts.len(), 1);
    assert_eq!(owner2_nfts.len(), 2);
}
