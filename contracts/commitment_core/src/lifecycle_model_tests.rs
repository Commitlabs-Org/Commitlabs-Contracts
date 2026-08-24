#![cfg(test)]

//! Bounded deterministic stateful model coverage for the commitment lifecycle.
//!
//! Issue #554. A compact reference model mirrors the economically relevant state of
//! [`crate::CommitmentCoreContract`] (per-commitment principal/value/status, TVL,
//! collected fees, token custody balances). A seeded generator emits bounded command
//! sequences over the real entrypoints (`create_commitment`, `update_value`, `settle`,
//! `early_exit`, `allocate`, fee management, ledger-time advance), including invalid
//! variants: wrong state, wrong actor, duplicate terminal operations, boundary amounts,
//! insufficient funding.
//!
//! After every executed command - valid or invalid - the harness compares the whole
//! observable contract state against the reference model and checks the invariants:
//!
//! - I1 principal conservation: contract token custody equals locked principal plus
//!   held fees, and tracked balances always sum to the minted supply.
//! - I2 fee conservation: collected fees == creation fees + exit penalties -
//!   withdrawals.
//! - I3 ownership/auth: only permitted actors succeed for each operation.
//! - I4 terminal immutability: `violated`, `settled`, `early_exit` reject all
//!   active-only flows and never move principal or fees twice.
//! - I5 invalid-command atomicity: state after an invalid command equals state before.
//! - I6 determinism: fixed seeds, fixed addresses, no entropy sources.
//!
//! Any mismatch prints the seed, the original sequence, and a greedily minimized
//! sequence. A failing seed can be replayed alone with:
//! `CGQA_LIFECYCLE_SEED=<seed> cargo test -p commitment_core --lib lifecycle_model`

extern crate std;

use crate::{CommitmentCoreContract, CommitmentCoreContractClient, CommitmentRules};
use soroban_sdk::{
    contract, contractimpl,
    testutils::Ledger,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String,
};

// ---------------------------------------------------------------------------
// Deterministic fixture addresses.
//
// Valid StrKey *contract* addresses (`C...`), fixed forever so runs, digests,
// and snapshots are reproducible. Contract-style addresses are used because the
// built-in Stellar Asset Contract requires account-style recipients to hold a
// trustline, while `Address::generate()` in testutils produces contract
// addresses (which need none) - fixed `C` strkeys give the same behavior with
// reproducible values.
// ---------------------------------------------------------------------------

const ADMIN_STR: &str = "CACRAGZGGE6EOUS5NBZX5CMUT6VLLQGL23Q6Z5YCBUMCGLRZIRHVUS34";
const UPDATER_STR: &str = "CAVDKQCLKZQWY54CRWMKHLVZYTH5VZPQ7MDBCHBHGI6UQU26NF2H7JQM";
const ALLOCATOR_STR: &str = "CBHVUZLQPODJDHFHWK64RU665H2P6CQVEAVTMQKMK5RG26EDR2M2JE3P";
const OUTSIDER_STR: &str = "CB2H7CUVUCV3NQOM27RO36ADBYMSILZ2IVIFWZTRPSDZFHNIWO7MTHFQ";
const OWNER_STRS: [&str; 3] = [
    "CCM2JL52YXINXZXR7QDREHJIGM7ESVC7NJ2YBC4WUGWLPQWN3DR65MAC",
    "CC7MTVG75L2QACYWEEWDOQSNLBRW46MER6NKLMF3Y3I5ZZ7S7UEBGE7B",
    "CDR656IEB4NCKMB3IZIVYZ3SPWEJHHVJWS74VVPA5P3ACDAXEIWTQD4A",
];
const RECIPIENT_STR: &str = "CAEBGHRJGQ7UUVLANN3IDDEXUKW3RQ6O3HSO76QFCANSMMJ4I5JF2YTH";
const POOL_STR: &str = "CAWTQQ2OLFSG66UFSCN2NMN4Y7JN32HT7YERIHZKGVAEWVTBNR3YE65Y";
const TOKEN_ADMIN_STR: &str = "CBJF22DTP2EZJH5KWXAMXVXB5T3QEDIYEMXDSRCPLJSXA64GSGOKP3RO";

fn addr(e: &Env, s: &str) -> Address {
    Address::from_string(&String::from_str(e, s))
}

// ---------------------------------------------------------------------------
// Mock NFT (same shape as the existing unit-test/fuzz mocks).
// ---------------------------------------------------------------------------

#[contract]
struct LifecycleModelNft;

#[contractimpl]
impl LifecycleModelNft {
    #[allow(clippy::too_many_arguments)]
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
        1
    }

    pub fn settle(_e: Env, _caller: Address, _token_id: u32) {}

    pub fn mark_inactive(_e: Env, _caller: Address, _token_id: u32) {}
}

// ---------------------------------------------------------------------------
// Seeded PRNG (xorshift64*) - no external dependencies, fully deterministic.
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }
}

// ---------------------------------------------------------------------------
// Reference model
// ---------------------------------------------------------------------------

const START_TS: u64 = 1_700_000_000;
const DAY: u64 = 86_400;
const OWNER_MINT: i128 = 1_000_000_000_000_000_000; // 1e18 per owner
const MAX_SLOTS: usize = 24;
const SEQ_LEN: usize = 20;
const SEED_COUNT: u64 = 40;
const BASE_SEED: u64 = 0x5EED_0000_0000_0001;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum St {
    Active,
    Violated,
    Settled,
    EarlyExit,
}

impl St {
    fn as_str(self) -> &'static str {
        match self {
            St::Active => "active",
            St::Violated => "violated",
            St::Settled => "settled",
            St::EarlyExit => "early_exit",
        }
    }
}

#[derive(Clone, Copy)]
struct Slot {
    counter: u64, // commitment number; id is `COMMIT_<counter>`
    owner: usize,
    amount: i128,   // net principal stored on-chain
    value: i128,    // current_value
    released: i128, // cumulative principal/value paid out of custody for this slot
    expires_at: u64,
    max_loss: u32,
    penalty: u32,
    st: St,
}

struct Model {
    slots: [Option<Slot>; MAX_SLOTS],
    counter: u64,
    tvl: i128,
    fees: i128,
    fee_bps: u32,
    now: u64,
    owner_lists: [usize; 3],
    bal_owner: [i128; 3],
    bal_contract: i128,
    bal_recipient: i128,
    bal_pool: i128,
}

impl Model {
    fn new() -> Self {
        Model {
            slots: Default::default(),
            counter: 0,
            tvl: 0,
            fees: 0,
            fee_bps: 0,
            now: START_TS,
            owner_lists: [0; 3],
            bal_owner: [OWNER_MINT; 3],
            bal_contract: 0,
            bal_recipient: 0,
            bal_pool: 0,
        }
    }

    /// Mirrors `SafeMath::loss_percent`.
    fn loss_percent(initial: i128, current: i128) -> i128 {
        if initial <= 0 {
            return 0;
        }
        if current >= initial {
            return 0;
        }
        if current <= 0 {
            return 100;
        }
        (initial - current) * 100 / initial
    }

    fn free_slot(&self) -> Option<usize> {
        (0..MAX_SLOTS).find(|&i| self.slots[i].is_none())
    }

    /// Number of ids an owner index list should hold. Only `settle` removes the
    /// owner-index entry; `violated` and `early_exit` records remain listed.
    fn expected_owner_list(&self, owner: usize) -> usize {
        self.slots
            .iter()
            .filter(|s| matches!(s, Some(sl) if sl.owner == owner && sl.st != St::Settled))
            .count()
    }
}

/// Mirrors `shared_utils::fees::fee_from_bps` for modeled ranges (no overflow).
fn fee_from_bps(amount: i128, bps: u32) -> i128 {
    amount * bps as i128 / 10_000
}

fn commit_id(e: &Env, counter: u64) -> String {
    String::from_str(e, &std::format!("COMMIT_{}", counter))
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Actor {
    Authorized,
    Outsider,
}

#[derive(Clone, Copy, Debug)]
enum Cmd {
    /// `create_commitment` ("fund" is part of create: tokens move in atomically).
    Create {
        owner: usize,
        amount_cls: usize,
        rules: usize,
    },
    /// `update_value` (oracle valuation; may persist `violated`).
    UpdateValue {
        slot: usize,
        value_cls: usize,
        actor: Actor,
    },
    /// `settle` (permissionless; requires expiry).
    Settle { slot: usize },
    /// `early_exit` (owner-only cancel analogue with penalty).
    EarlyExit { slot: usize, wrong_caller: bool },
    /// `allocate` (allocator-only partial principal release to a pool).
    Allocate {
        slot: usize,
        amount_cls: usize,
        actor: Actor,
    },
    /// `set_creation_fee_bps` (treasurer/admin config).
    SetFeeBps { bps: u32, outsider: bool },
    /// `withdraw_fees` (treasurer/admin).
    WithdrawFees { amount_cls: usize, outsider: bool },
    /// Ledger timestamp advance (drives expiry).
    AdvanceTime { days: u32 },
}

/// Last class intentionally exceeds any owner balance (insufficient funding).
const CREATE_AMOUNTS: [i128; 5] = [
    1_000,
    999_983,
    10_000_000_000,
    1_000_000_000_000_000,
    2_000_000_000_000_000_000_000, // 2e21 > 1e18 minted balance -> invalid
];

/// (duration_days, max_loss_percent, early_exit_penalty, type) - all valid combos.
const RULE_SETS: [(u32, u32, u32, &str); 3] = [
    (1, 10, 15, "safe"),
    (7, 30, 10, "balanced"),
    (30, 100, 5, "aggressive"),
];

const VALUE_CLASSES: usize = 6; // same, half, +1, zero, tiny(1), deep-loss(amount/2)
const ALLOC_CLASSES: usize = 4; // half, full, over(full+1), zero
const WITHDRAW_CLASSES: usize = 3; // exact, half, excess(+1)

fn cmd_debug(c: &Cmd) -> std::string::String {
    match *c {
        Cmd::Create {
            owner,
            amount_cls,
            rules,
        } => std::format!(
            "Create(owner=O{}, amount={}, rules={})",
            owner,
            CREATE_AMOUNTS[amount_cls],
            RULE_SETS[rules].3
        ),
        Cmd::UpdateValue {
            slot,
            value_cls,
            actor,
        } => std::format!(
            "UpdateValue(slot={}, value_cls={}, actor={:?})",
            slot,
            value_cls,
            actor
        ),
        Cmd::Settle { slot } => std::format!("Settle(slot={})", slot),
        Cmd::EarlyExit { slot, wrong_caller } => {
            std::format!("EarlyExit(slot={}, wrong_caller={})", slot, wrong_caller)
        }
        Cmd::Allocate {
            slot,
            amount_cls,
            actor,
        } => {
            std::format!(
                "Allocate(slot={}, amount_cls={}, actor={:?})",
                slot,
                amount_cls,
                actor
            )
        }
        Cmd::SetFeeBps { bps, outsider } => {
            std::format!("SetFeeBps(bps={}, outsider={})", bps, outsider)
        }
        Cmd::WithdrawFees {
            amount_cls,
            outsider,
        } => {
            std::format!(
                "WithdrawFees(amount_cls={}, outsider={})",
                amount_cls,
                outsider
            )
        }
        Cmd::AdvanceTime { days } => std::format!("AdvanceTime(days={})", days),
    }
}

fn seq_debug(cmds: &[Cmd]) -> std::string::String {
    cmds.iter()
        .map(cmd_debug)
        .collect::<std::vec::Vec<_>>()
        .join(";\n  ")
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

fn generate_commands(rng: &mut Rng, len: usize) -> std::vec::Vec<Cmd> {
    let mut out = std::vec::Vec::new();
    for _ in 0..len {
        let pick = rng.below(100);
        let cmd = if pick < 28 {
            Cmd::Create {
                owner: rng.below(3),
                amount_cls: if rng.chance(12) { 4 } else { rng.below(4) },
                rules: rng.below(3),
            }
        } else if pick < 44 {
            Cmd::UpdateValue {
                slot: rng.below(MAX_SLOTS),
                value_cls: rng.below(VALUE_CLASSES),
                actor: if rng.chance(30) {
                    Actor::Outsider
                } else {
                    Actor::Authorized
                },
            }
        } else if pick < 58 {
            Cmd::Settle {
                slot: rng.below(MAX_SLOTS),
            }
        } else if pick < 72 {
            Cmd::EarlyExit {
                slot: rng.below(MAX_SLOTS),
                wrong_caller: rng.chance(30),
            }
        } else if pick < 80 {
            Cmd::Allocate {
                slot: rng.below(MAX_SLOTS),
                amount_cls: rng.below(ALLOC_CLASSES),
                actor: if rng.chance(30) {
                    Actor::Outsider
                } else {
                    Actor::Authorized
                },
            }
        } else if pick < 85 {
            Cmd::SetFeeBps {
                bps: [0, 100, 250][rng.below(3)],
                outsider: rng.chance(25),
            }
        } else if pick < 90 {
            Cmd::WithdrawFees {
                amount_cls: rng.below(WITHDRAW_CLASSES),
                outsider: rng.chance(25),
            }
        } else {
            Cmd::AdvanceTime {
                days: [0, 1, 3, 10, 40][rng.below(5)],
            }
        };
        out.push(cmd);
    }
    out
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct World<'a> {
    core: CommitmentCoreContractClient<'a>,
    token: TokenClient<'a>,
    asset: Address,
}

fn setup_world(e: &Env) -> World<'_> {
    e.mock_all_auths_allowing_non_root_auth();
    // The model performs many verification reads per step on one Env; lift the
    // default per-Env budget so long bounded sequences are not cut short.
    e.budget().reset_unlimited();
    e.ledger().with_mut(|l| {
        l.timestamp = START_TS;
    });

    let contract_id = e.register_contract(None, CommitmentCoreContract);
    let nft = e.register_contract(None, LifecycleModelNft);
    let admin = addr(e, ADMIN_STR);

    let token_contract = e.register_stellar_asset_contract_v2(addr(e, TOKEN_ADMIN_STR));
    let asset = token_contract.address();
    let sac = StellarAssetClient::new(e, &asset);
    for o in OWNER_STRS {
        sac.mint(&addr(e, o), &OWNER_MINT);
    }

    let core = CommitmentCoreContractClient::new(e, &contract_id);
    core.initialize(&admin, &nft);
    core.add_updater(&admin, &addr(e, UPDATER_STR));
    core.add_allocator(&admin, &addr(e, ALLOCATOR_STR));
    core.set_fee_recipient(&admin, &addr(e, RECIPIENT_STR));

    World {
        core,
        token: TokenClient::new(e, &asset),
        asset,
    }
}

/// Concrete arguments derived from the model for parameterized commands.
#[derive(Default, Clone, Copy)]
struct Args {
    nv: i128,
    alloc_amt: i128,
    wd_amt: i128,
}

fn compute_args(m: &Model, cmd: &Cmd) -> Args {
    let mut a = Args::default();
    match *cmd {
        Cmd::UpdateValue {
            slot, value_cls, ..
        } => {
            if let Some(sl) = m.slots[slot] {
                a.nv = match value_cls {
                    0 => sl.value,
                    1 => sl.value / 2,
                    2 => sl.value + 1,
                    3 => 0,
                    4 => 1,
                    _ => sl.amount / 2, // deep loss relative to principal
                };
            }
        }
        Cmd::Allocate {
            slot, amount_cls, ..
        } => {
            if let Some(sl) = m.slots[slot] {
                a.alloc_amt = match amount_cls {
                    0 => sl.value / 2,
                    1 => sl.value,
                    2 => sl.value + 1,
                    _ => 0,
                };
            }
        }
        Cmd::WithdrawFees { amount_cls, .. } => {
            a.wd_amt = match amount_cls {
                0 => m.fees,
                1 => m.fees / 2,
                _ => m.fees + 1,
            };
        }
        _ => {}
    }
    a
}

/// Model prediction of success/failure. Pure - no mutation.
fn predicts_ok(m: &Model, cmd: &Cmd, a: &Args) -> bool {
    match *cmd {
        Cmd::Create {
            owner, amount_cls, ..
        } => {
            let amt = CREATE_AMOUNTS[amount_cls];
            amt > 0 && m.free_slot().is_some() && m.bal_owner[owner] >= amt
        }
        Cmd::UpdateValue { slot, actor, .. } => {
            matches!(m.slots[slot], Some(sl) if sl.st == St::Active)
                && actor == Actor::Authorized
                && a.nv >= 0
        }
        Cmd::Settle { slot } => match m.slots[slot] {
            Some(sl) => {
                sl.st == St::Active
                    && m.now >= sl.expires_at
                    // payout transfer must be covered by contract custody
                    && m.bal_contract >= sl.value
            }
            None => false,
        },
        Cmd::EarlyExit { slot, wrong_caller } => match m.slots[slot] {
            Some(sl) => {
                sl.st == St::Active
                    && !wrong_caller
                    && m.bal_contract >= sl.value - sl.value * sl.penalty as i128 / 100
            }
            None => false,
        },
        Cmd::Allocate { slot, actor, .. } => {
            matches!(m.slots[slot], Some(sl) if sl.st == St::Active)
                && actor == Actor::Authorized
                && a.alloc_amt > 0
                && matches!(m.slots[slot], Some(sl) if sl.value >= a.alloc_amt)
                && m.bal_contract >= a.alloc_amt
        }
        Cmd::SetFeeBps { outsider, .. } => !outsider,
        Cmd::WithdrawFees { outsider, .. } => !outsider && a.wd_amt > 0 && a.wd_amt <= m.fees,
        Cmd::AdvanceTime { .. } => true,
    }
}

/// Applies model mutations for a command that both model and contract accept.
fn mutate(m: &mut Model, cmd: &Cmd, a: &Args) {
    match *cmd {
        Cmd::Create {
            owner,
            amount_cls,
            rules,
        } => {
            let amt = CREATE_AMOUNTS[amount_cls];
            let (dur, ml, p, _) = RULE_SETS[rules];
            let fee = fee_from_bps(amt, m.fee_bps);
            let net = amt - fee;
            let slot_idx = m.free_slot().expect("model slot reserved");
            m.slots[slot_idx] = Some(Slot {
                counter: m.counter,
                owner,
                amount: net,
                value: net,
                released: 0,
                expires_at: m.now + dur as u64 * DAY,
                max_loss: ml,
                penalty: p,
                st: St::Active,
            });
            m.counter += 1;
            m.owner_lists[owner] += 1;
            m.tvl += net;
            m.fees += fee;
            m.bal_owner[owner] -= amt;
            m.bal_contract += amt;
        }
        Cmd::UpdateValue { slot, .. } => {
            let sl = m.slots[slot].as_mut().expect("modeled slot");
            let delta = a.nv - sl.value;
            m.tvl += delta;
            sl.value = a.nv;
            if Model::loss_percent(sl.amount, a.nv) > sl.max_loss as i128 {
                sl.st = St::Violated;
            }
        }
        Cmd::Settle { slot } => {
            let sl = m.slots[slot].as_mut().expect("modeled slot");
            let payout = sl.value;
            sl.st = St::Settled;
            sl.released += payout;
            m.tvl -= payout;
            m.bal_contract -= payout;
            m.bal_owner[sl.owner] += payout;
            m.owner_lists[sl.owner] -= 1;
        }
        Cmd::EarlyExit { slot, .. } => {
            let sl = m.slots[slot].as_mut().expect("modeled slot");
            let penalty = sl.value * sl.penalty as i128 / 100;
            let returned = sl.value - penalty;
            sl.st = St::EarlyExit;
            sl.released += returned + penalty;
            m.tvl -= sl.value;
            sl.value = 0;
            m.fees += penalty;
            m.bal_contract -= returned;
            m.bal_owner[sl.owner] += returned;
        }
        Cmd::Allocate { slot, .. } => {
            let sl = m.slots[slot].as_mut().expect("modeled slot");
            sl.value -= a.alloc_amt;
            sl.released += a.alloc_amt;
            m.tvl -= a.alloc_amt;
            m.bal_contract -= a.alloc_amt;
            m.bal_pool += a.alloc_amt;
        }
        Cmd::SetFeeBps { bps, .. } => {
            m.fee_bps = bps;
        }
        Cmd::WithdrawFees { .. } => {
            m.fees -= a.wd_amt;
            m.bal_contract -= a.wd_amt;
            m.bal_recipient += a.wd_amt;
        }
        Cmd::AdvanceTime { days } => {
            m.now += days as u64 * DAY;
        }
    }
}

/// Executes the command against the real contract. Returns success/failure.
fn execute(w: &World, m: &Model, cmd: &Cmd, a: &Args) -> bool {
    let e = &w.core.env;
    let c = &w.core;
    let admin = addr(e, ADMIN_STR);
    let updater = addr(e, UPDATER_STR);
    let allocator = addr(e, ALLOCATOR_STR);
    let outsider = addr(e, OUTSIDER_STR);
    match *cmd {
        Cmd::Create {
            owner,
            amount_cls,
            rules,
        } => {
            let (d, ml, p, t) = RULE_SETS[rules];
            let rules = CommitmentRules {
                duration_days: d,
                max_loss_percent: ml,
                commitment_type: String::from_str(e, t),
                early_exit_penalty: p,
                min_fee_threshold: 0,
                grace_period_days: 0,
            };
            c.try_create_commitment(
                &addr(e, OWNER_STRS[owner]),
                &CREATE_AMOUNTS[amount_cls],
                &w.asset,
                &rules,
            )
            .is_ok()
        }
        Cmd::UpdateValue { slot, actor, .. } => {
            let Some(sl) = m.slots[slot] else {
                return false;
            };
            let caller = match actor {
                Actor::Authorized => updater,
                Actor::Outsider => outsider,
            };
            let _ = admin;
            c.try_update_value(&caller, &commit_id(e, sl.counter), &a.nv)
                .is_ok()
        }
        Cmd::Settle { slot } => {
            let Some(sl) = m.slots[slot] else {
                return false;
            };
            c.try_settle(&commit_id(e, sl.counter)).is_ok()
        }
        Cmd::EarlyExit { slot, wrong_caller } => {
            let Some(sl) = m.slots[slot] else {
                return false;
            };
            let caller = if wrong_caller {
                outsider
            } else {
                addr(e, OWNER_STRS[sl.owner])
            };
            c.try_early_exit(&commit_id(e, sl.counter), &caller).is_ok()
        }
        Cmd::Allocate { slot, actor, .. } => {
            let Some(sl) = m.slots[slot] else {
                return false;
            };
            let caller = match actor {
                Actor::Authorized => allocator,
                Actor::Outsider => outsider,
            };
            c.try_allocate(
                &caller,
                &commit_id(e, sl.counter),
                &addr(e, POOL_STR),
                &a.alloc_amt,
            )
            .is_ok()
        }
        Cmd::SetFeeBps {
            bps,
            outsider: use_outsider,
        } => {
            let caller = if use_outsider { outsider } else { admin };
            c.try_set_creation_fee_bps(&caller, &bps).is_ok()
        }
        Cmd::WithdrawFees {
            outsider: use_outsider,
            ..
        } => {
            let caller = if use_outsider { outsider } else { admin };
            c.try_withdraw_fees(&caller, &w.asset, &a.wd_amt).is_ok()
        }
        Cmd::AdvanceTime { days } => {
            w.core.env.ledger().with_mut(|l| {
                l.timestamp += days as u64 * DAY;
            });
            true
        }
    }
}

type StepResult = Result<(), std::string::String>;

/// Runs one command and verifies the entire observable state afterwards, whether
/// the command succeeded or failed. This is what makes invalid-command atomicity
/// (I5) explicit: a failed command must leave every modeled field untouched.
fn apply(w: &World, m: &mut Model, cmd: &Cmd) -> StepResult {
    let a = compute_args(m, cmd);
    let expected = predicts_ok(m, cmd, &a);
    let actual = execute(w, m, cmd, &a);
    if expected != actual {
        return Err(std::format!(
            "outcome mismatch: model expected {}, contract returned {}",
            if expected { "success" } else { "failure" },
            if actual { "success" } else { "failure" }
        ));
    }
    if expected && actual {
        mutate(m, cmd, &a);
    }
    verify(w, m)
}

fn expect(cond: bool, what: &str, detail: std::string::String) -> StepResult {
    if cond {
        Ok(())
    } else {
        Err(std::format!("invariant {} violated: {}", what, detail))
    }
}

/// Full observable-state comparison plus the invariant battery.
fn verify(w: &World, m: &Model) -> StepResult {
    let e = &w.core.env;

    // Per-commitment record equality.
    for slot in m.slots.iter().flatten() {
        let id = commit_id(e, slot.counter);
        let rec = w.core.get_commitment(&id);
        expect(
            rec.status == String::from_str(e, slot.st.as_str()),
            "status-mirror",
            std::format!("COMMIT_{} expected {}", slot.counter, slot.st.as_str()),
        )?;
        expect(
            rec.current_value == slot.value,
            "value-mirror",
            std::format!("COMMIT_{}", slot.counter),
        )?;
        expect(
            rec.amount == slot.amount,
            "principal-mirror",
            std::format!("COMMIT_{}", slot.counter),
        )?;
        expect(
            rec.expires_at == slot.expires_at,
            "expiry-mirror",
            std::format!("COMMIT_{}", slot.counter),
        )?;
        expect(
            rec.owner == addr(e, OWNER_STRS[slot.owner]),
            "owner-mirror",
            std::format!("COMMIT_{}", slot.counter),
        )?;
    }

    // Aggregates mirror.
    expect(
        w.core.get_total_value_locked() == m.tvl,
        "tvl-mirror",
        std::format!(
            "model={} contract={}",
            m.tvl,
            w.core.get_total_value_locked()
        ),
    )?;
    expect(
        w.core.get_total_commitments() == m.counter,
        "counter-mirror",
        std::format!("model={}", m.counter),
    )?;
    expect(
        w.core.get_collected_fees(&w.asset) == m.fees,
        "fee-mirror",
        std::format!(
            "model={} contract={}",
            m.fees,
            w.core.get_collected_fees(&w.asset)
        ),
    )?;

    // I1 custody (flow form): the contract token balance equals every commitment's
    // not-yet-released principal plus held fees. `released` accumulates settle
    // payouts, exit returns+penalties, and allocations, so markdowns via
    // `update_value` correctly do NOT change custody.
    let unspent: i128 = m
        .slots
        .iter()
        .flatten()
        .map(|s| s.amount - s.released)
        .sum();
    let custody = w.token.balance(&w.core.address);
    expect(
        custody == unspent + m.fees,
        "custody",
        std::format!("balance={} unspent={} fees={}", custody, unspent, m.fees),
    )?;

    // Observed balances mirror the model.
    for (i, o) in OWNER_STRS.iter().enumerate() {
        let ob = w.token.balance(&addr(e, o));
        expect(
            ob == m.bal_owner[i],
            "balance-mirror",
            std::format!("owner{}", i),
        )?;
    }
    expect(
        w.token.balance(&addr(e, RECIPIENT_STR)) == m.bal_recipient,
        "balance-mirror",
        std::string::String::from("recipient"),
    )?;
    expect(
        w.token.balance(&addr(e, POOL_STR)) == m.bal_pool,
        "balance-mirror",
        std::string::String::from("pool"),
    )?;

    // I1 global conservation: tracked balances still hold the entire minted supply.
    let total = m.bal_owner.iter().sum::<i128>() + m.bal_contract + m.bal_recipient + m.bal_pool;
    expect(
        total == OWNER_MINT * 3,
        "supply-conservation",
        std::format!("sum={}", total),
    )?;

    // Owner-index lists: only settle removes entries.
    for (i, o) in OWNER_STRS.iter().enumerate() {
        let len = w.core.list_commitments_by_owner(&addr(e, o)).len();
        expect(
            len as usize == m.expected_owner_list(i),
            "owner-index-mirror",
            std::format!("owner{} len={}", i, len),
        )?;
    }
    Ok(())
}

fn digest_of(w: &World, m: &Model) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    fn feed(h: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *h ^= *b as u64;
            *h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    feed(&mut h, &m.tvl.to_le_bytes());
    feed(&mut h, &m.fees.to_le_bytes());
    feed(&mut h, &m.fee_bps.to_le_bytes());
    feed(&mut h, &m.now.to_le_bytes());
    feed(&mut h, &m.counter.to_le_bytes());
    for s in m.slots.iter().flatten() {
        feed(&mut h, &s.counter.to_le_bytes());
        feed(&mut h, &(s.owner as u64).to_le_bytes());
        feed(&mut h, &s.amount.to_le_bytes());
        feed(&mut h, &s.value.to_le_bytes());
        feed(&mut h, &s.expires_at.to_le_bytes());
        feed(&mut h, &(s.st as u64).to_le_bytes());
    }
    for i in 0..3 {
        feed(&mut h, &m.bal_owner[i].to_le_bytes());
    }
    feed(&mut h, &m.bal_contract.to_le_bytes());
    feed(&mut h, &m.bal_recipient.to_le_bytes());
    feed(&mut h, &m.bal_pool.to_le_bytes());
    feed(&mut h, &w.core.get_total_value_locked().to_le_bytes());
    feed(&mut h, &w.core.get_collected_fees(&w.asset).to_le_bytes());
    h
}

type ScenarioResult = Result<u64, (usize, std::string::String)>;

/// Runs a full scenario in a fresh environment; returns the final digest or the
/// first failing step with its reason.
fn run_scenario(_seed: u64, cmds: &[Cmd]) -> ScenarioResult {
    let e = Env::default();
    let w = setup_world(&e);
    let mut m = Model::new();
    for (step, cmd) in cmds.iter().enumerate() {
        if let Err(reason) = apply(&w, &mut m, cmd) {
            return Err((step, std::format!("{} [cmd: {}]", reason, cmd_debug(cmd))));
        }
    }
    Ok(digest_of(&w, &m))
}

/// Greedy delta-debugging minimization: repeatedly drop single commands while the
/// scenario keeps failing. Deterministic; O(n^2) worst case, failure-path only.
fn minimize(seed: u64, cmds: &[Cmd]) -> std::vec::Vec<Cmd> {
    let mut cur: std::vec::Vec<Cmd> = cmds.to_vec();
    loop {
        let mut changed = false;
        for i in 0..cur.len() {
            let mut candidate = cur.clone();
            candidate.remove(i);
            if run_scenario(seed, &candidate).is_err() {
                cur = candidate;
                changed = true;
                break;
            }
        }
        if !changed || cur.is_empty() {
            break;
        }
    }
    cur
}

fn failure_report(seed: u64, cmds: &[Cmd], failure: &(usize, std::string::String)) -> ! {
    let minimized = minimize(seed, cmds);
    let min_failure = run_scenario(seed, &minimized)
        .err()
        .unwrap_or_else(|| (0, std::string::String::from("no longer fails")));
    panic!(
        "\nLIFECYCLE MODEL VIOLATION\n\
         seed: {}\n\
         reproduce: CGQA_LIFECYCLE_SEED={} cargo test -p commitment_core --lib lifecycle_model\n\
         failing step: {} of {}\n\
         reason: {}\n\
         original sequence:\n  {}\n\
         minimized sequence ({} cmds):\n  {}\n\
         minimized failure at step {}: {}\n",
        seed,
        seed,
        failure.0,
        cmds.len(),
        failure.1,
        seq_debug(cmds),
        minimized.len(),
        seq_debug(&minimized),
        min_failure.0,
        min_failure.1
    );
}

// ---------------------------------------------------------------------------
// Seeded property suite
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_model_seeded_sequences_hold_invariants() {
    let replay = std::env::var("CGQA_LIFECYCLE_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let count = if replay.is_some() { 1 } else { SEED_COUNT };
    for i in 0..count {
        let seed = replay.unwrap_or(BASE_SEED + i);
        let mut rng = Rng::new(seed);
        let cmds = generate_commands(&mut rng, SEQ_LEN);
        if let Err(failure) = run_scenario(seed, &cmds) {
            failure_report(seed, &cmds, &failure);
        }
    }
}

/// Single-seed replay hook used by the reproduction instructions above.
#[test]
fn lifecycle_model_seed_replay_entrypoint() {
    let seed = std::env::var("CGQA_LIFECYCLE_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(BASE_SEED);
    let mut rng = Rng::new(seed);
    let cmds = generate_commands(&mut rng, SEQ_LEN);
    if let Err(failure) = run_scenario(seed, &cmds) {
        failure_report(seed, &cmds, &failure);
    }
}

/// I6: identical seed and identical environment produce identical results, and
/// distinct seeds produce distinct traces (guards against degenerate generation).
#[test]
fn lifecycle_model_is_deterministic() {
    let trace_digest = |seed: u64| -> u64 {
        let mut rng = Rng::new(seed);
        let cmds = generate_commands(&mut rng, SEQ_LEN);
        run_scenario(seed, &cmds)
            .unwrap_or_else(|f| panic!("seed {} unexpectedly fails: {}", seed, f.1))
    };
    let a1 = trace_digest(BASE_SEED);
    let a2 = trace_digest(BASE_SEED);
    let b = trace_digest(BASE_SEED + 7);
    assert_eq!(a1, a2, "same seed must yield identical final state");
    assert_ne!(
        a1, b,
        "distinct seeds must diverge (non-degenerate generator)"
    );
}

// ---------------------------------------------------------------------------
// Hand-written regression scripts (portable counterexample coverage)
// ---------------------------------------------------------------------------

fn script(cmds: std::vec::Vec<Cmd>) -> u64 {
    let seed = BASE_SEED; // scripts are seed-independent; kept for reporting parity
    run_scenario(seed, &cmds)
        .unwrap_or_else(|f| panic!("regression script failed at step {}: {}", f.0, f.1))
}

/// Duplicate settle must be rejected and must not pay out twice (I4/I5/I1).
#[test]
fn regression_duplicate_settle_pays_exactly_once() {
    script(std::vec![
        Cmd::Create {
            owner: 0,
            amount_cls: 2,
            rules: 0
        },
        Cmd::AdvanceTime { days: 1 },
        Cmd::Settle { slot: 0 },
        Cmd::Settle { slot: 0 }, // duplicate terminal op
    ]);
}

/// Terminal states are absorbing: settled, exited, and violated records reject all
/// active-only flows; repeated exit is rejected; value/principal frozen (I4).
#[test]
fn regression_terminal_states_absorbing() {
    script(std::vec![
        Cmd::Create {
            owner: 1,
            amount_cls: 3,
            rules: 1
        },
        Cmd::EarlyExit {
            slot: 0,
            wrong_caller: false
        },
        Cmd::EarlyExit {
            slot: 0,
            wrong_caller: false
        }, // repeat terminal op
        Cmd::Settle { slot: 0 }, // exit then settle attempt
        Cmd::UpdateValue {
            slot: 0,
            value_cls: 0,
            actor: Actor::Authorized
        },
        Cmd::Allocate {
            slot: 0,
            amount_cls: 0,
            actor: Actor::Authorized
        },
        Cmd::Create {
            owner: 1,
            amount_cls: 2,
            rules: 0
        }, // slot 1
        Cmd::UpdateValue {
            slot: 1,
            value_cls: 5,
            actor: Actor::Authorized
        }, // deep loss -> violated
        Cmd::UpdateValue {
            slot: 1,
            value_cls: 2,
            actor: Actor::Authorized
        }, // violated rejects
        Cmd::AdvanceTime { days: 7 },
        Cmd::Settle { slot: 1 }, // violated cannot settle even after expiry
        Cmd::EarlyExit {
            slot: 1,
            wrong_caller: false
        },
        Cmd::Allocate {
            slot: 1,
            amount_cls: 0,
            actor: Actor::Authorized
        },
        Cmd::Create {
            owner: 2,
            amount_cls: 0,
            rules: 2
        }, // slot 2
        Cmd::AdvanceTime { days: 30 },
        Cmd::Settle { slot: 2 },
        Cmd::Settle { slot: 2 }, // double settle
    ]);
}

/// Wrong-actor matrix: every privileged operation rejects outsiders and leaves
/// state byte-identical (I3/I5).
#[test]
fn regression_wrong_actor_matrix_is_atomic() {
    script(std::vec![
        Cmd::Create {
            owner: 0,
            amount_cls: 1,
            rules: 0
        },
        Cmd::Create {
            owner: 2,
            amount_cls: 1,
            rules: 1
        },
        Cmd::UpdateValue {
            slot: 0,
            value_cls: 0,
            actor: Actor::Outsider
        },
        Cmd::EarlyExit {
            slot: 1,
            wrong_caller: true
        }, // non-owner exit
        Cmd::Allocate {
            slot: 0,
            amount_cls: 0,
            actor: Actor::Outsider
        },
        Cmd::SetFeeBps {
            bps: 100,
            outsider: true
        },
        Cmd::WithdrawFees {
            amount_cls: 0,
            outsider: true
        },
    ]);
}

/// Fee accounting across creation fee, exit penalty, partial and full withdrawal
/// (I2/I1).
#[test]
fn regression_fee_accounting_conservation() {
    script(std::vec![
        Cmd::SetFeeBps {
            bps: 250,
            outsider: false
        },
        Cmd::Create {
            owner: 0,
            amount_cls: 3,
            rules: 0
        },
        Cmd::Create {
            owner: 1,
            amount_cls: 2,
            rules: 1
        },
        Cmd::EarlyExit {
            slot: 0,
            wrong_caller: false
        }, // penalty joins fees
        Cmd::WithdrawFees {
            amount_cls: 1,
            outsider: false
        }, // half
        Cmd::AdvanceTime { days: 7 },
        Cmd::Settle { slot: 1 },
        Cmd::WithdrawFees {
            amount_cls: 0,
            outsider: false
        }, // remainder
        Cmd::WithdrawFees {
            amount_cls: 2,
            outsider: false
        }, // excess rejected
    ]);
}

/// Boundary: tiny principal makes the exit penalty truncate to zero - the owner
/// receives the whole value and no fee may accrue (I2 boundary).
#[test]
fn regression_zero_penalty_boundary_exit() {
    script(std::vec![
        Cmd::Create {
            owner: 0,
            amount_cls: 0,
            rules: 2
        }, // 1000 @ aggressive 5%
        Cmd::EarlyExit {
            slot: 0,
            wrong_caller: false
        }, // penalty = 1000*5/100 = 50
        // Follow-up: value below 100 makes penalty truncate to zero.
        Cmd::Create {
            owner: 0,
            amount_cls: 0,
            rules: 0
        },
        Cmd::UpdateValue {
            slot: 1,
            value_cls: 4,
            actor: Actor::Authorized
        }, // value=1
        Cmd::EarlyExit {
            slot: 1,
            wrong_caller: false
        }, // penalty = 0
    ]);
}

/// Insufficient funding must fail atomically: no counters, no TVL, no custody (I5).
#[test]
fn regression_insufficient_create_is_atomic() {
    script(std::vec![
        Cmd::Create {
            owner: 0,
            amount_cls: 4,
            rules: 0
        }, // 2e21 >> balance
        Cmd::Create {
            owner: 0,
            amount_cls: 0,
            rules: 0
        }, // healthy follow-up works
    ]);
}

/// Settle ordering: before expiry settle is rejected even for settled records;
/// after expiry it is rejected as AlreadySettled - neither moves value (I4).
#[test]
fn regression_settle_ordering_guards() {
    script(std::vec![
        Cmd::Create {
            owner: 1,
            amount_cls: 2,
            rules: 2
        },
        Cmd::Settle { slot: 0 }, // before expiry -> rejected
        Cmd::AdvanceTime { days: 30 },
        Cmd::Settle { slot: 0 },
        Cmd::Settle { slot: 0 }, // after expiry, already settled -> rejected
    ]);
}

/// Allocation drains principal to a pool while custody/TVL/supply stay consistent.
#[test]
fn regression_allocation_conservation() {
    script(std::vec![
        Cmd::Create {
            owner: 0,
            amount_cls: 3,
            rules: 1
        },
        Cmd::Allocate {
            slot: 0,
            amount_cls: 0,
            actor: Actor::Authorized
        }, // half
        Cmd::Allocate {
            slot: 0,
            amount_cls: 2,
            actor: Actor::Authorized
        }, // overdraft rejected
        Cmd::Allocate {
            slot: 0,
            amount_cls: 3,
            actor: Actor::Authorized
        }, // zero rejected
        Cmd::Allocate {
            slot: 0,
            amount_cls: 1,
            actor: Actor::Authorized
        }, // remainder
        Cmd::AdvanceTime { days: 7 },
        Cmd::Settle { slot: 0 }, // settles the drained remainder
    ]);
}
