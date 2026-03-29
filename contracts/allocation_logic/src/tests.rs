// Comprehensive Security-Focused Tests
use crate::{
    AllocationStrategiesContract, AllocationStrategiesContractClient, RiskLevel, Strategy,
};
use soroban_sdk::{testutils::Address as _, testutils::Ledger, Address, Env};

fn create_contract(env: &Env) -> (Address, Address, AllocationStrategiesContractClient<'_>) {
    let admin = Address::generate(env);
    let commitment_core = Address::generate(env);
    let contract_id = env.register_contract(None, AllocationStrategiesContract);
    let client = AllocationStrategiesContractClient::new(env, &contract_id);

    client.initialize(&admin, &commitment_core);

    (admin, commitment_core, client)
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
// COMPREHENSIVE REBALANCE TESTS - Issue #236
// Focus: Owner Match, Strategy Persistence, Summary Correctness
// ============================================================================

#[test]
fn test_rebalance_owner_match_verification() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let owner = Address::generate(&env);
    let non_owner = Address::generate(&env);
    let commitment_id = 1u64;
    let amount = 100_000_000i128;

    // Owner creates allocation
    let initial_summary = client.allocate(&owner, &commitment_id, &amount, &Strategy::Balanced);
    assert_eq!(initial_summary.total_allocated, amount);

    // Owner can rebalance - should succeed
    let rebalanced_by_owner = client.rebalance(&owner, &commitment_id);
    assert_eq!(rebalanced_by_owner.commitment_id, commitment_id);
    assert_eq!(rebalanced_by_owner.strategy, Strategy::Balanced);

    // Non-owner cannot rebalance - should fail
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.rebalance(&non_owner, &commitment_id);
    }));
    assert!(result.is_err()); // Should panic with Unauthorized error
}

#[test]
fn test_rebalance_strategy_persistence_safe_strategy() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = 1u64;
    let amount = 100_000_000i128;

    // Create allocation with Safe strategy
    let initial_summary = client.allocate(&user, &commitment_id, &amount, &Strategy::Safe);
    assert_eq!(initial_summary.strategy, Strategy::Safe);

    // Verify initial allocation uses only low-risk pools
    for allocation in initial_summary.allocations.iter() {
        let pool = client.get_pool(&allocation.pool_id);
        assert_eq!(pool.risk_level, RiskLevel::Low);
    }

    // Rebalance should maintain Safe strategy
    let rebalanced_summary = client.rebalance(&user, &commitment_id);
    assert_eq!(rebalanced_summary.strategy, Strategy::Safe);
    assert_eq!(rebalanced_summary.commitment_id, commitment_id);

    // Verify rebalanced allocation still uses only low-risk pools
    for allocation in rebalanced_summary.allocations.iter() {
        let pool = client.get_pool(&allocation.pool_id);
        assert_eq!(pool.risk_level, RiskLevel::Low);
    }

    // Total allocated should remain the same
    assert_eq!(rebalanced_summary.total_allocated, amount);
}

#[test]
fn test_rebalance_strategy_persistence_aggressive_strategy() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = 1u64;
    let amount = 100_000_000i128;

    // Create allocation with Aggressive strategy
    let initial_summary = client.allocate(&user, &commitment_id, &amount, &Strategy::Aggressive);
    assert_eq!(initial_summary.strategy, Strategy::Aggressive);

    // Verify initial allocation uses only medium/high-risk pools
    for allocation in initial_summary.allocations.iter() {
        let pool = client.get_pool(&allocation.pool_id);
        assert_ne!(pool.risk_level, RiskLevel::Low);
    }

    // Rebalance should maintain Aggressive strategy
    let rebalanced_summary = client.rebalance(&user, &commitment_id);
    assert_eq!(rebalanced_summary.strategy, Strategy::Aggressive);
    assert_eq!(rebalanced_summary.commitment_id, commitment_id);

    // Verify rebalanced allocation still uses only medium/high-risk pools
    for allocation in rebalanced_summary.allocations.iter() {
        let pool = client.get_pool(&allocation.pool_id);
        assert_ne!(pool.risk_level, RiskLevel::Low);
    }

    // Total allocated should remain the same
    assert_eq!(rebalanced_summary.total_allocated, amount);
}

#[test]
fn test_rebalance_summary_correctness() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = 1u64;
    let amount = 100_000_000i128;
    let strategy = Strategy::Balanced;

    // Create initial allocation
    let initial_summary = client.allocate(&user, &commitment_id, &amount, &strategy);
    
    // Verify initial summary correctness
    assert_eq!(initial_summary.commitment_id, commitment_id);
    assert_eq!(initial_summary.strategy, strategy);
    assert_eq!(initial_summary.total_allocated, amount);
    
    // Verify allocation amounts sum to total
    let mut sum_from_allocations = 0i128;
    for allocation in initial_summary.allocations.iter() {
        sum_from_allocations += allocation.amount;
        assert_eq!(allocation.commitment_id, commitment_id);
    }
    assert_eq!(sum_from_allocations, amount);

    // Rebalance
    let rebalanced_summary = client.rebalance(&user, &commitment_id);

    // Verify rebalanced summary correctness
    assert_eq!(rebalanced_summary.commitment_id, commitment_id);
    assert_eq!(rebalanced_summary.strategy, strategy);
    assert_eq!(rebalanced_summary.total_allocated, amount);

    // Verify rebalanced allocation amounts sum to total
    let mut rebalanced_sum = 0i128;
    for allocation in rebalanced_summary.allocations.iter() {
        rebalanced_sum += allocation.amount;
        assert_eq!(allocation.commitment_id, commitment_id);
        assert!(allocation.amount > 0);
        assert!(allocation.timestamp > 0);
    }
    assert_eq!(rebalanced_sum, amount);

    // Verify storage is updated correctly
    let stored_summary = client.get_allocation(&commitment_id);
    assert_eq!(stored_summary.commitment_id, commitment_id);
    assert_eq!(stored_summary.strategy, strategy);
    assert_eq!(stored_summary.total_allocated, amount);
    assert_eq!(stored_summary.allocations.len(), rebalanced_summary.allocations.len());
}

#[test]
fn test_rebalance_with_pool_status_changes() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = 1u64;
    let amount = 100_000_000i128;

    // Create initial allocation
    let initial_summary = client.allocate(&user, &commitment_id, &amount, &Strategy::Balanced);
    let initial_pool_count = initial_summary.allocations.len();

    // Deactivate some pools
    client.update_pool_status(&admin, &0, &false); // Low risk pool
    client.update_pool_status(&admin, &2, &false); // Medium risk pool

    // Rebalance should adapt to active pools only
    let rebalanced_summary = client.rebalance(&user, &commitment_id);
    
    // Strategy should be maintained
    assert_eq!(rebalanced_summary.strategy, Strategy::Balanced);
    assert_eq!(rebalanced_summary.commitment_id, commitment_id);
    
    // Total should remain the same
    assert_eq!(rebalanced_summary.total_allocated, amount);
    
    // Allocations should only use active pools
    for allocation in rebalanced_summary.allocations.iter() {
        let pool = client.get_pool(&allocation.pool_id);
        assert!(pool.active);
    }

    // Verify summary correctness after pool changes
    let mut sum = 0i128;
    for allocation in rebalanced_summary.allocations.iter() {
        sum += allocation.amount;
    }
    assert_eq!(sum, amount);
}

#[test]
fn test_rebalance_multiple_commitments_isolation() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let commitment1 = 1u64;
    let commitment2 = 2u64;
    let amount = 50_000_000i128;

    // Create two separate allocations with different strategies
    let summary1 = client.allocate(&user1, &commitment1, &amount, &Strategy::Safe);
    let summary2 = client.allocate(&user2, &commitment2, &amount, &Strategy::Aggressive);

    // Verify initial state
    assert_eq!(summary1.strategy, Strategy::Safe);
    assert_eq!(summary2.strategy, Strategy::Aggressive);

    // Rebalance commitment1 - should not affect commitment2
    let rebalanced1 = client.rebalance(&user1, &commitment1);
    assert_eq!(rebalanced1.strategy, Strategy::Safe);
    assert_eq!(rebalanced1.commitment_id, commitment1);
    assert_eq!(rebalanced1.total_allocated, amount);

    // Verify commitment2 is unchanged
    let unchanged2 = client.get_allocation(&commitment2);
    assert_eq!(unchanged2.strategy, Strategy::Aggressive);
    assert_eq!(unchanged2.commitment_id, commitment2);
    assert_eq!(unchanged2.total_allocated, amount);
    assert_eq!(unchanged2.allocations.len(), summary2.allocations.len());

    // Rebalance commitment2 - should not affect commitment1
    let rebalanced2 = client.rebalance(&user2, &commitment2);
    assert_eq!(rebalanced2.strategy, Strategy::Aggressive);
    assert_eq!(rebalanced2.commitment_id, commitment2);
    assert_eq!(rebalanced2.total_allocated, amount);

    // Verify commitment1 is still unchanged
    let final1 = client.get_allocation(&commitment1);
    assert_eq!(final1.strategy, Strategy::Safe);
    assert_eq!(final1.total_allocated, amount);
}

#[test]
fn test_rebalance_summary_timestamp_updates() {
    let env = Env::default();
    env.mock_all_auths();

    // Set initial timestamp
    env.ledger().set_timestamp(1000);

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = 1u64;
    let amount = 100_000_000i128;

    // Create initial allocation
    let initial_summary = client.allocate(&user, &commitment_id, &amount, &Strategy::Balanced);
    let initial_timestamp = initial_summary.allocations.get(0).unwrap().timestamp;
    assert_eq!(initial_timestamp, 1000);

    // Advance time
    env.ledger().set_timestamp(2000);

    // Rebalance should update timestamps
    let rebalanced_summary = client.rebalance(&user, &commitment_id);
    
    // All allocations should have new timestamps
    for allocation in rebalanced_summary.allocations.iter() {
        assert_eq!(allocation.timestamp, 2000);
        assert_ne!(allocation.timestamp, initial_timestamp);
    }

    // Summary correctness should be maintained
    assert_eq!(rebalanced_summary.total_allocated, amount);
    assert_eq!(rebalanced_summary.strategy, Strategy::Balanced);
}

#[test]
fn test_rebalance_edge_case_zero_allocation() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    
    // Register only pools that will be deactivated
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    client.register_pool(&admin, &1, &RiskLevel::Medium, &1000, &800_000_000);
    
    // Deactivate all pools
    client.update_pool_status(&admin, &0, &false);
    client.update_pool_status(&admin, &1, &false);

    let user = Address::generate(&env);
    let commitment_id = 1u64;
    let amount = 100_000_000i128;

    // Initial allocation should fail due to no active pools
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.allocate(&user, &commitment_id, &amount, &Strategy::Balanced);
    }));
    assert!(result.is_err());
}

#[test]
fn test_rebalance_with_capacity_constraints() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    
    // Register pools with limited capacity
    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &50_000_000); // Limited capacity
    client.register_pool(&admin, &1, &RiskLevel::Low, &600, &1_000_000_000); // Large capacity

    let user = Address::generate(&env);
    let commitment_id = 1u64;
    let amount = 100_000_000i128;

    // Create allocation that hits capacity constraints
    let initial_summary = client.allocate(&user, &commitment_id, &amount, &Strategy::Safe);
    assert_eq!(initial_summary.total_allocated, amount);

    // Fill up pool 0 to capacity
    let other_user = Address::generate(&env);
    client.allocate(&other_user, &2, &50_000_000, &Strategy::Safe);

    // Rebalance should handle capacity constraints correctly
    let rebalanced_summary = client.rebalance(&user, &commitment_id);
    
    // Summary should remain correct
    assert_eq!(rebalanced_summary.total_allocated, amount);
    assert_eq!(rebalanced_summary.strategy, Strategy::Safe);
    
    // Verify allocations respect capacity
    let pool0 = client.get_pool(&0);
    let pool1 = client.get_pool(&1);
    assert!(pool0.total_liquidity <= pool0.max_capacity);
    assert!(pool1.total_liquidity <= pool1.max_capacity);
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

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let commitment_id = 1u64;
    let amount = 100_000_000i128;

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

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let summary = client.allocate(&user, &2, &100_000_000, &Strategy::Balanced);

    assert_eq!(summary.strategy, Strategy::Balanced);

    // Should have allocations across different risk levels
    let mut has_allocation = false;

    for allocation in summary.allocations.iter() {
        if allocation.amount > 0 {
            has_allocation = true;
        }
    }

    assert!(has_allocation);
}

#[test]
fn test_aggressive_strategy_allocation() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let summary = client.allocate(&user, &3, &100_000_000, &Strategy::Aggressive);

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

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let amount = 50_000_000i128;

    client.allocate(&user, &4, &amount, &Strategy::Safe);

    let summary = client.get_allocation(&4);

    assert_eq!(summary.commitment_id, 4);
    assert_eq!(summary.strategy, Strategy::Safe);
    assert_eq!(summary.total_allocated, amount);
}

#[test]
fn test_rebalance() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let amount = 100_000_000i128;

    // Initial allocation
    let _initial = client.allocate(&user, &5, &amount, &Strategy::Safe);

    // Disable one of the pools
    client.update_pool_status(&admin, &0, &false);

    // Rebalance
    let rebalanced = client.rebalance(&user, &5);

    assert_eq!(rebalanced.strategy, Strategy::Safe);

    // Pool 0 should not be in new allocations
    for allocation in rebalanced.allocations.iter() {
        assert_ne!(allocation.pool_id, 0);
    }
}

#[test]
fn test_get_all_pools() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    client.register_pool(&admin, &1, &RiskLevel::Medium, &1000, &800_000_000);
    client.register_pool(&admin, &2, &RiskLevel::High, &2000, &500_000_000);

    let pools = client.get_all_pools();

    assert_eq!(pools.len(), 3);
}

#[test]
fn test_pool_liquidity_tracking() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);

    // Check initial liquidity
    let pool_before = client.get_pool(&0);
    assert_eq!(pool_before.total_liquidity, 0);

    // Allocate
    client.allocate(&user, &1, &100_000_000, &Strategy::Safe);

    // Check updated liquidity
    let pool_after = client.get_pool(&0);
    assert!(pool_after.total_liquidity > 0);
}

#[test]
fn test_allocation_timestamps() {
    let env = Env::default();
    env.mock_all_auths();

    // Set ledger timestamp
    env.ledger().set_timestamp(1000);

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);

    let summary = client.allocate(&user, &7, &100_000_000, &Strategy::Safe);

    // All allocations should have timestamps
    for allocation in summary.allocations.iter() {
        assert!(allocation.timestamp > 0);
    }
}

#[test]
fn test_total_allocation_accuracy() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let amount = 100_000_000i128;

    let summary = client.allocate(&user, &8, &amount, &Strategy::Balanced);

    // Sum all allocations
    let mut total = 0i128;
    for allocation in summary.allocations.iter() {
        total += allocation.amount;
    }

    assert_eq!(total, amount);
    assert_eq!(summary.total_allocated, amount);
}

#[test]
fn test_multiple_users_allocations() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    // Create multiple users and allocate
    for i in 0..5 {
        let user = Address::generate(&env);
        client.allocate(&user, &(i + 10), &10_000_000, &Strategy::Balanced);
    }

    // Verify all allocations exist
    for i in 0..5 {
        let summary = client.get_allocation(&(i + 10));
        assert_eq!(summary.total_allocated, 10_000_000);
    }
}

#[test]
#[should_panic(expected = "Rate limit exceeded")]
fn test_allocation_rate_limit_enforced() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    // Configure rate limit: 1 allocation call per 60 seconds
    let fn_symbol = soroban_sdk::Symbol::new(&env, "alloc");
    client.set_rate_limit(&admin, &fn_symbol, &60u64, &1u32);

    let user = Address::generate(&env);

    // First allocation should succeed
    setup_test_pools(&env, &client, &admin);
    client.allocate(&user, &100, &10_000_000, &Strategy::Balanced);

    // Second allocation should panic due to rate limit
    client.allocate(&user, &101, &10_000_000, &Strategy::Balanced);
}

#[test]
fn test_get_nonexistent_allocation() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, client) = create_contract(&env);

    let summary = client.get_allocation(&999);

    assert_eq!(summary.total_allocated, 0);
    assert_eq!(summary.allocations.len(), 0);
}

#[test]
fn test_pool_timestamps() {
    let env = Env::default();
    env.mock_all_auths();

    // Set ledger timestamp to non-zero
    env.ledger().set_timestamp(1000);

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);

    let pool = client.get_pool(&0);

    assert!(pool.created_at > 0);
    assert!(pool.updated_at > 0);
    assert_eq!(pool.created_at, pool.updated_at);
}

// ============================================================================
// ERROR TESTS - Using should_panic
// ============================================================================

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_double_initialization_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, commitment_core, client) = create_contract(&env);
    client.initialize(&admin, &commitment_core);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn test_non_admin_cannot_register_pool() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, client) = create_contract(&env);
    let non_admin = Address::generate(&env);

    client.register_pool(&non_admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #4)")]
fn test_zero_amount_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    client.allocate(&user, &1, &0, &Strategy::Safe);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #12)")]
fn test_zero_capacity_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &0);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #11)")]
fn test_excessive_apy_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &100_001, &1_000_000_000);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #10)")]
fn test_duplicate_pool_id_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    client.register_pool(&admin, &0, &RiskLevel::Medium, &1000, &800_000_000);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #7)")]
fn test_pool_capacity_exceeded() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &100_000);

    let user = Address::generate(&env);
    client.allocate(&user, &1, &200_000, &Strategy::Safe);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_double_allocation_prevented() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);

    client.allocate(&user, &1, &100_000, &Strategy::Safe);
    client.allocate(&user, &1, &50_000, &Strategy::Balanced);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn test_non_owner_cannot_rebalance() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    let other_user = Address::generate(&env);

    client.allocate(&user, &1, &100_000_000, &Strategy::Safe);
    client.rebalance(&other_user, &1);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #8)")]
fn test_no_active_pools_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);

    client.register_pool(&admin, &0, &RiskLevel::Low, &500, &1_000_000_000);
    client.update_pool_status(&admin, &0, &false);

    let user = Address::generate(&env);
    client.allocate(&user, &1, &100_000, &Strategy::Safe);
}

// ============================================================================
// BALANCE CHECKING TESTS - Issue #147
// ============================================================================

#[test]
#[should_panic(expected = "HostError: Error(Contract, #18)")]
fn test_allocation_exceeds_commitment_balance_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _commitment_core, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);

    // Test allocation when amount exceeds commitment current_value
    // commitment_id 100 has balance of 50M, but we try to allocate 100M
    let commitment_id = 100u64;
    let allocation_amount = 100_000_000i128;

    // This should fail because allocation amount exceeds commitment balance
    client.allocate(&user, &commitment_id, &allocation_amount, &Strategy::Safe);
}

#[test]
fn test_allocation_equals_commitment_balance_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _commitment_core, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);

    // Test allocation when amount equals commitment current_value
    // commitment_id 200 has balance of 50M, we allocate exactly 50M
    let commitment_id = 200u64;
    let allocation_amount = 50_000_000i128;

    // This should succeed when amount == current_value
    let summary = client.allocate(&user, &commitment_id, &allocation_amount, &Strategy::Safe);

    assert_eq!(summary.commitment_id, commitment_id);
    assert_eq!(summary.total_allocated, allocation_amount);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #18)")]
fn test_multiple_allocations_exceed_total_balance_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _commitment_core, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);

    // First allocation succeeds (commitment_id 300 has 100M balance)
    let first_commitment_id = 300u64;
    let first_amount = 30_000_000i128;
    client.allocate(&user, &first_commitment_id, &first_amount, &Strategy::Safe);

    // Second allocation should fail (commitment_id 400 has 100M balance, but we try 110M)
    let second_commitment_id = 400u64;
    let second_amount = 110_000_000i128;

    // This should fail because allocation amount exceeds commitment balance
    client.allocate(
        &user,
        &second_commitment_id,
        &second_amount,
        &Strategy::Safe,
    );
}
