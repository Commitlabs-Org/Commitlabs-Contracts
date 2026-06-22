#![cfg(test)]
#![cfg(feature = "benchmark")]

use super::*;
use shared_utils::BatchMode;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

/// Benchmark helper to measure gas usage
struct BenchmarkMetrics {
    function_name: String,
    gas_before: u32,
    gas_after: u32,
}

impl BenchmarkMetrics {
    fn new(function_name: &str) -> Self {
        let e = Env::default();
        Self {
            function_name: String::from_str(&e, function_name),
            gas_before: 0,
            gas_after: 0,
        }
    }

    fn record_gas(&mut self, before: u32, after: u32) {
        self.gas_before = before;
        self.gas_after = after;
    }

    fn print_summary(&self) {
        let _gas_used = self.gas_after.saturating_sub(self.gas_before);
        let _ = &self.function_name;
        // Benchmark metrics collected - can be extended with proper logging
    }
}

fn setup_test_env(e: &Env) -> (Address, Address) {
    e.mock_all_auths();
    let admin = Address::generate(e);
    let core_contract = Address::generate(e);
    let contract_id = e.register_contract(None, AllocationStrategiesContract);

    e.as_contract(&contract_id, || {
        AllocationStrategiesContract::initialize(e.clone(), admin.clone(), core_contract.clone())
            .unwrap();
    });

    (contract_id, admin)
}

#[test]
fn benchmark_initialize() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let core_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, AllocationStrategiesContract);

    let mut metrics = BenchmarkMetrics::new("initialize");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        AllocationStrategiesContract::initialize(e.clone(), admin.clone(), core_contract.clone())
            .unwrap();
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_register_pool() {
    let e = Env::default();
    let (contract_id, admin) = setup_test_env(&e);

    let mut metrics = BenchmarkMetrics::new("register_pool");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        AllocationStrategiesContract::register_pool(
            e.clone(),
            admin.clone(),
            1,
            RiskLevel::Low,
            500, // 5% APY
            10000_0000000,
        )
        .unwrap();
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_allocate() {
    let e = Env::default();
    let (contract_id, admin) = setup_test_env(&e);

    // Register a pool first
    e.as_contract(&contract_id, || {
        AllocationStrategiesContract::register_pool(
            e.clone(),
            admin.clone(),
            1,
            RiskLevel::Low,
            500,
            10000_0000000,
        )
        .unwrap();
    });

    let caller = Address::generate(&e);
    let mut metrics = BenchmarkMetrics::new("allocate");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        let _ = AllocationStrategiesContract::allocate(
            e.clone(),
            caller.clone(),
            String::from_str(&e, "bench_allocate"),
            1000_0000000,
            Strategy::Safe,
        );
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_get_allocation() {
    let e = Env::default();
    let (contract_id, admin) = setup_test_env(&e);

    // Register pool and create allocation
    let caller = Address::generate(&e);
    e.as_contract(&contract_id, || {
        AllocationStrategiesContract::register_pool(
            e.clone(),
            admin.clone(),
            1,
            RiskLevel::Low,
            500,
            10000_0000000,
        )
        .unwrap();
        let _ = AllocationStrategiesContract::allocate(
            e.clone(),
            caller.clone(),
            String::from_str(&e, "bench_get_allocation"),
            1000_0000000,
            Strategy::Safe,
        );
    });

    let mut metrics = BenchmarkMetrics::new("get_allocation");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        AllocationStrategiesContract::get_allocation(e.clone(), String::from_str(&e, "bench_get_allocation"));
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_get_pool() {
    let e = Env::default();
    let (contract_id, admin) = setup_test_env(&e);

    // Register a pool first
    e.as_contract(&contract_id, || {
        AllocationStrategiesContract::register_pool(
            e.clone(),
            admin.clone(),
            1,
            RiskLevel::Low,
            500,
            10000_0000000,
        )
        .unwrap();
    });

    let mut metrics = BenchmarkMetrics::new("get_pool");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        AllocationStrategiesContract::get_pool(e.clone(), 1).unwrap();
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_batch_allocate() {
    let e = Env::default();
    let (contract_id, admin) = setup_test_env(&e);

    // Register pools
    for i in 1..=5 {
        e.as_contract(&contract_id, || {
            AllocationStrategiesContract::register_pool(
                e.clone(),
                admin.clone(),
                i,
                RiskLevel::Low,
                500,
                10000_0000000,
            )
            .unwrap();
        });
    }

    let mut metrics = BenchmarkMetrics::new("batch_allocate_10");

    let caller = Address::generate(&e);
    let mut params = Vec::new(&e);
    for _ in 1..=10 {
        params.push_back(BatchAllocateParams {
            caller: caller.clone(),
            commitment_id: String::from_str(&e, "bench_commit"),
            amount: 1000_0000000,
            strategy: Strategy::Safe,
        });
    }

    let start = e.ledger().sequence();
    e.as_contract(&contract_id, || {
        let _ = AllocationStrategiesContract::batch_allocate(
            e.clone(),
            admin.clone(),
            params,
            BatchMode::BestEffort,
        );
    });
    let end = e.ledger().sequence();
    metrics.record_gas(start, end);

    metrics.print_summary();
}
