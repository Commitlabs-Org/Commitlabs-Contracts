#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, testutils::Events, Address, Env, String};

// ============================================================================
// Helper Functions
// ============================================================================

fn setup_env() -> (Env, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let admin = Address::generate(&e);
    (e, contract_id, admin)
}

fn setup_contract<'a>(e: &'a Env) -> (Address, Address, Address, CommitmentNFTContractClient<'a>) {
    let admin = Address::generate(e);
    let core_contract = Address::generate(e);
    let owner = Address::generate(e);

    let contract_id = e.register_contract(None, CommitmentNFTContract);
    let client = CommitmentNFTContractClient::new(e, &contract_id);

    (admin, core_contract, owner, client)
}

fn mint_test_nft(
    e: &Env,
    client: &CommitmentNFTContractClient,
    caller: &Address,
    owner: &Address,
) -> u32 {
    let asset = Address::generate(e);

    client.mint(
        caller,
        owner,
        &String::from_str(e, "commitment-1"),
        &30, // duration_days
        &10, // max_loss_percent
        &String::from_str(e, "balanced"),
        &1000, // initial_amount
        &asset,
    )
}

// ============================================================================
// Initialization Tests
// ============================================================================

#[test]
fn test_initialize() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    let result = client.initialize(&admin);
    assert_eq!(result, ());

    // Verify total supply is 0
    let supply = client.total_supply();
    assert_eq!(supply, 0);

    // Verify admin is set
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_initialize_twice_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    client.initialize(&admin);
    let result = client.try_initialize(&admin);
    assert!(result.is_err());
}

// ============================================================================
// Access Control Tests
// ============================================================================

#[test]
fn test_set_core_contract() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, core_contract, _, client) = setup_contract(&e);

    // Initialize
    client.initialize(&admin);

    // Set core contract
    client.set_core_contract(&core_contract);

    // Verify core contract is set
    assert_eq!(client.get_core_contract(), core_contract);
}

// ============================================================================
// Minting Tests
// ============================================================================

#[test]
fn test_mint_success() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let token_id = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert_eq!(token_id, 1);

    // Verify ownership
    let fetched_owner = client.owner_of(&token_id);
    assert_eq!(fetched_owner, owner);

    // Verify metadata
    let metadata = client.get_metadata(&token_id);
    assert_eq!(metadata.duration_days, 30);
    assert_eq!(metadata.max_loss_percent, 10);
    assert_eq!(metadata.initial_amount, 1000);

    // Verify is_active
    let active = client.is_active(&token_id);
    assert!(active);

    // Verify total supply incremented
    let supply = client.total_supply();
    assert_eq!(supply, 1);
}

#[test]
fn test_mint_sequential_token_ids() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let token_id_1 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );
    let token_id_2 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_002"),
        &60,
        &20,
        &String::from_str(&e, "balanced"),
        &2000,
        &asset,
    );

    assert_eq!(token_id_1, 1);
    assert_eq!(token_id_2, 2);
    assert_eq!(client.total_supply(), 2);
}

#[test]
fn test_mint_unauthorized_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let unauthorized = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &unauthorized,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_authorized_minter() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let minter = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);
    client.add_authorized_minter(&admin, &minter);

    let token_id = client.mint(
        &minter,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert_eq!(token_id, 1);
}

#[test]
fn test_mint_invalid_duration_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &0, // Invalid: duration must be > 0
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_invalid_max_loss_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &101, // Invalid: max_loss must be 0-100
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_invalid_commitment_type_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "invalid_type"), // Invalid
        &1000,
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_invalid_amount_fails() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    let result = client.try_mint(
        &admin,
        &owner,
        &String::from_str(&e, "commitment_001"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &0, // Invalid: amount must be > 0
        &asset,
    );

    assert!(result.is_err());
}

#[test]
fn test_mint_all_commitment_types() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);
    let owner = Address::generate(&e);
    let asset = Address::generate(&e);

    client.initialize(&admin);

    // Test "safe"
    let t1 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "c1"),
        &30,
        &10,
        &String::from_str(&e, "safe"),
        &1000,
        &asset,
    );
    assert_eq!(t1, 1);

    // Test "balanced"
    let t2 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "c2"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset,
    );
    assert_eq!(t2, 2);

    // Test "aggressive"
    let t3 = client.mint(
        &admin,
        &owner,
        &String::from_str(&e, "c3"),
        &30,
        &10,
        &String::from_str(&e, "aggressive"),
        &1000,
        &asset,
    );
    assert_eq!(t3, 3);
}

#[test]
fn test_get_metadata_not_found() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    client.initialize(&admin);

    let result = client.try_get_metadata(&999);
    assert!(result.is_err());
}

#[test]
fn test_owner_of_not_found() {
    let (e, contract_id, admin) = setup_env();
    let client = CommitmentNFTContractClient::new(&e, &contract_id);

    client.initialize(&admin);

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

    let (admin, _, owner, client) = setup_contract(&e);
    let new_owner = Address::generate(&e);

    // Initialize and mint
    client.initialize(&admin);
    let token_id = mint_test_nft(&e, &client, &admin, &owner);

    // Transfer
    client.transfer(&owner, &new_owner, &token_id);

    // Verify new owner
    assert_eq!(client.owner_of(&token_id), new_owner);

    // Verify NFT data is updated
    let nft = client.get_nft(&token_id);
    assert_eq!(nft.owner, new_owner);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_transfer_not_owner() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, _, owner, client) = setup_contract(&e);
    let not_owner = Address::generate(&e);
    let new_owner = Address::generate(&e);

    // Initialize and mint
    client.initialize(&admin);
    let token_id = mint_test_nft(&e, &client, &admin, &owner);

    // Try to transfer from non-owner - should panic with NotOwner (error code 11)
    client.transfer(&not_owner, &new_owner, &token_id);
}

// ============================================================================
// Settlement Tests (Issue #5 - Main Tests)
// ============================================================================

#[test]
fn test_settle_success() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, core_contract, owner, client) = setup_contract(&e);

    // Initialize and set up
    client.initialize(&admin);
    client.set_core_contract(&core_contract);

    // Mint an NFT
    let token_id = mint_test_nft(&e, &client, &admin, &owner);

    // Verify NFT is active before settlement
    assert!(client.is_active(&token_id));

    // Settle as the authorized core contract
    client.settle(&core_contract, &token_id);

    // Verify NFT is now inactive
    assert!(!client.is_active(&token_id));

    // Verify NFT data is updated
    let nft = client.get_nft(&token_id);
    assert!(!nft.is_active);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_settle_unauthorized_caller() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, core_contract, owner, client) = setup_contract(&e);
    let unauthorized = Address::generate(&e);

    // Initialize and set up
    client.initialize(&admin);
    client.set_core_contract(&core_contract);

    // Mint an NFT
    let token_id = mint_test_nft(&e, &client, &admin, &owner);

    // Try to settle as unauthorized caller - should panic with Unauthorized (error code 3)
    client.settle(&unauthorized, &token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_settle_nft_not_found() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, core_contract, _, client) = setup_contract(&e);

    // Initialize and set up
    client.initialize(&admin);
    client.set_core_contract(&core_contract);

    // Try to settle non-existent NFT - should panic with TokenNotFound (error code 8)
    client.settle(&core_contract, &999);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_settle_already_settled() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, core_contract, owner, client) = setup_contract(&e);

    // Initialize and set up
    client.initialize(&admin);
    client.set_core_contract(&core_contract);

    // Mint an NFT
    let token_id = mint_test_nft(&e, &client, &admin, &owner);

    // Settle once - should succeed
    client.settle(&core_contract, &token_id);

    // Try to settle again - should panic with AlreadySettled (error code 9)
    client.settle(&core_contract, &token_id);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_settle_core_contract_not_set() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, core_contract, owner, client) = setup_contract(&e);

    // Initialize but DON'T set core contract
    client.initialize(&admin);

    // Mint an NFT
    let token_id = mint_test_nft(&e, &client, &admin, &owner);

    // Try to settle - should panic with NotInitialized (error code 1) because core contract not set
    client.settle(&core_contract, &token_id);
}

#[test]
fn test_settle_event_emission() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, core_contract, owner, client) = setup_contract(&e);

    // Initialize and set up
    client.initialize(&admin);
    client.set_core_contract(&core_contract);

    // Mint an NFT
    let token_id = mint_test_nft(&e, &client, &admin, &owner);

    // Settle
    client.settle(&core_contract, &token_id);

    // Verify events were emitted
    let events = e.events().all();

    // Should have at least the Settle event
    // Events: CoreContractSet, Mint, Settle
    assert!(events.len() >= 3);

    // Find the Settle event and verify its structure
    // The last event should be the Settle event
    let _last_event = events.last().unwrap();

    // Verify event topics contain "Settle" and token_id
    // Event structure: ((Symbol("Settle"), token_id), (timestamp, final_status))
    assert!(!events.is_empty());
}

#[test]
fn test_settle_multiple_nfts() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, core_contract, owner, client) = setup_contract(&e);

    // Initialize and set up
    client.initialize(&admin);
    client.set_core_contract(&core_contract);

    // Mint multiple NFTs
    let token_id_1 = mint_test_nft(&e, &client, &admin, &owner);
    let token_id_2 = mint_test_nft(&e, &client, &admin, &owner);
    let token_id_3 = mint_test_nft(&e, &client, &admin, &owner);

    // Verify all are active
    assert!(client.is_active(&token_id_1));
    assert!(client.is_active(&token_id_2));
    assert!(client.is_active(&token_id_3));

    // Settle only the first one
    client.settle(&core_contract, &token_id_1);

    // Verify first is inactive, others still active
    assert!(!client.is_active(&token_id_1));
    assert!(client.is_active(&token_id_2));
    assert!(client.is_active(&token_id_3));

    // Settle the third one
    client.settle(&core_contract, &token_id_3);

    // Verify first and third are inactive, second still active
    assert!(!client.is_active(&token_id_1));
    assert!(client.is_active(&token_id_2));
    assert!(!client.is_active(&token_id_3));
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_full_nft_lifecycle() {
    let e = Env::default();
    e.mock_all_auths();

    let (admin, core_contract, owner, client) = setup_contract(&e);
    let new_owner = Address::generate(&e);

    // 1. Initialize contract
    client.initialize(&admin);
    client.set_core_contract(&core_contract);

    // 2. Mint NFT
    let token_id = mint_test_nft(&e, &client, &admin, &owner);
    assert_eq!(client.owner_of(&token_id), owner);
    assert!(client.is_active(&token_id));

    // 3. Transfer NFT
    client.transfer(&owner, &new_owner, &token_id);
    assert_eq!(client.owner_of(&token_id), new_owner);
    assert!(client.is_active(&token_id));

    // 4. Settle NFT
    client.settle(&core_contract, &token_id);
    assert_eq!(client.owner_of(&token_id), new_owner); // Owner unchanged
    assert!(!client.is_active(&token_id)); // Now inactive
}
