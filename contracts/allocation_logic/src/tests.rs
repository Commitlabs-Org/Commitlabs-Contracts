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
// THREAT MODEL TESTS - Issue #242: Caller Authentication & Spoofing Prevention
// ============================================================================

#[test]
#[should_panic(expected = "HostError: Error(Contract, #3)")]
fn test_zero_address_caller_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    // Test with zero address - should be rejected as unauthorized
    let zero_address = Address::from_string(&soroban_sdk::String::from_str(&env, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"));
    client.allocate(&zero_address, &1, &100_000_000, &Strategy::Safe);
}

#[test]
fn test_caller_authentication_enforced() {
    let env = Env::default();
    // Don't mock all auths - we want to test actual authentication
    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user = Address::generate(&env);
    
    // Mock auth only for the specific user
    env.mock_auths(&[
        (&admin, &1, &admin),
        (&user, &1, &user),
    ]);

    // This should succeed with proper authentication
    let summary = client.allocate(&user, &1, &100_000_000, &Strategy::Safe);
    assert_eq!(summary.commitment_id, 1);
    assert_eq!(summary.total_allocated, 100_000_000);
}

#[test]
fn test_allocation_ownership_tracking() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let commitment_id = 1u64;

    // User1 allocates - should track ownership correctly
    let summary = client.allocate(&user1, &commitment_id, &100_000_000, &Strategy::Safe);
    assert_eq!(summary.total_allocated, 100_000_000);

    // Verify allocation exists
    let allocation = client.get_allocation(&commitment_id);
    assert_eq!(allocation.total_allocated, 100_000_000);

    // User2 cannot rebalance User1's allocation (ownership enforced)
    env.mock_auths(&[
        (&admin, &1, &admin),
        (&user1, &1, &user1),
        (&user2, &1, &user2), // Mock auth but should still fail due to ownership
    ]);

    // This should fail because user2 doesn't own the allocation
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.rebalance(&user2, &commitment_id);
    }));
    assert!(result.is_err());
}

#[test]
fn test_authentication_prevents_commitment_id_spoofing() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let commitment_id = 1u64;

    // User1 creates allocation
    client.allocate(&user1, &commitment_id, &50_000_000, &Strategy::Safe);

    // User2 cannot allocate to same commitment_id (double allocation prevention)
    // This tests that authentication + ownership tracking prevents spoofing
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.allocate(&user2, &commitment_id, &25_000_000, &Strategy::Balanced);
    }));
    assert!(result.is_err()); // Should panic with AlreadyInitialized error
}

#[test]
fn test_caller_auth_isolation_between_users() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Both users create separate allocations
    let summary1 = client.allocate(&user1, &1, &100_000_000, &Strategy::Safe);
    let summary2 = client.allocate(&user2, &2, &100_000_000, &Strategy::Safe);

    // Verify allocations are properly isolated
    assert_eq!(summary1.commitment_id, 1);
    assert_eq!(summary2.commitment_id, 2);
    assert_ne!(summary1.commitment_id, summary2.commitment_id);

    // Each user can only rebalance their own allocation
    client.rebalance(&user1, &1); // Should succeed
    client.rebalance(&user2, &2); // Should succeed

    // Cross-user rebalance should fail
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.rebalance(&user1, &2); // user1 trying to rebalance user2's allocation
    }));
    assert!(result.is_err());
}

#[test]
fn test_authentication_with_rate_limiting() {
    let env = Env::default();
    env.mock_all_auths();

    let (admin, _, client) = create_contract(&env);
    setup_test_pools(&env, &client, &admin);

    // Configure strict rate limit
    let fn_symbol = soroban_sdk::Symbol::new(&env, "alloc");
    client.set_rate_limit(&admin, &fn_symbol, &60u64, &1u32);

    let user = Address::generate(&env);

    // First allocation should succeed
    client.allocate(&user, &1, &50_000_000, &Strategy::Safe);

    // Second allocation should fail due to rate limit
    // This tests that authentication works correctly with rate limiting
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.allocate(&user, &2, &50_000_000, &Strategy::Safe);
    }));
    assert!(result.is_err());
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
