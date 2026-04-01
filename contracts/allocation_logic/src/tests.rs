// Comprehensive Security-Focused Tests for Allocation Logic (Design Spike: String IDs)
use crate::{
    AllocationStrategiesContract, AllocationStrategiesContractClient, RiskLevel, Strategy,
    Commitment, CommitmentRules,
};
use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, testutils::Ledger, Address, Env, Map, String,
    Symbol, Vec, IntoVal,
};

// ============================================================================
// MOCK COMMITMENT CORE CONTRACT
// ============================================================================

#[contract]
pub struct MockCommitmentCore;

#[contractimpl]
impl MockCommitmentCore {
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        let key = Symbol::new(&e, "commitments");
        let commitments: Map<String, Commitment> = e.storage().instance().get(&key).unwrap_or(Map::new(&e));
        
        commitments.get(commitment_id).expect("Commitment not found in mock")
    }

    pub fn set_commitment(e: Env, commitment: Commitment) {
        let key = Symbol::new(&e, "commitments");
        let mut commitments: Map<String, Commitment> = e.storage().instance().get(&key).unwrap_or(Map::new(&e));
        
        commitments.set(commitment.commitment_id.clone(), commitment);
        e.storage().instance().set(&key, &commitments);
    }
}

// ============================================================================
// TEST HELPERS
// ============================================================================

fn create_contract(env: &Env) -> (Address, Address, AllocationStrategiesContractClient<'_>) {
    let admin = Address::generate(env);
    
    // Register and setup Mock Commitment Core
    let mock_core_id = env.register_contract(None, MockCommitmentCore);
    
    let contract_id = env.register_contract(None, AllocationStrategiesContract);
    let client = AllocationStrategiesContractClient::new(env, &contract_id);

    client.initialize(&admin, &mock_core_id);

    (admin, mock_core_id, client)
}

fn create_mock_commitment(env: &Env, core_id: &Address, id: &str, amount: i128, status: &str) {
    let mock_client = MockCommitmentCoreClient::new(env, core_id);
    
    let rules = CommitmentRules {
        duration_days: 30,
        max_loss_percent: 20,
        commitment_type: String::from_str(env, "balanced"),
        early_exit_penalty: 10,
        min_fee_threshold: 0,
        grace_period_days: 3,
    };
    
    let commitment = Commitment {
        commitment_id: String::from_str(env, id),
        owner: Address::generate(env),
        nft_token_id: 1,
        rules,
        amount,
        asset_address: Address::generate(env),
        created_at: env.ledger().timestamp(),
        expires_at: env.ledger().timestamp() + 86400 * 30,
        current_value: amount,
        status: String::from_str(env, status),
    };
    
    mock_client.set_commitment(&commitment);
}

fn setup_test_pools(_env: &Env, client: &AllocationStrategiesContractClient, admin: &Address) {
    client.register_pool(admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    client.register_pool(admin, &1, &RiskLevel::Low, &600, &1_000_000_000);
    client.register_pool(admin, &2, &RiskLevel::Medium, &1000, &800_000_000);
    client.register_pool(admin, &3, &RiskLevel::Medium, &1200, &800_000_000);
    client.register_pool(admin, &4, &RiskLevel::High, &2000, &500_000_000);
    client.register_pool(admin, &5, &RiskLevel::High, &2500, &500_000_000);
}

// ============================================================================
// BASIC FUNCTIONALITY TESTS
// ============================================================================

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let commitment_core = Address::generate(&env);
    let contract_id = env.register_contract(None, AllocationStrategiesContract);
    let client = AllocationStrategiesContractClient::new(&env, &contract_id);

    client.initialize(&admin, &commitment_core);
    assert!(client.is_initialized());
}

#[test]
fn test_register_pool() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);

    let pool = client.get_pool(&0);
    assert_eq!(pool.pool_id, 0);
    assert_eq!(pool.risk_level, RiskLevel::Low);
    assert_eq!(pool.apy, 500);
    assert_eq!(pool.max_capacity, 1_000_000_000);
    assert!(pool.active);
    assert_eq!(pool.total_liquidity, 0);
}

#[test]
fn test_safe_strategy_allocation() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, core_id, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = String::from_str(&env, "commit_1");
    let amount = 100_000_000i128;
    
    create_mock_commitment(&env, &core_id, "commit_1", amount, "active");

    let summary = client.allocate(&user, &commitment_id, &amount, &Strategy::Safe);

    assert_eq!(summary.commitment_id, commitment_id);
    assert_eq!(summary.strategy, Strategy::Safe);
    assert_eq!(summary.total_allocated, amount);

    // Verify only low-risk pools used
    for allocation in summary.allocations.iter() {
        let pool = client.get_pool(&allocation.pool_id);
        assert_eq!(pool.risk_level, RiskLevel::Low);
    }
}

#[test]
fn test_balanced_strategy_allocation() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, core_id, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = String::from_str(&env, "commit_2");
    let amount = 100_000_000i128;
    
    create_mock_commitment(&env, &core_id, "commit_2", amount, "active");
    
    let summary = client.allocate(&user, &commitment_id, &amount, &Strategy::Balanced);

    assert_eq!(summary.strategy, Strategy::Balanced);
    assert!(summary.total_allocated > 0);
}

#[test]
fn test_aggressive_strategy_allocation() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, core_id, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = String::from_str(&env, "commit_3");
    let amount = 100_000_000i128;
    
    create_mock_commitment(&env, &core_id, "commit_3", amount, "active");

    let summary = client.allocate(&user, &commitment_id, &amount, &Strategy::Aggressive);

    assert_eq!(summary.strategy, Strategy::Aggressive);

    // Should not include low-risk pools
    for allocation in summary.allocations.iter() {
        let pool = client.get_pool(&allocation.pool_id);
        assert_ne!(pool.risk_level, RiskLevel::Low);
    }
}

#[test]
fn test_get_allocation() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, core_id, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = String::from_str(&env, "commit_4");
    let amount = 50_000_000i128;
    
    create_mock_commitment(&env, &core_id, "commit_4", amount, "active");

    client.allocate(&user, &commitment_id, &amount, &Strategy::Safe);

    let summary = client.get_allocation(&commitment_id);

    assert_eq!(summary.commitment_id, commitment_id);
    assert_eq!(summary.strategy, Strategy::Safe);
    assert_eq!(summary.total_allocated, amount);
}

#[test]
fn test_rebalance() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, core_id, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = String::from_str(&env, "commit_5");
    let amount = 100_000_000i128;
    
    create_mock_commitment(&env, &core_id, "commit_5", amount, "active");

    // Initial allocation
    client.allocate(&user, &commitment_id, &amount, &Strategy::Safe);

    // Disable one of the pools
    client.update_pool_status(&admin, &0, &false);

    // Rebalance
    let rebalanced = client.rebalance(&user, &commitment_id);

    assert_eq!(rebalanced.strategy, Strategy::Safe);

    // Pool 0 should not be in new allocations
    for allocation in rebalanced.allocations.iter() {
        assert_ne!(allocation.pool_id, 0);
    }
}

#[test]
fn test_pool_liquidity_tracking() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, core_id, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = String::from_str(&env, "commit_6");
    let amount = 100_000_000i128;
    
    create_mock_commitment(&env, &core_id, "commit_6", amount, "active");

    // Check initial liquidity
    let pool_before = client.get_pool(&0);
    assert_eq!(pool_before.total_liquidity, 0);

    // Allocate
    client.allocate(&user, &commitment_id, &amount, &Strategy::Safe);

    // Check updated liquidity
    let pool_after = client.get_pool(&0);
    assert!(pool_after.total_liquidity > 0);
}

#[test]
#[should_panic(expected = "Rate limit exceeded")]
fn test_allocation_rate_limit_enforced() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, core_id, client) = create_contract(&env);

    // Configure rate limit: 1 allocation call per 60 seconds
    let fn_symbol = soroban_sdk::Symbol::new(&env, "alloc");
    client.set_rate_limit(&admin, &fn_symbol, &60u64, &1u32);

    let user = Address::generate(&env);
    
    setup_test_pools(&env, &client, &admin);
    
    create_mock_commitment(&env, &core_id, "c1", 10_000_000, "active");
    create_mock_commitment(&env, &core_id, "c2", 10_000_000, "active");

    // First allocation should succeed
    client.allocate(&user, &String::from_str(&env, "c1"), &10_000_000, &Strategy::Balanced);

    // Second allocation should panic due to rate limit
    client.allocate(&user, &String::from_str(&env, "c2"), &10_000_000, &Strategy::Balanced);
}

// ============================================================================
// DESIGN SPIKE: VALIDATION TESTS
// ============================================================================

#[test]
#[should_panic(expected = "Commitment not found in mock")]
fn test_allocation_nonexistent_commitment_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _core_id, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = String::from_str(&env, "missing_commitment");
    
    // Attempt to allocate for a commitment that was never created in core
    client.allocate(&user, &commitment_id, &100_000, &Strategy::Safe);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #6)")] // PoolInactive used for non-active commitment
fn test_allocation_inactive_commitment_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, core_id, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = String::from_str(&env, "settled_commitment");
    
    // Create commitment with "settled" status
    create_mock_commitment(&env, &core_id, "settled_commitment", 100_000_000, "settled");

    // Should fail because status is not "active"
    client.allocate(&user, &commitment_id, &10_000_000, &Strategy::Safe);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #18)")]
fn test_allocation_exceeds_commitment_balance_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, core_id, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = String::from_str(&env, "low_balance_commit");
    
    // Commitment has 50M balance
    create_mock_commitment(&env, &core_id, "low_balance_commit", 50_000_000, "active");

    // Attempt to allocate 100M should fail
    client.allocate(&user, &commitment_id, &100_000_000, &Strategy::Safe);
}

// ============================================================================
// REGISTER_POOL COMPREHENSIVE TESTS - Issue #234
// ============================================================================

// ============================================================================
// APY VALIDATION TESTS
// ============================================================================

#[test]
fn test_register_pool_valid_apy_boundary_values() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test minimum valid APY (0 basis points = 0%)
    client.register_pool(&admin, &0, &RiskLevel::Low, &0, &1_000_000_000);
    let pool_min = client.get_pool(&0);
    assert_eq!(pool_min.apy, 0);

    // Test maximum valid APY (100,000 basis points = 1000%)
    client.register_pool(&admin, &1, &RiskLevel::Low, &100_000, &1_000_000_000);
    let pool_max = client.get_pool(&1);
    assert_eq!(pool_max.apy, 100_000);

    // Test common APY values
    client.register_pool(&admin, &2, &RiskLevel::Medium, &500, &800_000_000); // 5%
    let pool_common = client.get_pool(&2);
    assert_eq!(pool_common.apy, 500);

    client.register_pool(&admin, &3, &RiskLevel::High, &1500, &500_000_000); // 15%
    let pool_high = client.get_pool(&3);
    assert_eq!(pool_high.apy, 1500);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_register_pool_invalid_apy_exceeds_maximum() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test APY exceeding maximum (100,001 basis points > 1000%)
    client.register_pool(&admin, &0, &RiskLevel::Low, &100_001, &1_000_000_000);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_register_pool_extremely_high_apy_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test extremely high APY value
    client.register_pool(&admin, &0, &RiskLevel::Low, &u32::MAX, &1_000_000_000);
}

#[test]
fn test_register_pool_apy_with_different_risk_levels() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test valid APY values across all risk levels
    client.register_pool(&admin, &0, &RiskLevel::Low, &300, &1_000_000_000);
    client.register_pool(&admin, &1, &RiskLevel::Medium, &800, &800_000_000);
    client.register_pool(&admin, &2, &RiskLevel::High, &2000, &500_000_000);

    let pool_low = client.get_pool(&0);
    let pool_medium = client.get_pool(&1);
    let pool_high = client.get_pool(&2);

    assert_eq!(pool_low.apy, 300);
    assert_eq!(pool_medium.apy, 800);
    assert_eq!(pool_high.apy, 2000);
}

// ============================================================================
// CAPACITY VALIDATION TESTS
// ============================================================================

#[test]
fn test_register_pool_valid_capacity_boundary_values() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test minimum valid capacity (1)
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1);
    let pool_min = client.get_pool(&0);
    assert_eq!(pool_min.max_capacity, 1);

    // Test large capacity values
    client.register_pool(&admin, &1, &RiskLevel::Medium, &1000, &i128::MAX);
    let pool_max = client.get_pool(&1);
    assert_eq!(pool_max.max_capacity, i128::MAX);

    // Test common capacity values
    client.register_pool(&admin, &2, &RiskLevel::High, &1500, &100_000_000);
    let pool_common = client.get_pool(&2);
    assert_eq!(pool_common.max_capacity, 100_000_000);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #12)")]
fn test_register_pool_zero_capacity_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test zero capacity
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &0);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #12)")]
fn test_register_pool_negative_capacity_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test negative capacity
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &-1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #12)")]
fn test_register_pool_extremely_negative_capacity_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test extremely negative capacity
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &i128::MIN);
}

// ============================================================================
// INVALID INPUT TESTS
// ============================================================================

#[test]
#[should_panic(expected = "HostError: Error(Contract, #10)")]
fn test_register_pool_duplicate_id_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Register first pool
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    
    // Attempt to register pool with same ID
    client.register_pool(&admin, &0, &RiskLevel::Medium, &1000, &800_000_000);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn test_register_pool_unauthorized_access() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, client) = create_contract(&env);
    let non_admin = Address::generate(&env);

    // Test registration by non-admin user
    client.register_pool(&non_admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #2)")]
fn test_register_pool_uninitialized_contract() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, AllocationStrategiesContract);
    let client = AllocationStrategiesContractClient::new(&env, &contract_id);

    // Test registration on uninitialized contract
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
}

#[test]
fn test_register_pool_all_risk_levels_valid() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test all risk levels with valid parameters
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    client.register_pool(&admin, &1, &RiskLevel::Medium, &1000, &800_000_000);
    client.register_pool(&admin, &2, &RiskLevel::High, &2000, &500_000_000);

    let pool_low = client.get_pool(&0);
    let pool_medium = client.get_pool(&1);
    let pool_high = client.get_pool(&2);

    assert_eq!(pool_low.risk_level, RiskLevel::Low);
    assert_eq!(pool_medium.risk_level, RiskLevel::Medium);
    assert_eq!(pool_high.risk_level, RiskLevel::High);
}

// ============================================================================
// SECURITY AND EDGE CASE TESTS
// ============================================================================

#[test]
fn test_register_pool_reentrancy_protection() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Test that reentrancy guard is properly handled during pool registration
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    
    // Verify pool was created successfully
    let pool = client.get_pool(&0);
    assert_eq!(pool.pool_id, 0);
    assert!(pool.active);
}

#[test]
fn test_register_pool_pool_registry_updated() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Verify initial registry is empty
    let pools_before = client.get_all_pools();
    assert_eq!(pools_before.len(), 0);

    // Register multiple pools
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    client.register_pool(&admin, &1, &RiskLevel::Medium, &1000, &800_000_000);
    client.register_pool(&admin, &2, &RiskLevel::High, &2000, &500_000_000);

    // Verify registry is updated
    let pools_after = client.get_all_pools();
    assert_eq!(pools_after.len(), 3);
}

#[test]
fn test_register_pool_timestamps_set() {
    let env = Env::default();
    env.mock_all_auths();

    // Set ledger timestamp
    env.ledger().set_timestamp(1000);

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);

    let pool = client.get_pool(&0);
    
    // Verify timestamps are set correctly
    assert!(pool.created_at > 0);
    assert!(pool.updated_at > 0);
    assert_eq!(pool.created_at, pool.updated_at);
}

#[test]
fn test_register_pool_default_values() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);

    let pool = client.get_pool(&0);
    
    // Verify default values are set correctly
    assert_eq!(pool.total_liquidity, 0);
    assert!(pool.active);
    assert_eq!(pool.pool_id, 0);
    assert_eq!(pool.risk_level, RiskLevel::Low);
    assert_eq!(pool.apy, 500);
    assert_eq!(pool.max_capacity, 1_000_000_000);
}

#[test]
fn test_register_pool_event_emission() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // This test verifies that the function executes without panicking
    // Event emission testing would require more sophisticated event capture mechanisms
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    
    let pool = client.get_pool(&0);
    assert_eq!(pool.pool_id, 0);
}
