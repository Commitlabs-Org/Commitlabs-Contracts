use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockCoreCommitmentRules {
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String,
    pub early_exit_penalty: u32,
    pub min_fee_threshold: i128,
    pub grace_period_days: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MockCoreCommitment {
    pub commitment_id: String,
    pub owner: Address,
    pub nft_token_id: u32,
    pub rules: MockCoreCommitmentRules,
    pub amount: i128,
    pub asset_address: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub current_value: i128,
    pub status: String,
}

#[contract]
pub struct MockCommitmentCore;

#[contractimpl]
impl MockCommitmentCore {
    pub fn get_commitment(e: Env, commitment_id: String) -> MockCoreCommitment {
        let rules = MockCoreCommitmentRules {
            duration_days: 30,
            max_loss_percent: 10,
            commitment_type: String::from_str(&e, "test"),
            early_exit_penalty: 5,
            min_fee_threshold: 0,
            grace_period_days: 0,
        };

        // Standard user address (index 2 in tests)
        let user_addr = Address::from_string(&String::from_str(&e, "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHK3M"));
        // Default address for other cases
        let default_addr = Address::from_string(&String::from_str(&e, "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAITA4"));

        // We can't use to_string() or starts_with() easily on soroban_sdk::String.
        // But we can compare with known prefixes if we really want to.
        // However, for tests, we'll just use a few checks.
        
        let mut owner = default_addr;
        
        // If it starts with 'c_', it's likely a user-owned commitment in our tests.
        // Since we can't easily check 'starts_with', we'll just check if it's NOT 'unknown'.
        if commitment_id != String::from_str(&e, "unknown") {
            owner = user_addr;
        }

        if commitment_id == String::from_str(&e, "c_expired") {
            MockCoreCommitment {
                commitment_id,
                owner,
                nft_token_id: 1,
                rules,
                amount: 1000,
                asset_address: Address::from_string(&String::from_str(&e, "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAK3IM")),
                created_at: 0,
                expires_at: 1000,
                current_value: 1000,
                status: String::from_str(&e, "expired"),
            }
        } else if commitment_id == String::from_str(&e, "unknown") {
             panic!("Commitment not found")
        } else {
            MockCoreCommitment {
                commitment_id,
                owner,
                nft_token_id: 1,
                rules,
                amount: 1000,
                asset_address: Address::from_string(&String::from_str(&e, "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAK3IM")),
                created_at: 0,
                expires_at: 1000,
                current_value: 1000,
                status: String::from_str(&e, "active"),
            }
        }
    }
}
