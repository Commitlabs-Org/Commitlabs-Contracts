#![cfg(test)]

//! Deterministic metric consistency tests for issue #549.
//!
//! The mock core contract supplies the canonical commitment record so these
//! tests exercise the real attestation writers and storage aggregation paths.

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, Address, Env, Map, String};

#[contract]
struct MockCore;

#[contractimpl]
impl MockCore {
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        Commitment {
            commitment_id,
            owner: Address::generate(&e),
            nft_token_id: 1,
            rules: CommitmentRules {
                duration_days: 30,
                max_loss_percent: 25,
                commitment_type: String::from_str(&e, "balanced"),
                early_exit_penalty: 10,
                min_fee_threshold: 1_000,
                grace_period_days: 0,
            },
            amount: 1_000,
            asset_address: Address::generate(&e),
            created_at: 0,
            expires_at: 30 * 86_400,
            current_value: 1_000,
            status: String::from_str(&e, "active"),
        }
    }
}

struct Fixture {
    env: Env,
    client: AttestationEngineContractClient<'static>,
    contract_id: Address,
    admin: Address,
    commitment_id: String,
}

fn fixture() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
    let core_id = env.register_contract(None, MockCore);
    let contract_id = env.register_contract(None, AttestationEngineContract);
    let client = AttestationEngineContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let commitment_id = String::from_str(&env, "metric-consistency");
    client.initialize(&admin, &core_id);
    Fixture {
        env,
        client,
        contract_id,
        admin,
        commitment_id,
    }
}

fn fee(fixture: &Fixture, amount: i128) {
    fixture
        .client
        .record_fees(&fixture.admin, &fixture.commitment_id, &amount);
}

fn drawdown(fixture: &Fixture, amount: i128) {
    fixture
        .client
        .record_drawdown(&fixture.admin, &fixture.commitment_id, &amount);
}

fn violation_data(env: &Env, severity: &str) -> Map<String, String> {
    let mut data = Map::new(env);
    data.set(
        String::from_str(env, "violation_type"),
        String::from_str(env, "policy"),
    );
    data.set(
        String::from_str(env, "severity"),
        String::from_str(env, severity),
    );
    data
}

#[test]
fn empty_history_has_bounded_baseline_metrics() {
    let fixture = fixture();
    let metrics = fixture
        .client
        .get_health_metrics(&fixture.commitment_id);

    assert_eq!(metrics.fees_generated, 0);
    assert_eq!(metrics.volatility_exposure, 0);
    assert_eq!(metrics.last_attestation, 0);
    assert!(metrics.compliance_score <= MAX_COMPLIANCE_SCORE);
}

#[test]
fn negative_and_out_of_range_records_are_rejected_without_history_changes() {
    let fixture = fixture();

    assert_eq!(
        fixture
            .client
            .try_record_fees(&fixture.admin, &fixture.commitment_id, &-1),
        Err(Ok(AttestationError::InvalidFeeAmount))
    );
    assert_eq!(
        fixture
            .client
            .try_record_drawdown(&fixture.admin, &fixture.commitment_id, &101),
        Err(Ok(AttestationError::InvalidAttestationData))
    );
    assert_eq!(
        fixture
            .client
            .try_record_drawdown(&fixture.admin, &fixture.commitment_id, &-1),
        Err(Ok(AttestationError::InvalidAttestationData))
    );
    assert_eq!(fixture.client.get_attestation_count(&fixture.commitment_id), 0);
    assert!(fixture
        .client
        .get_stored_health_metrics(&fixture.commitment_id)
        .is_none());
}

#[test]
fn equivalent_fee_sequences_produce_the_same_total() {
    let first = fixture();
    let second = fixture();

    fee(&first, 10);
    fee(&first, 20);
    fee(&second, 20);
    fee(&second, 10);

    let first_metrics = first
        .client
        .get_stored_health_metrics(&first.commitment_id)
        .unwrap();
    let second_metrics = second
        .client
        .get_stored_health_metrics(&second.commitment_id)
        .unwrap();
    assert_eq!(first_metrics.fees_generated, 30);
    assert_eq!(second_metrics.fees_generated, 30);
    assert_eq!(first.client.get_attestation_count(&first.commitment_id), 2);
    assert_eq!(second.client.get_attestation_count(&second.commitment_id), 2);
}

#[test]
fn repeated_reads_do_not_change_metric_totals_or_counts() {
    let fixture = fixture();
    fee(&fixture, 75);
    drawdown(&fixture, 12);

    let before_count = fixture.client.get_attestation_count(&fixture.commitment_id);
    let first = fixture.client.get_health_metrics(&fixture.commitment_id);
    let second = fixture.client.get_health_metrics(&fixture.commitment_id);

    assert_eq!(first, second);
    assert_eq!(first.fees_generated, 75);
    assert_eq!(first.drawdown_percent, 12);
    assert_eq!(
        fixture.client.get_attestation_count(&fixture.commitment_id),
        before_count
    );
}

#[test]
fn mixed_history_keeps_score_and_percentages_in_documented_bounds() {
    let fixture = fixture();
    drawdown(&fixture, 0);
    drawdown(&fixture, 25);
    for _ in 0..8 {
        let data = violation_data(&fixture.env, "high");
        fixture.client.attest(
            &fixture.admin,
            &fixture.commitment_id,
            &String::from_str(&fixture.env, "violation"),
            &data,
            &false,
        );
    }

    let metrics = fixture
        .client
        .get_stored_health_metrics(&fixture.commitment_id)
        .unwrap();
    assert!(metrics.compliance_score <= MAX_COMPLIANCE_SCORE);
    assert!(metrics.drawdown_percent >= 0);
    assert!(metrics.drawdown_percent <= MAX_PERCENT);
    assert_eq!(fixture.client.get_attestation_count(&fixture.commitment_id), 10);
}

#[test]
fn maximum_valid_drawdown_is_accepted_and_preserved() {
    let fixture = fixture();
    drawdown(&fixture, MAX_PERCENT);

    let metrics = fixture
        .client
        .get_stored_health_metrics(&fixture.commitment_id)
        .unwrap();
    assert_eq!(metrics.drawdown_percent, MAX_PERCENT);
    assert!(metrics.compliance_score <= MAX_COMPLIANCE_SCORE);
}

#[test]
fn fee_accumulator_overflow_rejects_the_record_atomically() {
    let fixture = fixture();
    fixture.env.as_contract(&fixture.contract_id, || {
        fixture
            .env
            .storage()
            .instance()
            .set(&DataKey::TotalFees, &i128::MAX);
    });

    assert_eq!(
        fixture
            .client
            .try_record_fees(&fixture.admin, &fixture.commitment_id, &1),
        Err(Ok(AttestationError::StorageError))
    );
    assert_eq!(fixture.client.get_attestation_count(&fixture.commitment_id), 0);
    fixture.env.as_contract(&fixture.contract_id, || {
        assert_eq!(
            fixture
                .env
                .storage()
                .instance()
                .get::<DataKey, i128>(&DataKey::TotalFees),
            Some(i128::MAX)
        );
    });
}

#[test]
fn rejected_generic_fee_payload_does_not_change_metrics() {
    let fixture = fixture();
    let mut data = Map::new(&fixture.env);
    data.set(
        String::from_str(&fixture.env, "fee_amount"),
        String::from_str(&fixture.env, "-5"),
    );

    assert_eq!(
        fixture.client.try_attest(
            &fixture.admin,
            &fixture.commitment_id,
            &String::from_str(&fixture.env, "fee_generation"),
            &data,
            &true,
        ),
        Err(Ok(AttestationError::InvalidFeeAmount))
    );
    assert_eq!(fixture.client.get_attestation_count(&fixture.commitment_id), 0);
}
