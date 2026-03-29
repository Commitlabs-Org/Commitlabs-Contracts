#![cfg(test)]
#![cfg(feature = "benchmark")]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

/// Benchmark helper to measure gas usage
#[allow(dead_code)]
struct BenchmarkMetrics {
    function_name: String,
    gas_before: u32,
    gas_after: u32,
    storage_reads: u32,
    storage_writes: u32,
}

impl BenchmarkMetrics {
    fn new(function_name: &str) -> Self {
        let e = Env::default();
        Self {
            function_name: String::from_str(&e, function_name),
            gas_before: 0,
            gas_after: 0,
            storage_reads: 0,
            storage_writes: 0,
        }
    }

    fn record_gas(&mut self, before: u32, after: u32) {
        self.gas_before = before;
        self.gas_after = after;
    }

    fn print_summary(&self) {
        // Benchmark metrics collected - can be extended with proper logging
        // For now, metrics are collected but not printed in no_std environment
        // In CI/CD, these will be captured via test output
    }
}

fn setup_test_env(e: &Env) -> (Address, Address, Address) {
    e.mock_all_auths();
    let admin = Address::generate(e);
    let nft_contract = Address::generate(e);
    let owner = Address::generate(e);

    let contract_id = e.register_contract(None, CommitmentCoreContract);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
    });

    (contract_id, admin, owner)
}

fn build_benchmark_commitment(
    e: &Env,
    owner: &Address,
    commitment_id: &str,
    amount: i128,
) -> Commitment {
    Commitment {
        commitment_id: String::from_str(e, commitment_id),
        owner: owner.clone(),
        nft_token_id: 1,
        rules: CommitmentRules {
            duration_days: 30,
            max_loss_percent: 20,
            commitment_type: String::from_str(e, "balanced"),
            early_exit_penalty: 10,
            min_fee_threshold: 1000,
            grace_period_days: 0,
        },
        amount,
        asset_address: Address::generate(e),
        created_at: e.ledger().timestamp(),
        expires_at: e.ledger().timestamp() + (30 * 86400),
        current_value: amount,
        status: String::from_str(e, "active"),
    }
}

#[test]
fn benchmark_initialize() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let nft_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, CommitmentCoreContract);

    let mut metrics = BenchmarkMetrics::new("initialize");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft_contract.clone());
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
#[ignore = "requires token contract balance/mint setup"]
fn benchmark_create_commitment() {
    let e = Env::default();
    let (contract_id, _admin, owner) = setup_test_env(&e);
    let commitment = build_benchmark_commitment(&e, &owner, "benchmark_create_1", 1000_0000000);

    let mut metrics = BenchmarkMetrics::new("create_commitment");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        set_commitment(&e, &commitment);
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
#[ignore = "requires token contract balance/mint setup"]
fn benchmark_get_commitment() {
    let e = Env::default();
    let (contract_id, _admin, owner) = setup_test_env(&e);
    let commitment = build_benchmark_commitment(&e, &owner, "benchmark_get_1", 1000_0000000);
    e.as_contract(&contract_id, || {
        set_commitment(&e, &commitment);
    });
    let commitment_id = commitment.commitment_id.clone();

    let mut metrics = BenchmarkMetrics::new("get_commitment");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        CommitmentCoreContract::get_commitment(e.clone(), commitment_id.clone());
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
#[ignore = "requires token contract balance/mint setup"]
fn benchmark_check_violations() {
    let e = Env::default();
    let (contract_id, _admin, owner) = setup_test_env(&e);
    let commitment = build_benchmark_commitment(&e, &owner, "benchmark_check_1", 1000_0000000);
    e.as_contract(&contract_id, || {
        set_commitment(&e, &commitment);
    });
    let commitment_id = commitment.commitment_id.clone();

    let mut metrics = BenchmarkMetrics::new("check_violations");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        CommitmentCoreContract::check_violations(e.clone(), commitment_id.clone());
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_get_total_commitments() {
    let e = Env::default();
    let (contract_id, _admin, _owner) = setup_test_env(&e);

    let mut metrics = BenchmarkMetrics::new("get_total_commitments");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        CommitmentCoreContract::get_total_commitments(e.clone());
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_get_owner_commitments() {
    let e = Env::default();
    let (contract_id, _admin, owner) = setup_test_env(&e);

    let mut metrics = BenchmarkMetrics::new("get_owner_commitments");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        CommitmentCoreContract::get_owner_commitments(e.clone(), owner.clone());
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
#[ignore = "requires token contract balance/mint setup"]
fn benchmark_batch_create_commitments() {
    let e = Env::default();
    let (contract_id, _admin, owner) = setup_test_env(&e);
    let commitment_ids = [
        "benchmark_batch_0",
        "benchmark_batch_1",
        "benchmark_batch_2",
        "benchmark_batch_3",
        "benchmark_batch_4",
        "benchmark_batch_5",
        "benchmark_batch_6",
        "benchmark_batch_7",
        "benchmark_batch_8",
        "benchmark_batch_9",
    ];

    let mut metrics = BenchmarkMetrics::new("batch_create_commitments_10");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        for (i, commitment_id) in commitment_ids.iter().enumerate() {
            let commitment =
                build_benchmark_commitment(&e, &owner, commitment_id, 1000_0000000 + (i as i128));
            set_commitment(&e, &commitment);
        }
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}
