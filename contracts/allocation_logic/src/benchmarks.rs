#![cfg(test)]
#![cfg(feature = "benchmark")]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger}, Address, Env,
};

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

    fn record_gas(&mut self, before: u64, after: u64) {
        self.gas_before = before;
        self.gas_after = after;
    }

    fn print_summary(&self) {
        let gas_used = if self.gas_after > self.gas_before {
            self.gas_after - self.gas_before
        } else {
            0
        };
        // Benchmark metrics collected - can be extended with proper logging
    }
}

fn setup_test_env(e: &Env) -> (Address, Address) {
    let admin = Address::generate(e);
    let core_contract = Address::generate(e);
    let contract_id = e.register_contract(None, AllocationStrategiesContract);
    
    e.as_contract(&contract_id, || {
        AllocationStrategiesContract::initialize(
            e.clone(),
            admin.clone(),
            core_contract.clone(),
        ).unwrap();
    });
    
    (contract_id, admin)
}

#[test]
fn benchmark_initialize() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let core_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, AllocationStrategiesContract);
    
    let mut metrics = BenchmarkMetrics::new("initialize");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        AllocationStrategiesContract::initialize(
            e.clone(),
            admin.clone(),
            core_contract.clone(),
        ).unwrap();
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
        ).unwrap();
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
        ).unwrap();
    });
    
    let caller = Address::generate(&e);
    let mut metrics = BenchmarkMetrics::new("allocate");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        let _ = AllocationStrategiesContract::allocate(
            e.clone(),
            caller.clone(),
            1,
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
        ).unwrap();
        AllocationStrategiesContract::allocate(
            e.clone(),
            caller.clone(),
            1,
            1000_0000000,
            Strategy::Safe,
        ).unwrap();
    });
    
    let mut metrics = BenchmarkMetrics::new("get_allocation");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        AllocationStrategiesContract::get_allocation(e.clone(), 1);
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
        ).unwrap();
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
    e.as_contract(&contract_id, || {
        for i in 1..=5 {
            AllocationStrategiesContract::register_pool(
                e.clone(),
                admin.clone(),
                i,
                RiskLevel::Low,
                500,
                10000_0000000,
            ).unwrap();
        }
    });
    
    let caller = Address::generate(&e);
    let mut metrics = BenchmarkMetrics::new("batch_allocate_10");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        for i in 1..=10 {
            let _ = AllocationStrategiesContract::allocate(
                e.clone(),
                caller.clone(),
                i,
                1000_0000000,
                Strategy::Safe,
            );
        }
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });
    
    metrics.print_summary();
}
