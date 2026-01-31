// Integration tests for cross-contract interactions
// This file contains tests that verify interactions between Commitment NFT, Commitment Core, Attestation Engine, and Price Oracle contracts

#![cfg(test)]

use commitment_core::{CommitmentCoreContract, CommitmentCoreContractClient, CommitmentRules};
use commitment_nft::{CommitmentNFTContract, CommitmentNFTContractClient};
use attestation_engine::{AttestationEngineContract, AttestationEngineContractClient};
use price_oracle::{PriceOracleContract, PriceOracleContractClient};
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, String, Map};

pub struct IntegrationTestFixture {
    pub env: Env,
    pub admin: Address,
    pub owner: Address,
    pub user1: Address,
    pub verifier: Address,
    pub nft_client: CommitmentNFTContractClient<'static>,
    pub core_client: CommitmentCoreContractClient<'static>,
    pub attestation_client: AttestationEngineContractClient<'static>,
    pub asset_address: Address,
}

impl IntegrationTestFixture {
    pub fn setup() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let user1 = Address::generate(&env);
        let verifier = Address::generate(&env);
        let asset_address = Address::generate(&env);

        // Deploy NFT contract
        let nft_contract_id = env.register_contract(None, CommitmentNFTContract);
        let nft_client = CommitmentNFTContractClient::new(&env, &nft_contract_id);
        nft_client.initialize(&admin);

        // Deploy Core contract
        let core_contract_id = env.register_contract(None, CommitmentCoreContract);
        let core_client = CommitmentCoreContractClient::new(&env, &core_contract_id);
        core_client.initialize(&admin, &nft_contract_id);

        // Deploy Attestation Engine contract
        let attestation_contract_id = env.register_contract(None, AttestationEngineContract);
        let attestation_client = AttestationEngineContractClient::new(&env, &attestation_contract_id);
        attestation_client.initialize(&admin, &core_contract_id);

        IntegrationTestFixture {
            env,
            admin,
            owner,
            user1,
            verifier,
            nft_client,
            core_client,
            attestation_client,
            asset_address,
        }
    }

    pub fn create_test_rules(&self) -> CommitmentRules {
        CommitmentRules {
            duration_days: 30,
            max_loss_percent: 10,
            commitment_type: String::from_str(&self.env, "safe"),
            early_exit_penalty: 5,
            min_fee_threshold: 100_0000000,
            grace_period_days: 3,
        }
    }
}

// ============================================
// Cross-Contract Integration Tests
// ============================================

#[test]
#[ignore] // Requires token contract setup
fn test_create_commitment_with_attestation_flow() {
    let fixture = IntegrationTestFixture::setup();

    let rules = fixture.create_test_rules();

    // Step 1: Create commitment in core contract - returns String commitment_id
    let commitment_id: String = fixture.core_client.create_commitment(
        &fixture.owner,
        &1000_0000000,
        &fixture.asset_address,
        &rules,
    );

    let commitment = fixture.core_client.get_commitment(&commitment_id);
    assert_eq!(commitment.owner, fixture.owner);
    assert_eq!(commitment.amount, 1000_0000000);
    assert_eq!(commitment.status, String::from_str(&fixture.env, "active"));

    // Step 2: Record attestation for the commitment
    let mut data = Map::new(&fixture.env);
    data.set(
        String::from_str(&fixture.env, "initial_value"),
        String::from_str(&fixture.env, "1000"),
    );

    fixture.attestation_client.attest(
        &fixture.verifier,
        &commitment_id,
        &String::from_str(&fixture.env, "health_check"),
        &data,
        &true,
    );

    // Verify attestation was recorded
    let attestations = fixture.attestation_client.get_attestations(&commitment_id);
    assert_eq!(attestations.len(), 1);
}

#[test]
#[ignore] // Requires token contract setup
fn test_commitment_value_update_with_health_tracking() {
    let fixture = IntegrationTestFixture::setup();

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&fixture.env, "balanced"),
        early_exit_penalty: 5,
        min_fee_threshold: 100_0000000,
        grace_period_days: 3,
    };

    // Create commitment
    let commitment_id: String = fixture.core_client.create_commitment(
        &fixture.owner,
        &1000_0000000,
        &fixture.asset_address,
        &rules,
    );

    // Update value in core contract
    fixture.core_client.update_value(&commitment_id, &1050_0000000);

    // Record health metrics in attestation engine
    fixture.attestation_client.record_fees(&fixture.admin, &commitment_id, &50_0000000);
    fixture.attestation_client.record_drawdown(&fixture.admin, &commitment_id, &0);

    // Verify metrics
    let metrics = fixture.attestation_client.get_health_metrics(&commitment_id);
    assert_eq!(metrics.fees_generated, 50_0000000);

    // Verify commitment status
    let commitment = fixture.core_client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, 1050_0000000);
    assert_eq!(commitment.status, String::from_str(&fixture.env, "active"));
}

#[test]
#[ignore] // Requires token contract setup
fn test_settlement_flow_end_to_end() {
    let fixture = IntegrationTestFixture::setup();

    let rules = fixture.create_test_rules();

    // Create commitment
    let commitment_id: String = fixture.core_client.create_commitment(
        &fixture.owner,
        &1000_0000000,
        &fixture.asset_address,
        &rules,
    );

    // Record some fees
    fixture.attestation_client.record_fees(&fixture.admin, &commitment_id, &100_0000000);

    // Fast forward past expiration
    let commitment = fixture.core_client.get_commitment(&commitment_id);
    fixture.env.ledger().with_mut(|li| {
        li.timestamp = commitment.expires_at + 1;
    });

    // Settle commitment
    fixture.core_client.settle(&commitment_id);

    // Verify commitment is settled
    let settled_commitment = fixture.core_client.get_commitment(&commitment_id);
    assert_eq!(settled_commitment.status, String::from_str(&fixture.env, "settled"));
}

#[test]
#[ignore] // Requires token contract setup
fn test_early_exit_flow_end_to_end() {
    let fixture = IntegrationTestFixture::setup();

    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(&fixture.env, "aggressive"),
        early_exit_penalty: 10,
        min_fee_threshold: 100_0000000,
        grace_period_days: 3,
    };

    // Create commitment
    let commitment_id: String = fixture.core_client.create_commitment(
        &fixture.owner,
        &1000_0000000,
        &fixture.asset_address,
        &rules,
    );

    // Update value
    fixture.core_client.update_value(&commitment_id, &1100_0000000);

    // Record attestation for early exit
    let mut data = Map::new(&fixture.env);
    data.set(
        String::from_str(&fixture.env, "reason"),
        String::from_str(&fixture.env, "user_request"),
    );

    fixture.attestation_client.attest(
        &fixture.verifier,
        &commitment_id,
        &String::from_str(&fixture.env, "health_check"),
        &data,
        &true,
    );

    // Perform early exit
    fixture.core_client.early_exit(&commitment_id, &fixture.owner);

    // Verify commitment is marked as early exit
    let commitment = fixture.core_client.get_commitment(&commitment_id);
    assert_eq!(commitment.status, String::from_str(&fixture.env, "early_exit"));
}

#[test]
#[ignore] // Requires token contract setup
fn test_compliance_verification_flow() {
    let fixture = IntegrationTestFixture::setup();

    let rules = fixture.create_test_rules();

    // Create commitment
    let commitment_id: String = fixture.core_client.create_commitment(
        &fixture.owner,
        &1000_0000000,
        &fixture.asset_address,
        &rules,
    );

    // Record fees and attest - commitment in good standing
    fixture.attestation_client.record_fees(&fixture.admin, &commitment_id, &100_0000000);

    let mut data = Map::new(&fixture.env);
    data.set(
        String::from_str(&fixture.env, "status"),
        String::from_str(&fixture.env, "healthy"),
    );

    fixture.attestation_client.attest(
        &fixture.verifier,
        &commitment_id,
        &String::from_str(&fixture.env, "health_check"),
        &data,
        &true,
    );

    // Verify compliance
    let is_compliant = fixture.attestation_client.verify_compliance(&commitment_id);
    assert!(is_compliant);

    // Calculate compliance score
    let score = fixture.attestation_client.calculate_compliance_score(&commitment_id);
    assert!(score > 0);
}

// ============================================
// Gas Optimization Tests
// ============================================

#[test]
#[ignore] // Requires token contract setup
fn test_gas_single_commitment_creation() {
    let fixture = IntegrationTestFixture::setup();

    let rules = fixture.create_test_rules();

    // Single commitment creation
    let _commitment_id: String = fixture.core_client.create_commitment(
        &fixture.owner,
        &1000_0000000,
        &fixture.asset_address,
        &rules,
    );
}

#[test]
#[ignore] // Requires token contract setup
fn test_gas_multiple_operations() {
    let fixture = IntegrationTestFixture::setup();

    let rules = fixture.create_test_rules();

    // Create commitment
    let commitment_id: String = fixture.core_client.create_commitment(
        &fixture.owner,
        &1000_0000000,
        &fixture.asset_address,
        &rules,
    );

    // Multiple update operations
    fixture.core_client.update_value(&commitment_id, &1010_0000000);
    fixture.core_client.update_value(&commitment_id, &1020_0000000);
    fixture.core_client.update_value(&commitment_id, &1030_0000000);

    // Multiple attestation operations
    fixture.attestation_client.record_fees(&fixture.admin, &commitment_id, &10_0000000);
    fixture.attestation_client.record_fees(&fixture.admin, &commitment_id, &20_0000000);
    fixture.attestation_client.record_fees(&fixture.admin, &commitment_id, &30_0000000);

    // Verify final state
    let commitment = fixture.core_client.get_commitment(&commitment_id);
    assert_eq!(commitment.current_value, 1030_0000000);

    let metrics = fixture.attestation_client.get_health_metrics(&commitment_id);
    assert_eq!(metrics.fees_generated, 60_0000000);
}

#[test]
#[ignore] // Requires token contract setup
fn test_gas_batch_attestations() {
    let fixture = IntegrationTestFixture::setup();

    let rules = fixture.create_test_rules();

    // Create commitment
    let commitment_id: String = fixture.core_client.create_commitment(
        &fixture.owner,
        &1000_0000000,
        &fixture.asset_address,
        &rules,
    );

    // Multiple attestations
    let check_numbers = ["1", "2", "3", "4", "5"];
    for check_num in check_numbers.iter() {
        let mut data = Map::new(&fixture.env);
        data.set(
            String::from_str(&fixture.env, "check_number"),
            String::from_str(&fixture.env, check_num),
        );

        fixture.attestation_client.attest(
            &fixture.verifier,
            &commitment_id,
            &String::from_str(&fixture.env, "health_check"),
            &data,
            &true,
        );
    }

    // Verify all attestations recorded
    let attestations = fixture.attestation_client.get_attestations(&commitment_id);
    assert_eq!(attestations.len(), 5);
}

// ============================================
// Oracle Integration Tests
// ============================================

#[test]
fn test_oracle_integration_price_feed() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle_feeder = Address::generate(&env);
    let asset = Address::generate(&env);

    let oracle_id = env.register_contract(None, PriceOracleContract);
    let oracle_client = PriceOracleContractClient::new(&env, &oracle_id);

    oracle_client.initialize(&admin);
    oracle_client.add_oracle(&admin, &oracle_feeder);

    oracle_client.set_price(&oracle_feeder, &asset, &1_500_000000, &8);
    let data = oracle_client.get_price(&asset);
    assert_eq!(data.price, 1_500_000000);
    assert_eq!(data.decimals, 8);
}

#[test]
fn test_oracle_integration_validation_and_staleness() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle_feeder = Address::generate(&env);
    let asset = Address::generate(&env);

    let oracle_id = env.register_contract(None, PriceOracleContract);
    let oracle_client = PriceOracleContractClient::new(&env, &oracle_id);

    oracle_client.initialize(&admin);
    oracle_client.add_oracle(&admin, &oracle_feeder);
    oracle_client.set_max_staleness(&admin, &300);

    oracle_client.set_price(&oracle_feeder, &asset, &2_000_000000, &8);
    let data = oracle_client.get_price_valid(&asset, &None);
    assert_eq!(data.price, 2_000_000000);

    env.ledger().with_mut(|li| {
        li.timestamp += 400;
    });
    let _ = oracle_client.get_price_valid(&asset, &Some(500));
}

#[test]
fn test_oracle_whitelist_only_can_set_price() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let oracle_feeder = Address::generate(&env);
    let asset = Address::generate(&env);

    let oracle_id = env.register_contract(None, PriceOracleContract);
    let oracle_client = PriceOracleContractClient::new(&env, &oracle_id);

    oracle_client.initialize(&admin);
    oracle_client.add_oracle(&admin, &oracle_feeder);

    assert!(oracle_client.is_oracle_whitelisted(&oracle_feeder));
    oracle_client.set_price(&oracle_feeder, &asset, &100, &6);

    oracle_client.remove_oracle(&admin, &oracle_feeder);
    assert!(!oracle_client.is_oracle_whitelisted(&oracle_feeder));
}
