#![no_std]

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentRules {
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String,
    pub early_exit_penalty: u32,
    pub min_fee_threshold: i128,
    pub grace_period_days: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commitment {
    pub commitment_id: String,
    pub owner: Address,
    pub nft_token_id: u32,
    pub rules: CommitmentRules,
    pub amount: i128,
    pub asset_address: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub current_value: i128,
    pub status: String,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Commitment(String),
}

fn default_rules(e: &Env) -> CommitmentRules {
    CommitmentRules {
        duration_days: 30,
        max_loss_percent: 10,
        commitment_type: String::from_str(e, "safe"),
        early_exit_penalty: 15,
        min_fee_threshold: 100,
        grace_period_days: 0,
    }
}

fn default_active_commitment(e: &Env, commitment_id: String) -> Commitment {
    let created_at = e.ledger().timestamp();
    Commitment {
        commitment_id: commitment_id.clone(),
        owner: Address::generate(e),
        nft_token_id: 1,
        rules: default_rules(e),
        amount: 1_000_000,
        asset_address: Address::generate(e),
        created_at,
        expires_at: created_at + 30 * 86_400,
        current_value: 1_000_000,
        status: String::from_str(e, "active"),
    }
}

#[contract]
pub struct MockCommitmentCore;

#[contractimpl]
impl MockCommitmentCore {
    pub fn set_commitment(e: Env, commitment: Commitment) {
        e.storage()
            .instance()
            .set(&DataKey::Commitment(commitment.commitment_id.clone()), &commitment);
    }

    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {
        if let Some(stored) = e
            .storage()
            .instance()
            .get::<_, Commitment>(&DataKey::Commitment(commitment_id.clone()))
        {
            return stored;
        }

        if commitment_id == String::from_str(&e, "unknown") {
            panic!("Commitment not found");
        }

        if commitment_id == String::from_str(&e, "c_expired") {
            let mut commitment = default_active_commitment(&e, commitment_id);
            commitment.status = String::from_str(&e, "expired");
            return commitment;
        }

        if commitment_id == String::from_str(&e, "c_valid") {
            let mut commitment = default_active_commitment(&e, commitment_id);
            commitment.amount = 1000;
            commitment.current_value = 1000;
            return commitment;
        }

        default_active_commitment(&e, commitment_id)
    }
}
