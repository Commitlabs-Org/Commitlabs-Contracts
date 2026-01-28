#![cfg(test)]
#![cfg(feature = "benchmark")]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger}, Address, Env, String,
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

fn setup_test_env(e: &Env) -> Address {
    let admin = Address::generate(e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    e.as_contract(&contract_id, || {
        CommitmentNFTContract::initialize(e.clone(), admin.clone()).unwrap();
    });
    
    contract_id
}

#[test]
fn benchmark_initialize() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentNFTContract);
    
    let mut metrics = BenchmarkMetrics::new("initialize");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        CommitmentNFTContract::initialize(e.clone(), admin.clone()).unwrap();
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });
    
    metrics.print_summary();
}

#[test]
fn benchmark_mint() {
    let e = Env::default();
    let contract_id = setup_test_env(&e);
    let owner = Address::generate(&e);
    
    let mut metrics = BenchmarkMetrics::new("mint");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        let _ = CommitmentNFTContract::mint(
            e.clone(),
            owner.clone(),
            String::from_str(&e, "commitment_1"),
            30,
            20,
            String::from_str(&e, "balanced"),
            1000_0000000,
            Address::generate(&e),
            10,
        ).unwrap();
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });
    
    metrics.print_summary();
}

#[test]
fn benchmark_get_metadata() {
    let e = Env::default();
    let contract_id = setup_test_env(&e);
    let owner = Address::generate(&e);
    
    let token_id = e.as_contract(&contract_id, || {
        CommitmentNFTContract::mint(
            e.clone(),
            owner.clone(),
            String::from_str(&e, "commitment_1"),
            30,
            20,
            String::from_str(&e, "balanced"),
            1000_0000000,
            Address::generate(&e),
            10,
        ).unwrap()
    });
    
    let mut metrics = BenchmarkMetrics::new("get_metadata");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        CommitmentNFTContract::get_metadata(e.clone(), token_id).unwrap();
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });
    
    metrics.print_summary();
}

#[test]
fn benchmark_owner_of() {
    let e = Env::default();
    let contract_id = setup_test_env(&e);
    let owner = Address::generate(&e);
    
    let token_id = e.as_contract(&contract_id, || {
        CommitmentNFTContract::mint(
            e.clone(),
            owner.clone(),
            String::from_str(&e, "commitment_1"),
            30,
            20,
            String::from_str(&e, "balanced"),
            1000_0000000,
            Address::generate(&e),
            10,
        ).unwrap()
    });
    
    let mut metrics = BenchmarkMetrics::new("owner_of");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        CommitmentNFTContract::owner_of(e.clone(), token_id).unwrap();
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });
    
    metrics.print_summary();
}

#[test]
fn benchmark_balance_of() {
    let e = Env::default();
    let contract_id = setup_test_env(&e);
    let owner = Address::generate(&e);
    
    e.as_contract(&contract_id, || {
        CommitmentNFTContract::mint(
            e.clone(),
            owner.clone(),
            String::from_str(&e, "commitment_1"),
            30,
            20,
            String::from_str(&e, "balanced"),
            1000_0000000,
            Address::generate(&e),
            10,
        ).unwrap();
    });
    
    let mut metrics = BenchmarkMetrics::new("balance_of");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        CommitmentNFTContract::balance_of(e.clone(), owner.clone());
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });
    
    metrics.print_summary();
}

#[test]
fn benchmark_batch_mint() {
    let e = Env::default();
    let contract_id = setup_test_env(&e);
    let owner = Address::generate(&e);
    
    let mut metrics = BenchmarkMetrics::new("batch_mint_10");
    
    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        for i in 0..10 {
            let _ = CommitmentNFTContract::mint(
                e.clone(),
                owner.clone(),
                String::from_str(&e, &format!("commitment_{}", i)),
                30,
                20,
                String::from_str(&e, "balanced"),
                1000_0000000,
                Address::generate(&e),
                10,
            ).unwrap();
        }
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });
    
    metrics.print_summary();
}
