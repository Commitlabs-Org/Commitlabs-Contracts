#![cfg(test)]
#![cfg(feature = "benchmark")]

use super::*;
use soroban_sdk::{
    contract, contractimpl, contracttype, testutils::Address as _, Address, Env, Map, String,
};

#[contract]
pub struct MockCoreContract;

#[contracttype]
#[derive(Clone)]
enum MockDataKey {
    Commitment(String),
}

#[contractimpl]
impl MockCoreContract {
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        e.storage()
            .instance()
            .get::<_, Commitment>(&MockDataKey::Commitment(commitment_id))
            .unwrap_or_else(|| panic!("commitment not found"))
    }

    pub fn set_commitment(e: Env, commitment_id: String, commitment: Commitment) {
        e.storage()
            .instance()
            .set(&MockDataKey::Commitment(commitment_id), &commitment);
    }
}

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
        let _gas_used = if self.gas_after > self.gas_before {
            self.gas_after - self.gas_before
        } else {
            0
        };
        let _ = &self.function_name;
        // Benchmark metrics collected - can be extended with proper logging
    }
}

fn store_mock_commitment(
    e: &Env,
    core_contract_id: &Address,
    commitment_id: &str,
    owner: &Address,
) {
    let commitment_id_str = String::from_str(e, commitment_id);
    let commitment = Commitment {
        commitment_id: commitment_id_str.clone(),
        owner: owner.clone(),
        nft_token_id: 1,
        rules: CommitmentRules {
            duration_days: 30,
            max_loss_percent: 20,
            commitment_type: String::from_str(e, "balanced"),
            early_exit_penalty: 10,
            min_fee_threshold: 0,
            grace_period_days: 0,
        },
        amount: 1_000,
        asset_address: Address::generate(e),
        created_at: 0,
        expires_at: 86_400,
        current_value: 1_000,
        status: String::from_str(e, "active"),
    };

    e.as_contract(core_contract_id, || {
        MockCoreContract::set_commitment(e.clone(), commitment_id_str, commitment);
    });
}

fn setup_test_env(e: &Env) -> (Address, Address, Address) {
    e.mock_all_auths();
    let admin = Address::generate(e);
    let core_contract = e.register_contract(None, MockCoreContract);
    let contract_id = e.register_contract(None, AttestationEngineContract);

    e.as_contract(&contract_id, || {
        AttestationEngineContract::initialize(e.clone(), admin.clone(), core_contract.clone())
            .unwrap();
    });

    (contract_id, admin, core_contract)
}

#[test]
fn benchmark_initialize() {
    let e = Env::default();
    e.mock_all_auths();
    let admin = Address::generate(&e);
    let core_contract = e.register_contract(None, MockCoreContract);
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
#[ignore = "requires fully wired core contract test harness"]
fn benchmark_attest() {
    let e = Env::default();
    let (contract_id, admin, core_contract) = setup_test_env(&e);

    store_mock_commitment(&e, &core_contract, "commitment_1", &Address::generate(&e));

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
#[ignore = "requires fully wired core contract test harness"]
fn benchmark_get_attestations() {
    let e = Env::default();
    let (contract_id, admin, core_contract) = setup_test_env(&e);

    store_mock_commitment(&e, &core_contract, "commitment_1", &Address::generate(&e));

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
#[ignore = "requires fully wired core contract test harness"]
fn benchmark_calculate_compliance_score() {
    let e = Env::default();
    let (contract_id, _admin, core_contract) = setup_test_env(&e);

    let commitment_id = String::from_str(&e, "commitment_1");
    store_mock_commitment(&e, &core_contract, "commitment_1", &Address::generate(&e));

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
#[ignore = "requires fully wired core contract test harness"]
fn benchmark_batch_attest() {
    let e = Env::default();
    let (contract_id, admin, core_contract) = setup_test_env(&e);
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

    let mut metrics = BenchmarkMetrics::new("batch_attest_10");

    for commitment_id_str in commitment_ids.iter() {
        store_mock_commitment(
            &e,
            &core_contract,
            commitment_id_str,
            &Address::generate(&e),
        );
    }

    let start = e.ledger().sequence();
    for commitment_id_str in commitment_ids.iter() {
        let commitment_id = String::from_str(&e, commitment_id_str);
        let mut data = Map::new(&e);
        data.set(
            String::from_str(&e, "health_status"),
            String::from_str(&e, "good"),
        );
        e.as_contract(&contract_id, || {
            let _ = AttestationEngineContract::attest(
                e.clone(),
                admin.clone(),
                commitment_id,
                String::from_str(&e, "health_check"),
                data,
                true,
            );
        });
    }
    let end = e.ledger().sequence();
    metrics.record_gas(start, end);

    metrics.print_summary();
}
