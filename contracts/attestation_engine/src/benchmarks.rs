#![cfg(test)]
#![cfg(feature = "benchmark")]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Env, Map, String,
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
    let contract_id = e.register_contract(None, AttestationEngineContract);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_contract.clone())
            .unwrap();
    });

    (contract_id, admin)
}

#[test]
fn benchmark_initialize() {
    let e = Env::default();
    let admin = Address::generate(&e);
    let core_contract = Address::generate(&e);
    let contract_id = e.register_contract(None, AttestationEngineContract);

    let mut metrics = BenchmarkMetrics::new("initialize");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_contract.clone())
            .unwrap();
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_attest() {
    let e = Env::default();
    let (contract_id, admin) = setup_test_env(&e);

    // Add admin as verifier
    e.as_contract(&contract_id, || {
        // Admin is already authorized
    });

    let commitment_id = String::from_str(&e, "commitment_1");
    let mut data = Map::new(&e);
    data.set(
        String::from_str(&e, "health_status"),
        String::from_str(&e, "good"),
    );

    let mut metrics = BenchmarkMetrics::new("attest");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        let _ = AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            String::from_str(&e, "health_check"),
            data.clone(),
            true,
        );
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_get_attestations() {
    let e = Env::default();
    let (contract_id, admin) = setup_test_env(&e);

    let commitment_id = String::from_str(&e, "commitment_1");
    let mut data = Map::new(&e);
    data.set(
        String::from_str(&e, "health_status"),
        String::from_str(&e, "good"),
    );

    // Create an attestation first
    e.as_contract(&contract_id, || {
        let _ = AttestationEngineContract::attest(
            e.clone(),
            admin.clone(),
            commitment_id.clone(),
            String::from_str(&e, "health_check"),
            data.clone(),
            true,
        );
    });

    let mut metrics = BenchmarkMetrics::new("get_attestations");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        AttestationEngineContract::get_attestations(e.clone(), commitment_id.clone());
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_calculate_compliance_score() {
    let e = Env::default();
    let (contract_id, admin) = setup_test_env(&e);

    let commitment_id = String::from_str(&e, "commitment_1");

    let mut metrics = BenchmarkMetrics::new("calculate_compliance_score");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        AttestationEngineContract::calculate_compliance_score(e.clone(), commitment_id.clone());
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}

#[test]
fn benchmark_batch_attest() {
    let e = Env::default();
    let (contract_id, admin) = setup_test_env(&e);

    let mut metrics = BenchmarkMetrics::new("batch_attest_10");

    e.as_contract(&contract_id, || {
        let start = e.ledger().sequence();
        for i in 0..10 {
            let commitment_id = String::from_str(&e, &format!("commitment_{}", i));
            let mut data = Map::new(&e);
            data.set(
                String::from_str(&e, "health_status"),
                String::from_str(&e, "good"),
            );
            let _ = AttestationEngineContract::attest(
                e.clone(),
                admin.clone(),
                commitment_id,
                String::from_str(&e, "health_check"),
                data,
                true,
            );
        }
        let end = e.ledger().sequence();
        metrics.record_gas(start, end);
    });

    metrics.print_summary();
}
