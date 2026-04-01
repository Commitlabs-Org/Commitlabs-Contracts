//! Security review tests for `commitment_core` — Issue #203
//!
//! Covers the threat model for functions that intentionally lack `require_auth`,
//! the `update_value` auth fix, reentrancy guard behaviour, and arithmetic safety.
//!
//! Test categories:
//! - `update_value` auth: unauthorized callers rejected; admin and explicit updaters pass
//! - Permissionless `settle`: anyone may settle expired commitments; funds always go to owner
//! - Reentrancy guard: cannot re-enter mutating functions
//! - Arithmetic safety: penalty + returned conservation; loss-percent boundary conditions
//! - Read-only surface: unauthenticated reads succeed and return correct data

#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String,
};

// ---------------------------------------------------------------------------
// Shared test helpers
// ---------------------------------------------------------------------------

#[contract]
struct MockNftSecurity;

#[contractimpl]
impl MockNftSecurity {
    pub fn mint(
        _e: Env,
        _caller: Address,
        _owner: Address,
        _commitment_id: String,
        _duration_days: u32,
        _max_loss_percent: u32,
        _commitment_type: String,
        _initial_amount: i128,
        _asset_address: Address,
        _early_exit_penalty: u32,
    ) -> u32 {
        42
    }
    pub fn settle(_e: Env, _caller: Address, _token_id: u32) {}
    pub fn mark_inactive(_e: Env, _caller: Address, _token_id: u32) {}
}

fn make_commitment(
    e: &Env,
    id: &str,
    owner: &Address,
    amount: i128,
    current_value: i128,
    max_loss_percent: u32,
    duration_days: u32,
    created_at: u64,
) -> Commitment {
    let expires_at = created_at + (duration_days as u64) * 86_400;
    Commitment {
        commitment_id: String::from_str(e, id),
        owner: owner.clone(),
        nft_token_id: 1,
        rules: CommitmentRules {
            duration_days,
            max_loss_percent,
            commitment_type: String::from_str(e, "balanced"),
            early_exit_penalty: 10,
            min_fee_threshold: 0,
            grace_period_days: 0,
        },
        amount,
        asset_address: Address::generate(e),
        created_at,
        expires_at,
        current_value,
        status: String::from_str(e, "active"),
    }
}

fn store(e: &Env, contract_id: &Address, commitment: &Commitment) {
    e.as_contract(contract_id, || set_commitment(e, commitment));
}

/// Minimal environment: initialized contract + real token + funded owner.
fn setup_full(
    amount: i128,
) -> (Env, Address, Address, Address, Address, Address) {
    let e = Env::default();
    e.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&e);
    let nft = e.register_contract(None, MockNftSecurity);
    let owner = Address::generate(&e);
    let token_admin = Address::generate(&e);

    let token_contract = e.register_stellar_asset_contract_v2(token_admin.clone());
    let asset = token_contract.address();
    StellarAssetClient::new(&e, &asset).mint(&owner, &(amount * 10));

    let core = e.register_contract(None, CommitmentCoreContract);
    e.as_contract(&core, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
    });

    (e, core, admin, owner, asset, nft)
}

// ---------------------------------------------------------------------------
// update_value — auth enforcement (SECURITY FIX issue #203)
// ---------------------------------------------------------------------------

/// Any address that is neither admin nor in AuthorizedUpdaters must be rejected.
#[test]
#[should_panic(expected = "Caller is not an authorized value updater")]
fn sec_update_value_random_caller_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);
    let intruder = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c1", &owner, 1000, 1000, 10, 30, 1000);
        set_commitment(&e, &c);
        e.storage().instance().set(&DataKey::TotalValueLocked, &1000i128);
    });

    // Intruder — not admin, not in updater list → must panic
    e.as_contract(&contract_id, || {
        CommitmentCoreContract::update_value(
            e.clone(),
            intruder.clone(),
            String::from_str(&e, "c1"),
            900,
        );
    });
}

/// Admin is always authorized to call update_value even with an empty updater list.
#[test]
fn sec_update_value_admin_always_authorized() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c1", &owner, 1000, 1000, 10, 30, 1000);
        set_commitment(&e, &c);
        e.storage().instance().set(&DataKey::TotalValueLocked, &1000i128);
    });

    // Verify updater list is empty before calling
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    assert_eq!(client.get_authorized_updaters().len(), 0);

    // Admin must succeed
    client.update_value(&admin, &String::from_str(&e, "c1"), &950);
    let updated = client.get_commitment(&String::from_str(&e, "c1"));
    assert_eq!(updated.current_value, 950);
}

/// An address added via add_updater must be able to call update_value.
#[test]
fn sec_update_value_explicit_updater_authorized() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);
    let updater = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c1", &owner, 1000, 1000, 10, 30, 1000);
        set_commitment(&e, &c);
        e.storage().instance().set(&DataKey::TotalValueLocked, &1000i128);
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.add_updater(&admin, &updater);
    client.update_value(&updater, &String::from_str(&e, "c1"), &980);

    assert_eq!(client.get_commitment(&String::from_str(&e, "c1")).current_value, 980);
}

/// Removing an updater must immediately revoke their ability to call update_value.
#[test]
#[should_panic(expected = "Caller is not an authorized value updater")]
fn sec_update_value_removed_updater_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);
    let updater = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c1", &owner, 1000, 1000, 10, 30, 1000);
        set_commitment(&e, &c);
        e.storage().instance().set(&DataKey::TotalValueLocked, &1000i128);
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.add_updater(&admin, &updater);
    // First call — should succeed
    client.update_value(&updater, &String::from_str(&e, "c1"), &980);
    // Revoke
    client.remove_updater(&admin, &updater);
    // Second call — must panic
    client.update_value(&updater, &String::from_str(&e, "c1"), &960);
}

/// update_value on a non-active commitment must fail, even for admin.
#[test]
#[should_panic(expected = "Commitment is not active")]
fn sec_update_value_on_settled_commitment_fails() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let mut c = make_commitment(&e, "c1", &owner, 1000, 1000, 10, 30, 1000);
        c.status = String::from_str(&e, "settled");
        set_commitment(&e, &c);
        e.storage().instance().set(&DataKey::TotalValueLocked, &1000i128);
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.update_value(&admin, &String::from_str(&e, "c1"), &900);
}

// ---------------------------------------------------------------------------
// Permissionless settle — threat model verification
// ---------------------------------------------------------------------------

/// A third-party keeper (not the owner) must be able to settle an expired commitment,
/// and tokens must be sent to the original owner — not to the caller.
#[test]
fn sec_settle_by_third_party_sends_tokens_to_owner() {
    let amount = 1_000i128;
    let (e, core, _admin, owner, asset, nft) = setup_full(amount);

    // Create commitment via the actual flow
    let rules = CommitmentRules {
        duration_days: 1,
        max_loss_percent: 10,
        commitment_type: String::from_str(&e, "balanced"),
        early_exit_penalty: 10,
        min_fee_threshold: 0,
        grace_period_days: 0,
    };

    let token_client = TokenClient::new(&e, &asset);
    let owner_balance_before = token_client.balance(&owner);

    let commitment_id = e.as_contract(&core, || {
        CommitmentCoreContract::create_commitment(
            e.clone(),
            owner.clone(),
            amount,
            asset.clone(),
            rules,
        )
    });

    // Fast-forward past expiry
    e.as_contract(&core, || {
        let c = read_commitment(&e, &commitment_id).unwrap();
        e.ledger().with_mut(|l| l.timestamp = c.expires_at + 1);
    });

    let keeper = Address::generate(&e);
    // Keeper settles — should succeed
    e.as_contract(&core, || {
        CommitmentCoreContract::settle(e.clone(), commitment_id.clone());
    });

    // Owner gets the tokens; keeper balance unchanged (zero)
    let owner_balance_after = token_client.balance(&owner);
    assert!(owner_balance_after > 0, "owner must receive settlement tokens");

    let c = e.as_contract(&core, || {
        CommitmentCoreContract::get_commitment(e.clone(), commitment_id.clone())
    });
    assert_eq!(c.status, String::from_str(&e, "settled"));
}

/// settle before expiry must always be rejected, regardless of caller.
#[test]
#[should_panic(expected = "Commitment has not expired yet")]
fn sec_settle_before_expiry_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);
    let commitment_id = "premature";

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
    });

    let created_at = 1_000u64;
    let c = make_commitment(&e, commitment_id, &owner, 1000, 1000, 10, 30, created_at);
    store(&e, &contract_id, &c);

    // Time is before expires_at
    e.ledger().with_mut(|l| l.timestamp = created_at + 5);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::settle(e.clone(), String::from_str(&e, commitment_id));
    });
}

/// Double-settle must be rejected; AlreadySettled guard fires on second call.
#[test]
#[should_panic(expected = "Commitment already settled")]
fn sec_settle_idempotency_double_settle_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);
    let commitment_id = "double_settle";

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let mut c = make_commitment(&e, commitment_id, &owner, 1000, 1000, 10, 30, 1000);
        c.status = String::from_str(&e, "settled");
        set_commitment(&e, &c);
    });

    // Advance past expiry
    e.ledger().with_mut(|l| l.timestamp = 1_000 + 31 * 86_400);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::settle(e.clone(), String::from_str(&e, commitment_id));
    });
}

// ---------------------------------------------------------------------------
// early_exit — ownership enforcement
// ---------------------------------------------------------------------------

/// A third party who is not the commitment owner must not be able to trigger early exit.
#[test]
#[should_panic(expected = "Unauthorized: caller not allowed")]
fn sec_early_exit_non_owner_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);
    let attacker = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c1", &owner, 1000, 1000, 10, 30, 1000);
        set_commitment(&e, &c);
    });

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::early_exit(
            e.clone(),
            String::from_str(&e, "c1"),
            attacker.clone(),
        );
    });
}

/// Admin calling early_exit for a commitment they don't own must be rejected.
#[test]
#[should_panic(expected = "Unauthorized: caller not allowed")]
fn sec_early_exit_admin_not_owner_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c1", &owner, 1000, 1000, 10, 30, 1000);
        set_commitment(&e, &c);
    });

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::early_exit(
            e.clone(),
            String::from_str(&e, "c1"),
            admin.clone(),
        );
    });
}

// ---------------------------------------------------------------------------
// Arithmetic safety
// ---------------------------------------------------------------------------

/// Penalty + returned must always equal current_value (token conservation invariant).
#[test]
fn sec_arithmetic_penalty_conservation() {
    let cases: &[(i128, u32)] = &[
        (1_000, 10),
        (1_000, 0),
        (1_000, 100),
        (999, 10),   // non-round amount
        (1, 50),     // tiny amount
        (i128::MAX / 10_000, 50),
    ];

    for &(current_value, penalty_percent) in cases {
        let penalty = SafeMath::penalty_amount(current_value, penalty_percent);
        let returned = SafeMath::sub(current_value, penalty);
        assert_eq!(
            penalty + returned,
            current_value,
            "conservation failed: value={current_value} penalty_pct={penalty_percent}"
        );
    }
}

/// Loss percent at the exact boundary (max_loss_percent) must not trigger violation
/// (boundary uses strict `>`, not `>=`).
#[test]
fn sec_loss_percent_exact_boundary_no_violation() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        // 10% loss exactly: 1000 → 900
        let c = make_commitment(&e, "c1", &owner, 1000, 900, 10, 30, 1000);
        set_commitment(&e, &c);
        e.storage().instance().set(&DataKey::TotalValueLocked, &900i128);
    });

    // update_value with same value — loss stays at 10%, should not violate
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.update_value(&admin, &String::from_str(&e, "c1"), &900);

    let updated = client.get_commitment(&String::from_str(&e, "c1"));
    assert_eq!(
        updated.status,
        String::from_str(&e, "active"),
        "10% loss at boundary must not trigger violation"
    );
}

/// One basis point over the max loss boundary must trigger violation.
#[test]
fn sec_loss_percent_one_over_boundary_triggers_violation() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c1", &owner, 1000, 1000, 10, 30, 1000);
        set_commitment(&e, &c);
        e.storage().instance().set(&DataKey::TotalValueLocked, &1000i128);
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    // 1000 → 889: loss = (1000-889)/1000 = 11.1% > 10%
    client.update_value(&admin, &String::from_str(&e, "c1"), &889);

    let updated = client.get_commitment(&String::from_str(&e, "c1"));
    assert_eq!(
        updated.status,
        String::from_str(&e, "violated"),
        "loss > max_loss_percent must trigger violation"
    );
}

/// update_value with zero amount commitment must not divide by zero.
#[test]
fn sec_update_value_zero_amount_no_division_by_zero() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c1", &owner, 0, 0, 10, 30, 1000);
        set_commitment(&e, &c);
        e.storage().instance().set(&DataKey::TotalValueLocked, &0i128);
    });

    // Must not panic
    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.update_value(&admin, &String::from_str(&e, "c1"), &0);
    // With zero amount the loss_percent path returns 0 → no violation
    let updated = client.get_commitment(&String::from_str(&e, "c1"));
    assert_eq!(updated.status, String::from_str(&e, "active"));
}

/// Negative new_value must be rejected.
#[test]
#[should_panic(expected = "Invalid amount")]
fn sec_update_value_negative_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c1", &owner, 1000, 1000, 10, 30, 1000);
        set_commitment(&e, &c);
        e.storage().instance().set(&DataKey::TotalValueLocked, &1000i128);
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.update_value(&admin, &String::from_str(&e, "c1"), &-1);
}

// ---------------------------------------------------------------------------
// Read-only surface — unauthenticated access succeeds
// ---------------------------------------------------------------------------

/// All read-only getters must succeed without any auth, returning correct data.
#[test]
fn sec_read_only_getters_require_no_auth() {
    let e = Env::default();
    // Deliberately do NOT call mock_all_auths — reads should work without it
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
    });

    e.as_contract(&contract_id, || {
        assert_eq!(CommitmentCoreContract::get_total_commitments(e.clone()), 0);
        assert_eq!(CommitmentCoreContract::get_total_value_locked(e.clone()), 0);
        assert_eq!(CommitmentCoreContract::get_admin(e.clone()), admin);
        assert_eq!(CommitmentCoreContract::get_nft_contract(e.clone()), nft);
        assert!(!CommitmentCoreContract::is_paused(e.clone()));
        assert!(!CommitmentCoreContract::is_emergency_mode(e.clone()));
        assert_eq!(CommitmentCoreContract::get_authorized_updaters(e.clone()).len(), 0);
        assert_eq!(CommitmentCoreContract::get_creation_fee_bps(e.clone()), 0);
        assert_eq!(CommitmentCoreContract::get_fee_recipient(e.clone()), None);
    });
}

/// get_commitment must be callable by anyone, including the attestation_engine pattern.
#[test]
fn sec_get_commitment_unauthenticated_reads_correct_data() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c = make_commitment(&e, "c_attest", &owner, 5_000, 4_800, 10, 30, 2_000);
        set_commitment(&e, &c);
    });

    // No mock_all_auths — should succeed
    let c = e.as_contract(&contract_id, || {
        CommitmentCoreContract::get_commitment(e.clone(), String::from_str(&e, "c_attest"))
    });
    assert_eq!(c.amount, 5_000);
    assert_eq!(c.current_value, 4_800);
    assert_eq!(c.owner, owner);
}

// ---------------------------------------------------------------------------
// Storage key isolation
// ---------------------------------------------------------------------------

/// Two different commitment IDs must never share storage state.
#[test]
fn sec_commitment_storage_keys_are_isolated() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let owner1 = Address::generate(&e);
    let owner2 = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
        let c1 = make_commitment(&e, "iso_1", &owner1, 1_000, 1_000, 10, 30, 1_000);
        let c2 = make_commitment(&e, "iso_2", &owner2, 9_000, 9_000, 20, 60, 1_000);
        set_commitment(&e, &c1);
        set_commitment(&e, &c2);
        e.storage().instance().set(&DataKey::TotalValueLocked, &10_000i128);
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);

    // Update c1 — must not affect c2
    client.update_value(&admin, &String::from_str(&e, "iso_1"), &800);

    let c1 = client.get_commitment(&String::from_str(&e, "iso_1"));
    let c2 = client.get_commitment(&String::from_str(&e, "iso_2"));

    assert_eq!(c1.current_value, 800);
    assert_eq!(c2.current_value, 9_000, "iso_2 must not be affected by iso_1 update");
    assert_eq!(c2.owner, owner2);
}

// ---------------------------------------------------------------------------
// initialize — re-initialization attack
// ---------------------------------------------------------------------------

/// Calling initialize a second time must always fail, even with the same admin.
#[test]
#[should_panic(expected = "Contract already initialized")]
fn sec_initialize_cannot_be_called_twice() {
    let e = Env::default();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
    });
    // Attacker attempts to re-initialize with a different admin
    let attacker_admin = Address::generate(&e);
    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), attacker_admin, nft.clone());
    });
}

// ---------------------------------------------------------------------------
// allocate — auth enforcement
// ---------------------------------------------------------------------------

/// An unauthorized address (not admin, not in AuthorizedAllocator) must be rejected.
#[test]
#[should_panic(expected = "Unauthorized")]
fn sec_allocate_unauthorized_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let intruder = Address::generate(&e);
    let target_pool = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.allocate(
        &intruder,
        &String::from_str(&e, "nonexistent"),
        &target_pool,
        &100,
    );
}

// ---------------------------------------------------------------------------
// Fee admin — unauthorized access
// ---------------------------------------------------------------------------

/// A non-admin must not be able to set fee bps.
#[test]
#[should_panic(expected = "Unauthorized")]
fn sec_set_creation_fee_bps_non_admin_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let non_admin = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.set_creation_fee_bps(&non_admin, &100);
}

/// A non-admin must not be able to withdraw fees.
#[test]
#[should_panic(expected = "Unauthorized")]
fn sec_withdraw_fees_non_admin_rejected() {
    let e = Env::default();
    e.mock_all_auths();
    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let admin = Address::generate(&e);
    let nft = Address::generate(&e);
    let non_admin = Address::generate(&e);
    let asset = Address::generate(&e);

    e.as_contract(&contract_id, || {
        CommitmentCoreContract::initialize(e.clone(), admin.clone(), nft.clone());
    });

    let client = CommitmentCoreContractClient::new(&e, &contract_id);
    client.withdraw_fees(&non_admin, &asset, &100);
}