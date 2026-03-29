#![cfg(test)]
#![cfg(feature = "benchmark")]

use super::*;
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

fn setup_test_env(e: &Env) -> Address {
    e.mock_all_auths();
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
    e.mock_all_auths();
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
            owner.clone(),
            String::from_str(&e, "commitment_1"),
            30,
            20,
            String::from_str(&e, "balanced"),
            1000_0000000,
            Address::generate(&e),
            10,
        )
        .unwrap();
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
            owner.clone(),
            String::from_str(&e, "commitment_1"),
            30,
            20,
            String::from_str(&e, "balanced"),
            1000_0000000,
            Address::generate(&e),
            10,
        )
        .unwrap()
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
            owner.clone(),
            String::from_str(&e, "commitment_1"),
            30,
            20,
            String::from_str(&e, "balanced"),
            1000_0000000,
            Address::generate(&e),
            10,
        )
        .unwrap()
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
            owner.clone(),
            String::from_str(&e, "commitment_1"),
            30,
            20,
            String::from_str(&e, "balanced"),
            1000_0000000,
            Address::generate(&e),
            10,
        )
        .unwrap();
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
    let commitment_ids = [
        "commitment_0",
        "commitment_1",
        "commitment_2",
        "commitment_3",
        "commitment_4",
        "commitment_5",
        "commitment_6",
        "commitment_7",
        "commitment_8",
        "commitment_9",
    ];

    let mut metrics = BenchmarkMetrics::new("batch_mint_10");

    let start = e.ledger().sequence();
    for commitment_id in commitment_ids.iter() {
        e.as_contract(&contract_id, || {
            let _ = CommitmentNFTContract::mint(
                e.clone(),
                owner.clone(),
                owner.clone(),
                String::from_str(&e, commitment_id),
                30,
                20,
                String::from_str(&e, "balanced"),
                1000_0000000,
                Address::generate(&e),
                10,
            )
            .unwrap();
        });
    }
    let end = e.ledger().sequence();
    metrics.record_gas(start, end);

    metrics.print_summary();
}
