#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone)]
pub struct Commitment {
    pub commitment_id: String,
    pub owner: Address,
    pub amount: i128,
    pub status: String,
}

#[contract]
pub struct MockCommitmentCore;

#[contractimpl]
impl MockCommitmentCore {
    pub fn get_commitment(e: Env, commitment_id: String) -> Commitment {

        // deterministic responses for tests

        if commitment_id == String::from_str(&e, "c_valid") {
            Commitment {
                commitment_id,
                owner: Address::generate(&e),
                amount: 1000,
                status: String::from_str(&e, "active"),
            }
        } else if commitment_id == String::from_str(&e, "c_expired") {
            Commitment {
                commitment_id,
                owner: Address::generate(&e),
                amount: 1000,
                status: String::from_str(&e, "expired"),
            }
        } else {
            panic!("Commitment not found")
        }
    }
}