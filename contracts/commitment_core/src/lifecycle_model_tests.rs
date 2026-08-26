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
//! After every executed command - valid or invalid - the harness compares the modeled
//! observable core/token state against the reference model and checks the invariants.
//! Event history, rate-limit internals, downstream NFT storage, multi-asset
//! interactions, and production resource ceilings remain outside this harness. The
//! fixture uses a single asset, `reset_unlimited`, and mocked authorizations, so I3
//! proves owner/role predicates but does not prove that every path calls
//! `require_auth`:
//!
//! - I1 principal conservation: contract token custody equals locked principal plus
//!   held fees, and tracked balances always sum to the minted supply.
//! - I2 fee conservation: collected fees == creation fees + exit penalties -
//!   withdrawals.
//! - I3 ownership/auth: only permitted actors succeed for each operation.
//! - I4 terminal immutability: `violated`, `settled`, `early_exit` reject all
//!   active-only flows and never move principal or fees twice.
//! - I5 invalid-command atomicity: every modeled observable after an invalid command
//!   equals its pre-command value.
//! - I6 determinism: fixed seeds, fixed addresses, no entropy sources.
//!
//! Any mismatch prints the seed, the original sequence, and a greedily minimized
//! sequence. A failing seed can be replayed alone with:
//! `CGQA_LIFECYCLE_SEED=<seed> cargo test -p commitment_core --lib lifecycle_model_seed_replay_entrypoint`

extern crate std;

use crate::{CommitmentCoreContract, CommitmentCoreContractClient, CommitmentRules, DataKey};
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
        // xorshift64* has one forbidden state (zero). Do not force the low bit:
        // `seed | 1` aliases every even seed with the following odd seed.
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
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
    created_at: u64,
    expires_at: u64,
    rules: usize,
    max_loss: u32,
    penalty: u32,
    st: St,
}

fn own_remaining_principal(slot: &Slot) -> i128 {
    assert!(
        slot.released >= 0,
        "released principal cannot be negative in the safe generator"
    );
    assert!(
        slot.released <= slot.amount,
        "released principal cannot exceed the slot's own principal in the safe generator"
    );
    slot.amount - slot.released
}

struct Model {
    slots: [Option<Slot>; MAX_SLOTS],
    counter: u64,
    tvl: i128,
    fees: i128,
    creation_fees_accrued: i128,
    penalty_fees_accrued: i128,
    fees_withdrawn: i128,
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
            creation_fees_accrued: 0,
            penalty_fees_accrued: 0,
            fees_withdrawn: 0,
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
    /// Ledger timestamp advance (drives expiry, including exact +/- 1s boundaries).
    AdvanceTime { seconds: u64 },
}

/// Classes 4-6 are invalid: insufficient funding, zero, and negative.
const CREATE_AMOUNTS: [i128; 7] = [
    1_000,
    999_983,
    10_000_000_000,
    1_000_000_000_000_000,
    2_000_000_000_000_000_000_000, // 2e21 > 1e18 minted balance -> invalid
    0,
    -1,
];

/// (duration_days, max_loss_percent, early_exit_penalty, type) - all valid combos.
const RULE_SETS: [(u32, u32, u32, &str); 3] = [
    (1, 10, 15, "safe"),
    (7, 30, 10, "balanced"),
    (30, 100, 5, "aggressive"),
];

const VALUE_SAME: usize = 0;
const VALUE_MILD_MARKDOWN: usize = 1;
const VALUE_RECOVER_WITHIN_OWN_PRINCIPAL: usize = 2;
const VALUE_ZERO: usize = 3;
const VALUE_TINY: usize = 4;
const VALUE_DEEP_LOSS: usize = 5;
const VALUE_NEGATIVE: usize = 6;
const VALUE_CLASSES: usize = 7;
// half, full, over(full+1), zero, negative
const ALLOC_CLASSES: usize = 5;
// exact, half, excess(+1), zero, negative
const WITHDRAW_CLASSES: usize = 5;

fn value_class_name(value_cls: usize) -> &'static str {
    match value_cls {
        VALUE_SAME => "Same",
        VALUE_MILD_MARKDOWN => "MildMarkdown",
        VALUE_RECOVER_WITHIN_OWN_PRINCIPAL => "RecoverWithinOwnPrincipal",
        VALUE_ZERO => "Zero",
        VALUE_TINY => "Tiny",
        VALUE_DEEP_LOSS => "DeepLoss",
        VALUE_NEGATIVE => "Negative",
        _ => "Unknown",
    }
}

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
            "UpdateValue(slot={}, value_cls={}({}), actor={:?})",
            slot,
            value_cls,
            value_class_name(value_cls),
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
        Cmd::AdvanceTime { seconds } => {
            std::format!("AdvanceTime(seconds={})", seconds)
        }
    }
}

fn seq_debug(cmds: &[Cmd]) -> std::string::String {
    cmds.iter()
        .map(cmd_debug)
        .collect::<std::vec::Vec<_>>()
        .join(";\n  ")
}

fn encode_commands(cmds: &[Cmd]) -> std::string::String {
    cmds.iter()
        .map(|cmd| match *cmd {
            Cmd::Create {
                owner,
                amount_cls,
                rules,
            } => std::format!("C,{owner},{amount_cls},{rules}"),
            Cmd::UpdateValue {
                slot,
                value_cls,
                actor,
            } => std::format!(
                "U,{slot},{value_cls},{}",
                if actor == Actor::Authorized { 0 } else { 1 }
            ),
            Cmd::Settle { slot } => std::format!("S,{slot}"),
            Cmd::EarlyExit { slot, wrong_caller } => {
                std::format!("E,{slot},{}", u8::from(wrong_caller))
            }
            Cmd::Allocate {
                slot,
                amount_cls,
                actor,
            } => std::format!(
                "A,{slot},{amount_cls},{}",
                if actor == Actor::Authorized { 0 } else { 1 }
            ),
            Cmd::SetFeeBps { bps, outsider } => {
                std::format!("F,{bps},{}", u8::from(outsider))
            }
            Cmd::WithdrawFees {
                amount_cls,
                outsider,
            } => std::format!("W,{amount_cls},{}", u8::from(outsider)),
            Cmd::AdvanceTime { seconds } => std::format!("T,{seconds}"),
        })
        .collect::<std::vec::Vec<_>>()
        .join(";")
}

fn parse_field<T: std::str::FromStr>(raw: &str, name: &str) -> Result<T, std::string::String> {
    raw.parse::<T>()
        .map_err(|_| std::format!("invalid {name}: {raw:?}"))
}

fn parse_flag(raw: &str, name: &str) -> Result<bool, std::string::String> {
    match raw {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(std::format!("invalid {name} flag: {raw:?}")),
    }
}

fn decode_commands(raw: &str) -> Result<std::vec::Vec<Cmd>, std::string::String> {
    if raw.trim().is_empty() {
        return Err(std::string::String::from(
            "command token must contain at least one command",
        ));
    }
    raw.split(';')
        .enumerate()
        .map(|(index, encoded)| {
            let fields = encoded.split(',').collect::<std::vec::Vec<_>>();
            let field = |position: usize| {
                fields.get(position).copied().ok_or_else(|| {
                    std::format!("command {index} missing field {position}: {encoded:?}")
                })
            };
            let exact = |count: usize| {
                if fields.len() == count {
                    Ok(())
                } else {
                    Err(std::format!(
                        "command {index} expected {count} fields, got {}: {encoded:?}",
                        fields.len()
                    ))
                }
            };
            match field(0)? {
                "C" => {
                    exact(4)?;
                    let owner = parse_field::<usize>(field(1)?, "owner")?;
                    let amount_cls = parse_field::<usize>(field(2)?, "create amount class")?;
                    let rules = parse_field::<usize>(field(3)?, "rules")?;
                    if owner >= OWNER_STRS.len()
                        || amount_cls >= CREATE_AMOUNTS.len()
                        || rules >= RULE_SETS.len()
                    {
                        return Err(std::format!("command {index} create field out of range"));
                    }
                    Ok(Cmd::Create {
                        owner,
                        amount_cls,
                        rules,
                    })
                }
                "U" => {
                    exact(4)?;
                    let slot = parse_field::<usize>(field(1)?, "slot")?;
                    let value_cls = parse_field::<usize>(field(2)?, "value class")?;
                    let outsider = parse_flag(field(3)?, "actor")?;
                    if slot >= MAX_SLOTS || value_cls >= VALUE_CLASSES {
                        return Err(std::format!("command {index} update field out of range"));
                    }
                    Ok(Cmd::UpdateValue {
                        slot,
                        value_cls,
                        actor: if outsider {
                            Actor::Outsider
                        } else {
                            Actor::Authorized
                        },
                    })
                }
                "S" => {
                    exact(2)?;
                    let slot = parse_field::<usize>(field(1)?, "slot")?;
                    if slot >= MAX_SLOTS {
                        return Err(std::format!("command {index} settle slot out of range"));
                    }
                    Ok(Cmd::Settle { slot })
                }
                "E" => {
                    exact(3)?;
                    let slot = parse_field::<usize>(field(1)?, "slot")?;
                    if slot >= MAX_SLOTS {
                        return Err(std::format!("command {index} exit slot out of range"));
                    }
                    Ok(Cmd::EarlyExit {
                        slot,
                        wrong_caller: parse_flag(field(2)?, "wrong caller")?,
                    })
                }
                "A" => {
                    exact(4)?;
                    let slot = parse_field::<usize>(field(1)?, "slot")?;
                    let amount_cls = parse_field::<usize>(field(2)?, "allocation amount class")?;
                    let outsider = parse_flag(field(3)?, "actor")?;
                    if slot >= MAX_SLOTS || amount_cls >= ALLOC_CLASSES {
                        return Err(std::format!(
                            "command {index} allocation field out of range"
                        ));
                    }
                    Ok(Cmd::Allocate {
                        slot,
                        amount_cls,
                        actor: if outsider {
                            Actor::Outsider
                        } else {
                            Actor::Authorized
                        },
                    })
                }
                "F" => {
                    exact(3)?;
                    Ok(Cmd::SetFeeBps {
                        bps: parse_field::<u32>(field(1)?, "fee bps")?,
                        outsider: parse_flag(field(2)?, "outsider")?,
                    })
                }
                "W" => {
                    exact(3)?;
                    let amount_cls = parse_field::<usize>(field(1)?, "withdraw amount class")?;
                    if amount_cls >= WITHDRAW_CLASSES {
                        return Err(std::format!(
                            "command {index} withdrawal field out of range"
                        ));
                    }
                    Ok(Cmd::WithdrawFees {
                        amount_cls,
                        outsider: parse_flag(field(2)?, "outsider")?,
                    })
                }
                "T" => {
                    exact(2)?;
                    Ok(Cmd::AdvanceTime {
                        seconds: parse_field::<u64>(field(1)?, "seconds")?,
                    })
                }
                tag => Err(std::format!("command {index} has unknown tag {tag:?}")),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

fn generated_slot(rng: &mut Rng, created: usize) -> usize {
    // Exercise real entrypoints most of the time while retaining nonexistent-slot
    // rejection coverage. The original uniform 0..24 selection short-circuited
    // almost every lifecycle command before it reached the contract.
    if created > 0 && rng.chance(85) {
        rng.below(created)
    } else {
        rng.below(MAX_SLOTS)
    }
}

fn deterministic_coverage_prefix() -> [Cmd; 3] {
    [
        Cmd::Create {
            owner: 0,
            amount_cls: 0,
            rules: 0,
        },
        Cmd::UpdateValue {
            slot: 0,
            value_cls: VALUE_MILD_MARKDOWN,
            actor: Actor::Authorized,
        },
        Cmd::UpdateValue {
            slot: 0,
            value_cls: VALUE_RECOVER_WITHIN_OWN_PRINCIPAL,
            actor: Actor::Authorized,
        },
    ]
}

fn generate_commands(rng: &mut Rng, len: usize) -> std::vec::Vec<Cmd> {
    let mut out = std::vec::Vec::new();
    for cmd in deterministic_coverage_prefix().into_iter().take(len) {
        out.push(cmd);
    }
    let mut created = if out.is_empty() { 0 } else { 1 };
    for _ in out.len()..len {
        let pick = rng.below(100);
        let cmd = if pick < 28 {
            let invalid_roll = rng.below(100);
            let amount_cls = match invalid_roll {
                0..=7 => 4,   // insufficient funding
                8..=12 => 5,  // zero
                13..=17 => 6, // negative
                _ => rng.below(4),
            };
            let cmd = Cmd::Create {
                owner: rng.below(3),
                amount_cls,
                rules: rng.below(3),
            };
            if amount_cls < 4 && created < MAX_SLOTS {
                created += 1;
            }
            cmd
        } else if pick < 44 {
            Cmd::UpdateValue {
                slot: generated_slot(rng, created),
                value_cls: rng.below(VALUE_CLASSES),
                actor: if rng.chance(30) {
                    Actor::Outsider
                } else {
                    Actor::Authorized
                },
            }
        } else if pick < 58 {
            Cmd::Settle {
                slot: generated_slot(rng, created),
            }
        } else if pick < 72 {
            Cmd::EarlyExit {
                slot: generated_slot(rng, created),
                wrong_caller: rng.chance(30),
            }
        } else if pick < 80 {
            Cmd::Allocate {
                slot: generated_slot(rng, created),
                amount_cls: rng.below(ALLOC_CLASSES),
                actor: if rng.chance(30) {
                    Actor::Outsider
                } else {
                    Actor::Authorized
                },
            }
        } else if pick < 85 {
            Cmd::SetFeeBps {
                bps: [0, 100, 250, 9_999, 10_000, 10_001][rng.below(6)],
                outsider: rng.chance(25),
            }
        } else if pick < 90 {
            Cmd::WithdrawFees {
                amount_cls: rng.below(WITHDRAW_CLASSES),
                outsider: rng.chance(25),
            }
        } else {
            Cmd::AdvanceTime {
                seconds: [0, 1, DAY - 1, DAY, DAY + 1, 3 * DAY, 10 * DAY, 40 * DAY][rng.below(8)],
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
                let own_remaining = own_remaining_principal(&sl);
                let bounded_current = sl.value.min(own_remaining).max(0);
                let requested = match value_cls {
                    VALUE_SAME => bounded_current,
                    VALUE_MILD_MARKDOWN => {
                        if bounded_current == 0 {
                            0
                        } else {
                            let candidate = bounded_current - 1;
                            if Model::loss_percent(sl.amount, candidate) <= sl.max_loss as i128 {
                                candidate
                            } else {
                                bounded_current
                            }
                        }
                    }
                    VALUE_RECOVER_WITHIN_OWN_PRINCIPAL => {
                        if bounded_current < own_remaining {
                            bounded_current + 1
                        } else {
                            bounded_current
                        }
                    }
                    VALUE_ZERO => 0,
                    VALUE_TINY => 1.min(own_remaining),
                    VALUE_DEEP_LOSS => (sl.amount / 2).min(own_remaining),
                    VALUE_NEGATIVE => -1,
                    _ => unreachable!("value class validated by generator/decoder"),
                };
                a.nv = if requested < 0 {
                    requested
                } else {
                    requested.min(own_remaining)
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
                    3 => 0,
                    _ => -1,
                };
            }
        }
        Cmd::WithdrawFees { amount_cls, .. } => {
            a.wd_amt = match amount_cls {
                0 => m.fees,
                1 => m.fees / 2,
                2 => m.fees + 1,
                3 => 0,
                _ => -1,
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
        Cmd::UpdateValue { slot, actor, .. } => match m.slots[slot] {
            Some(sl) => {
                sl.st == St::Active
                    && actor == Actor::Authorized
                    && a.nv >= 0
                    && a.nv <= own_remaining_principal(&sl)
            }
            None => false,
        },
        Cmd::Settle { slot } => match m.slots[slot] {
            Some(sl) => {
                sl.st == St::Active
                    && m.now >= sl.expires_at
                    // Payout stays within this slot's modeled budget and custody.
                    && sl.value >= 0
                    && sl.value <= own_remaining_principal(&sl)
                    && m.bal_contract >= sl.value
            }
            None => false,
        },
        Cmd::EarlyExit { slot, wrong_caller } => match m.slots[slot] {
            Some(sl) => {
                sl.st == St::Active
                    && !wrong_caller
                    && sl.value >= 0
                    && sl.value <= own_remaining_principal(&sl)
                    && m.bal_contract >= sl.value - sl.value * sl.penalty as i128 / 100
            }
            None => false,
        },
        Cmd::Allocate { slot, actor, .. } => match m.slots[slot] {
            Some(sl) => {
                sl.st == St::Active
                    && actor == Actor::Authorized
                    && a.alloc_amt > 0
                    && sl.value >= a.alloc_amt
                    && a.alloc_amt <= own_remaining_principal(&sl)
                    && m.bal_contract >= a.alloc_amt
            }
            None => false,
        },
        Cmd::SetFeeBps { bps, outsider } => !outsider && bps <= 10_000,
        Cmd::WithdrawFees { outsider, .. } => {
            !outsider && a.wd_amt > 0 && a.wd_amt <= m.fees && a.wd_amt <= m.bal_contract
        }
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
                created_at: m.now,
                expires_at: m.now + dur as u64 * DAY,
                rules,
                max_loss: ml,
                penalty: p,
                st: St::Active,
            });
            m.counter += 1;
            m.owner_lists[owner] += 1;
            m.tvl += net;
            m.fees += fee;
            m.creation_fees_accrued += fee;
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
            m.penalty_fees_accrued += penalty;
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
            m.fees_withdrawn += a.wd_amt;
            m.bal_contract -= a.wd_amt;
            m.bal_recipient += a.wd_amt;
        }
        Cmd::AdvanceTime { seconds } => {
            m.now += seconds;
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
        Cmd::AdvanceTime { seconds } => {
            w.core.env.ledger().with_mut(|l| {
                l.timestamp += seconds;
            });
            true
        }
    }
}

type StepResult = Result<(), std::string::String>;
type ApplyResult = Result<bool, std::string::String>;

/// Runs one command and verifies the entire observable state afterwards, whether
/// the command succeeded or failed. This is what makes invalid-command atomicity
/// (I5) explicit: a failed command must leave every modeled field untouched.
fn apply(w: &World, m: &mut Model, cmd: &Cmd) -> ApplyResult {
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
    verify(w, m)?;
    Ok(actual)
}

fn expect(cond: bool, what: &str, detail: std::string::String) -> StepResult {
    if cond {
        Ok(())
    } else {
        Err(std::format!("invariant {} violated: {}", what, detail))
    }
}

/// Modeled observable-state comparison plus the invariant battery.
fn verify(w: &World, m: &Model) -> StepResult {
    let e = &w.core.env;

    expect(
        e.ledger().timestamp() == m.now,
        "ledger-time-mirror",
        std::format!("ledger={} model={}", e.ledger().timestamp(), m.now),
    )?;

    // Per-commitment record equality.
    for slot in m.slots.iter().flatten() {
        let id = commit_id(e, slot.counter);
        let rec = w.core.get_commitment(&id);
        let (duration_days, max_loss_percent, early_exit_penalty, commitment_type) =
            RULE_SETS[slot.rules];
        expect(
            rec.commitment_id == id,
            "id-mirror",
            std::format!("COMMIT_{}", slot.counter),
        )?;
        expect(
            rec.nft_token_id == 1,
            "nft-id-mirror",
            std::format!("COMMIT_{} nft={}", slot.counter, rec.nft_token_id),
        )?;
        expect(
            rec.rules.duration_days == duration_days
                && rec.rules.max_loss_percent == max_loss_percent
                && rec.rules.early_exit_penalty == early_exit_penalty
                && rec.rules.commitment_type == String::from_str(e, commitment_type)
                && rec.rules.min_fee_threshold == 0
                && rec.rules.grace_period_days == 0,
            "rules-mirror",
            std::format!("COMMIT_{}", slot.counter),
        )?;
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
            rec.created_at == slot.created_at,
            "created-at-mirror",
            std::format!("COMMIT_{}", slot.counter),
        )?;
        expect(
            rec.owner == addr(e, OWNER_STRS[slot.owner]),
            "owner-mirror",
            std::format!("COMMIT_{}", slot.counter),
        )?;
        expect(
            rec.asset_address == w.asset,
            "asset-mirror",
            std::format!("COMMIT_{}", slot.counter),
        )?;
        let remaining = own_remaining_principal(slot);
        expect(
            slot.released >= 0 && remaining >= 0,
            "slot-custody-budget",
            std::format!(
                "COMMIT_{} amount={} released={} remaining={}",
                slot.counter,
                slot.amount,
                slot.released,
                remaining
            ),
        )?;
        if slot.st == St::Active {
            expect(
                slot.value >= 0 && slot.value <= remaining,
                "active-value-budget",
                std::format!(
                    "COMMIT_{} value={} remaining={}",
                    slot.counter,
                    slot.value,
                    remaining
                ),
            )?;
        }
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
    expect(
        w.core.get_creation_fee_bps() == m.fee_bps,
        "fee-bps-mirror",
        std::format!(
            "model={} contract={}",
            m.fee_bps,
            w.core.get_creation_fee_bps()
        ),
    )?;
    expect(
        m.fees == m.creation_fees_accrued + m.penalty_fees_accrued - m.fees_withdrawn,
        "independent-fee-ledger",
        std::format!(
            "held={} creation={} penalties={} withdrawn={}",
            m.fees,
            m.creation_fees_accrued,
            m.penalty_fees_accrued,
            m.fees_withdrawn
        ),
    )?;
    expect(
        w.core.get_fee_recipient() == Some(addr(e, RECIPIENT_STR)),
        "fee-recipient-mirror",
        std::string::String::from("configured recipient changed"),
    )?;

    // I1 custody (flow form): the contract token balance equals every commitment's
    // not-yet-released principal plus held fees. `released` accumulates settle
    // payouts, exit returns+penalties, and allocations, so markdowns via
    // `update_value` correctly do NOT change custody.
    let unspent: i128 = m.slots.iter().flatten().map(own_remaining_principal).sum();
    let custody = w.token.balance(&w.core.address);
    expect(
        custody == m.bal_contract,
        "contract-balance-mirror",
        std::format!("model={} contract={}", m.bal_contract, custody),
    )?;
    expect(
        custody == unspent + m.fees,
        "custody",
        std::format!("balance={} unspent={} fees={}", custody, unspent, m.fees),
    )?;

    // Observed balances mirror the model.
    for (i, o) in OWNER_STRS.iter().enumerate() {
        let ob = w.token.balance(&addr(e, o));
        let dimension = std::format!("owner{}-balance-mirror", i);
        expect(
            ob == m.bal_owner[i],
            &dimension,
            std::format!("actual={} model={}", ob, m.bal_owner[i]),
        )?;
    }
    expect(
        w.token.balance(&addr(e, RECIPIENT_STR)) == m.bal_recipient,
        "recipient-balance-mirror",
        std::format!(
            "actual={} model={}",
            w.token.balance(&addr(e, RECIPIENT_STR)),
            m.bal_recipient
        ),
    )?;
    expect(
        w.token.balance(&addr(e, POOL_STR)) == m.bal_pool,
        "pool-balance-mirror",
        std::format!(
            "actual={} model={}",
            w.token.balance(&addr(e, POOL_STR)),
            m.bal_pool
        ),
    )?;

    // I1 global conservation: tracked balances still hold the entire minted supply.
    let total = m.bal_owner.iter().sum::<i128>() + m.bal_contract + m.bal_recipient + m.bal_pool;
    expect(
        total == OWNER_MINT * 3,
        "supply-conservation",
        std::format!("sum={}", total),
    )?;

    // Owner-index lists: compare exact IDs and order, not only lengths.
    for (i, o) in OWNER_STRS.iter().enumerate() {
        let actual = w.core.list_commitments_by_owner(&addr(e, o));
        let len = actual.len();
        let expected_len = m.expected_owner_list(i);
        expect(
            m.owner_lists[i] == expected_len,
            "owner-index-model",
            std::format!(
                "owner{} tracked={} derived={}",
                i,
                m.owner_lists[i],
                expected_len
            ),
        )?;
        expect(
            len as usize == expected_len,
            "owner-index-mirror",
            std::format!("owner{} len={}", i, len),
        )?;
        for (pos, slot) in m
            .slots
            .iter()
            .flatten()
            .filter(|slot| slot.owner == i && slot.st != St::Settled)
            .enumerate()
        {
            let expected_id = commit_id(e, slot.counter);
            expect(
                actual.get(pos as u32) == Some(expected_id.clone()),
                "owner-index-content",
                std::format!("owner{} pos={} expected={:?}", i, pos, expected_id),
            )?;
        }
    }

    // Private storage dimensions that have no public aggregate getter are still
    // observable from this in-crate test. They make invalid-command rollback checks
    // cover the all-ID index and the reentrancy flag as well.
    let (all_ids, reentrancy_guard): (soroban_sdk::Vec<String>, bool) =
        e.as_contract(&w.core.address, || {
            (
                e.storage()
                    .instance()
                    .get(&DataKey::AllCommitmentIds)
                    .unwrap_or(soroban_sdk::Vec::new(e)),
                e.storage()
                    .instance()
                    .get(&DataKey::ReentrancyGuard)
                    .unwrap_or(false),
            )
        });
    expect(
        all_ids.len() as u64 == m.counter,
        "all-id-index-length",
        std::format!("ids={} counter={}", all_ids.len(), m.counter),
    )?;
    for counter in 0..m.counter {
        expect(
            all_ids.get(counter as u32) == Some(commit_id(e, counter)),
            "all-id-index-content",
            std::format!("counter={}", counter),
        )?;
    }
    expect(
        !reentrancy_guard,
        "reentrancy-guard-cleared",
        std::string::String::from("guard remained set after command"),
    )?;
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
    feed(&mut h, &m.creation_fees_accrued.to_le_bytes());
    feed(&mut h, &m.penalty_fees_accrued.to_le_bytes());
    feed(&mut h, &m.fees_withdrawn.to_le_bytes());
    feed(&mut h, &m.fee_bps.to_le_bytes());
    feed(&mut h, &m.now.to_le_bytes());
    feed(&mut h, &w.core.env.ledger().timestamp().to_le_bytes());
    feed(&mut h, &m.counter.to_le_bytes());
    for s in m.slots.iter().flatten() {
        feed(&mut h, &s.counter.to_le_bytes());
        feed(&mut h, &(s.owner as u64).to_le_bytes());
        feed(&mut h, &s.amount.to_le_bytes());
        feed(&mut h, &s.value.to_le_bytes());
        feed(&mut h, &s.released.to_le_bytes());
        feed(&mut h, &s.created_at.to_le_bytes());
        feed(&mut h, &s.expires_at.to_le_bytes());
        feed(&mut h, &(s.rules as u64).to_le_bytes());
        feed(&mut h, &s.max_loss.to_le_bytes());
        feed(&mut h, &s.penalty.to_le_bytes());
        feed(&mut h, &(s.st as u64).to_le_bytes());
    }
    for i in 0..3 {
        feed(&mut h, &m.bal_owner[i].to_le_bytes());
        feed(&mut h, &(m.owner_lists[i] as u64).to_le_bytes());
    }
    feed(&mut h, &m.bal_contract.to_le_bytes());
    feed(&mut h, &m.bal_recipient.to_le_bytes());
    feed(&mut h, &m.bal_pool.to_le_bytes());
    feed(&mut h, &w.core.get_total_value_locked().to_le_bytes());
    feed(&mut h, &w.core.get_collected_fees(&w.asset).to_le_bytes());
    feed(&mut h, &w.core.get_creation_fee_bps().to_le_bytes());
    feed(&mut h, &w.token.balance(&w.core.address).to_le_bytes());
    h
}

const CREATE_KIND: usize = 0;
const UPDATE_KIND: usize = 1;
const SETTLE_KIND: usize = 2;
const EXIT_KIND: usize = 3;
const ALLOCATE_KIND: usize = 4;
const SET_FEE_KIND: usize = 5;
const WITHDRAW_KIND: usize = 6;
const ADVANCE_KIND: usize = 7;
const COMMAND_KIND_COUNT: usize = 8;

fn command_kind(cmd: &Cmd) -> usize {
    match cmd {
        Cmd::Create { .. } => CREATE_KIND,
        Cmd::UpdateValue { .. } => UPDATE_KIND,
        Cmd::Settle { .. } => SETTLE_KIND,
        Cmd::EarlyExit { .. } => EXIT_KIND,
        Cmd::Allocate { .. } => ALLOCATE_KIND,
        Cmd::SetFeeBps { .. } => SET_FEE_KIND,
        Cmd::WithdrawFees { .. } => WITHDRAW_KIND,
        Cmd::AdvanceTime { .. } => ADVANCE_KIND,
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Coverage {
    accepted: [u64; COMMAND_KIND_COUNT],
    rejected: [u64; COMMAND_KIND_COUNT],
    reached_entrypoint: [u64; COMMAND_KIND_COUNT],
    accepted_same_updates: u64,
    accepted_downward_updates: u64,
    accepted_upward_recoveries: u64,
    rejected_updates: u64,
    terminal_or_invalid_updates: u64,
    unsafe_generated_upward_updates: u64,
}

impl Coverage {
    fn add(&mut self, other: &Self) {
        for i in 0..COMMAND_KIND_COUNT {
            self.accepted[i] += other.accepted[i];
            self.rejected[i] += other.rejected[i];
            self.reached_entrypoint[i] += other.reached_entrypoint[i];
        }
        self.accepted_same_updates += other.accepted_same_updates;
        self.accepted_downward_updates += other.accepted_downward_updates;
        self.accepted_upward_recoveries += other.accepted_upward_recoveries;
        self.rejected_updates += other.rejected_updates;
        self.terminal_or_invalid_updates += other.terminal_or_invalid_updates;
        self.unsafe_generated_upward_updates += other.unsafe_generated_upward_updates;
    }
}

#[derive(Clone, Copy, Debug)]
struct ScenarioOk {
    digest: u64,
    coverage: Coverage,
}

type ScenarioResult = Result<ScenarioOk, (usize, std::string::String)>;

/// Runs a full scenario in a fresh environment; returns the final digest or the
/// first failing step with its reason.
fn run_scenario(cmds: &[Cmd]) -> ScenarioResult {
    let e = Env::default();
    let w = setup_world(&e);
    let mut m = Model::new();
    let mut coverage = Coverage::default();
    verify(&w, &m).map_err(|reason| (0, std::format!("{} [cmd: initial-state]", reason)))?;
    for (step, cmd) in cmds.iter().enumerate() {
        let kind = command_kind(cmd);
        let reached = match *cmd {
            Cmd::UpdateValue { slot, .. }
            | Cmd::Settle { slot }
            | Cmd::EarlyExit { slot, .. }
            | Cmd::Allocate { slot, .. } => m.slots[slot].is_some(),
            _ => true,
        };
        let update_observation = match *cmd {
            Cmd::UpdateValue { slot, actor, .. } => match m.slots[slot] {
                Some(slot_state) => {
                    let new_value = compute_args(&m, cmd).nv;
                    Some((
                        Some(slot_state.value),
                        new_value,
                        Some(own_remaining_principal(&slot_state)),
                        slot_state.st != St::Active || actor != Actor::Authorized || new_value < 0,
                    ))
                }
                None => Some((None, 0, None, true)),
            },
            _ => None,
        };
        if matches!(update_observation, Some((_, _, _, true))) {
            coverage.terminal_or_invalid_updates += 1;
        }
        if let Some((Some(old_value), new_value, Some(own_remaining), _)) = update_observation {
            if new_value > old_value && new_value > own_remaining {
                coverage.unsafe_generated_upward_updates += 1;
            }
        }
        match apply(&w, &mut m, cmd) {
            Ok(accepted) => {
                if reached {
                    coverage.reached_entrypoint[kind] += 1;
                }
                if accepted {
                    coverage.accepted[kind] += 1;
                    if let Some((Some(old_value), new_value, Some(own_remaining), _)) =
                        update_observation
                    {
                        if new_value == old_value {
                            coverage.accepted_same_updates += 1;
                        } else if new_value < old_value {
                            coverage.accepted_downward_updates += 1;
                        } else if new_value <= own_remaining {
                            coverage.accepted_upward_recoveries += 1;
                        }
                    }
                } else {
                    coverage.rejected[kind] += 1;
                    if update_observation.is_some() {
                        coverage.rejected_updates += 1;
                    }
                }
            }
            Err(reason) => {
                return Err((step, std::format!("{} [cmd: {}]", reason, cmd_debug(cmd))));
            }
        }
    }
    Ok(ScenarioOk {
        digest: digest_of(&w, &m),
        coverage,
    })
}

fn failure_signature(reason: &str) -> std::string::String {
    let core = reason.split(" [cmd: ").next().unwrap_or(reason);
    let cause = if let Some(rest) = core.strip_prefix("invariant ") {
        if let Some((invariant, detail)) = rest.split_once(" violated: ") {
            let dimension = detail.split_whitespace().next().unwrap_or("no-detail");
            std::format!("invariant:{invariant}:{dimension}")
        } else {
            std::format!("invariant:{rest}")
        }
    } else {
        std::string::String::from(core)
    };
    let command = reason
        .split(" [cmd: ")
        .nth(1)
        .and_then(|part| part.strip_suffix(']'))
        .unwrap_or("unknown-command");
    std::format!("{}|{}", cause, command)
}

fn creation_bindings(cmds: &[Cmd]) -> std::vec::Vec<(usize, usize, usize)> {
    cmds.iter()
        .filter_map(|cmd| match *cmd {
            Cmd::Create {
                owner,
                amount_cls,
                rules,
            } if amount_cls < 4 => Some((owner, amount_cls, rules)),
            _ => None,
        })
        .collect()
}

fn preserves_slot_bindings(original: &[Cmd], candidate: &[Cmd]) -> bool {
    let original_bindings = creation_bindings(original);
    let candidate_bindings = creation_bindings(candidate);
    candidate.iter().all(|cmd| {
        let referenced = match *cmd {
            Cmd::UpdateValue { slot, .. }
            | Cmd::Settle { slot }
            | Cmd::EarlyExit { slot, .. }
            | Cmd::Allocate { slot, .. } => Some(slot),
            _ => None,
        };
        referenced.is_none_or(|slot| original_bindings.get(slot) == candidate_bindings.get(slot))
    })
}

fn minimize_preserving<F>(
    cmds: &[Cmd],
    target_signature: &str,
    mut failing_signature: F,
) -> std::vec::Vec<Cmd>
where
    F: FnMut(&[Cmd]) -> Option<std::string::String>,
{
    let mut cur: std::vec::Vec<Cmd> = cmds.to_vec();
    loop {
        let mut changed = false;
        for i in 0..cur.len() {
            let mut candidate = cur.clone();
            candidate.remove(i);
            if failing_signature(&candidate).as_deref() == Some(target_signature) {
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

/// Greedy delta-debugging minimization that preserves the original failure
/// signature and the semantic create-to-slot bindings of all remaining commands.
fn minimize(cmds: &[Cmd], failure: &(usize, std::string::String)) -> std::vec::Vec<Cmd> {
    let target_signature = failure_signature(&failure.1);
    minimize_preserving(cmds, &target_signature, |candidate| {
        if !preserves_slot_bindings(cmds, candidate) {
            return None;
        }
        run_scenario(candidate)
            .err()
            .map(|(_, reason)| failure_signature(&reason))
    })
}

fn failure_report(seed: u64, cmds: &[Cmd], failure: &(usize, std::string::String)) -> ! {
    let minimized = minimize(cmds, failure);
    let minimized_token = encode_commands(&minimized);
    let min_failure = run_scenario(&minimized)
        .err()
        .unwrap_or_else(|| (0, std::string::String::from("no longer fails")));
    panic!(
        "\nLIFECYCLE MODEL VIOLATION\n\
         seed: {}\n\
         reproduce: CGQA_LIFECYCLE_SEED={} cargo test -p commitment_core --lib lifecycle_model_seed_replay_entrypoint\n\
         replay minimized (POSIX): CGQA_LIFECYCLE_COMMANDS='{}' cargo test -p commitment_core --lib lifecycle_model_seed_replay_entrypoint\n\
         replay minimized (PowerShell): $env:CGQA_LIFECYCLE_COMMANDS='{}'; cargo test -p commitment_core --lib lifecycle_model_seed_replay_entrypoint\n\
         failing step: {} of {}\n\
         reason: {}\n\
         original sequence:\n  {}\n\
         minimized sequence ({} cmds):\n  {}\n\
         minimized failure at step {}: {}\n",
        seed,
        seed,
        minimized_token,
        minimized_token,
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
    let mut coverage = Coverage::default();
    for i in 0..SEED_COUNT {
        let seed = BASE_SEED + i;
        let mut rng = Rng::new(seed);
        let cmds = generate_commands(&mut rng, SEQ_LEN);
        match run_scenario(&cmds) {
            Ok(result) => coverage.add(&result.coverage),
            Err(failure) => failure_report(seed, &cmds, &failure),
        }
    }
    for kind in [
        CREATE_KIND,
        UPDATE_KIND,
        SETTLE_KIND,
        EXIT_KIND,
        ALLOCATE_KIND,
    ] {
        assert!(
            coverage.accepted[kind] > 0,
            "generated suite must accept command kind {kind}: {coverage:?}"
        );
    }
    assert!(
        coverage.accepted_downward_updates > 0,
        "generated suite must accept downward value updates: {coverage:?}"
    );
    assert!(
        coverage.accepted_upward_recoveries > 0,
        "generated suite must accept bounded upward recoveries: {coverage:?}"
    );
    assert_eq!(
        coverage.unsafe_generated_upward_updates, 0,
        "generated suite must not construct an over-budget upward update: {coverage:?}"
    );
    assert!(
        coverage.rejected_updates > 0,
        "generated suite must reject invalid update commands: {coverage:?}"
    );
    assert!(
        coverage.terminal_or_invalid_updates > 0,
        "generated suite must exercise terminal/invalid updates: {coverage:?}"
    );
    assert!(
        coverage.rejected.iter().sum::<u64>() > 0,
        "generated suite must exercise rejected commands: {coverage:?}"
    );
    let reached_lifecycle = [UPDATE_KIND, SETTLE_KIND, EXIT_KIND, ALLOCATE_KIND]
        .iter()
        .map(|kind| coverage.reached_entrypoint[*kind])
        .sum::<u64>();
    assert!(
        reached_lifecycle >= 100,
        "generated lifecycle commands must reach production entrypoints: {coverage:?}"
    );
    std::println!(
        "LIFECYCLE_COVERAGE same={} downward={} upward_recoveries={} rejected_updates={} terminal_or_invalid={} unsafe_upward={}",
        coverage.accepted_same_updates,
        coverage.accepted_downward_updates,
        coverage.accepted_upward_recoveries,
        coverage.rejected_updates,
        coverage.terminal_or_invalid_updates,
        coverage.unsafe_generated_upward_updates
    );
}

/// Single-seed replay hook used by the reproduction instructions above.
#[test]
fn lifecycle_model_seed_replay_entrypoint() {
    let seed = match std::env::var("CGQA_LIFECYCLE_SEED") {
        Ok(raw) => raw
            .parse::<u64>()
            .unwrap_or_else(|_| panic!("CGQA_LIFECYCLE_SEED must be a u64, got {raw:?}")),
        Err(std::env::VarError::NotPresent) => BASE_SEED,
        Err(error) => panic!("cannot read CGQA_LIFECYCLE_SEED: {error}"),
    };
    let cmds = match std::env::var("CGQA_LIFECYCLE_COMMANDS") {
        Ok(raw) => decode_commands(&raw)
            .unwrap_or_else(|error| panic!("invalid CGQA_LIFECYCLE_COMMANDS: {error}")),
        Err(std::env::VarError::NotPresent) => {
            let mut rng = Rng::new(seed);
            generate_commands(&mut rng, SEQ_LEN)
        }
        Err(error) => panic!("cannot read CGQA_LIFECYCLE_COMMANDS: {error}"),
    };
    if let Err(failure) = run_scenario(&cmds) {
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
        run_scenario(&cmds)
            .unwrap_or_else(|f| panic!("seed {} unexpectedly fails: {}", seed, f.1))
            .digest
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

fn command_trace_digest(cmds: &[Cmd]) -> u64 {
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    for cmd in cmds {
        for byte in cmd_debug(cmd).as_bytes() {
            digest ^= *byte as u64;
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    digest
}

#[test]
fn lifecycle_model_configured_seeds_are_unique() {
    let mut fingerprints = std::collections::BTreeSet::new();
    for i in 0..SEED_COUNT {
        let mut rng = Rng::new(BASE_SEED + i);
        let cmds = generate_commands(&mut rng, SEQ_LEN);
        assert!(
            fingerprints.insert(command_trace_digest(&cmds)),
            "configured seed {} aliases an earlier command trace",
            BASE_SEED + i
        );
    }
    assert_eq!(fingerprints.len(), SEED_COUNT as usize);
}

#[test]
fn lifecycle_command_token_replays_exact_sequence() {
    let mut rng = Rng::new(BASE_SEED + 11);
    let original = generate_commands(&mut rng, SEQ_LEN);
    let token = encode_commands(&original);
    let decoded = decode_commands(&token).expect("generated command token must decode");
    assert_eq!(encode_commands(&decoded), token);
    let original_result = run_scenario(&original).expect("original trace must pass");
    let replay_result = run_scenario(&decoded).expect("decoded trace must pass");
    assert_eq!(original_result.digest, replay_result.digest);
}

#[test]
fn lifecycle_command_token_rejects_invalid_input() {
    assert_eq!(
        decode_commands("").unwrap_err(),
        "command token must contain at least one command"
    );
    assert_eq!(
        decode_commands("   ").unwrap_err(),
        "command token must contain at least one command"
    );
    for token in [
        "C,3,0,0", "C,0,7,0", "C,0,0,3", "U,24,0,0", "U,0,7,0", "S,24", "E,24,0", "A,24,0,0",
        "A,0,5,0", "W,5,0",
    ] {
        assert!(
            decode_commands(token).is_err(),
            "out-of-range token unexpectedly decoded: {token:?}"
        );
    }
}

#[test]
fn lifecycle_minimizer_preserves_causal_recovery() {
    let prefix = deterministic_coverage_prefix();
    let original = std::vec![
        Cmd::SetFeeBps {
            bps: 100,
            outsider: false,
        },
        prefix[0],
        prefix[1],
        Cmd::AdvanceTime { seconds: 0 },
        prefix[2],
        Cmd::WithdrawFees {
            amount_cls: 4,
            outsider: false,
        },
    ];
    let target = "synthetic-safe-recovery";
    let minimized = minimize_preserving(&original, target, |candidate| {
        let mut value = None;
        for cmd in candidate {
            match *cmd {
                Cmd::Create {
                    owner: 0,
                    amount_cls: 0,
                    rules: 0,
                } if value.is_none() => value = Some(CREATE_AMOUNTS[0]),
                Cmd::UpdateValue {
                    slot: 0,
                    value_cls: VALUE_MILD_MARKDOWN,
                    actor: Actor::Authorized,
                } if value == Some(CREATE_AMOUNTS[0]) => value = Some(CREATE_AMOUNTS[0] - 1),
                Cmd::UpdateValue {
                    slot: 0,
                    value_cls: VALUE_RECOVER_WITHIN_OWN_PRINCIPAL,
                    actor: Actor::Authorized,
                } if value == Some(CREATE_AMOUNTS[0] - 1) => {
                    return Some(std::string::String::from(target));
                }
                _ => {}
            }
        }
        None
    });

    assert_eq!(encode_commands(&minimized), encode_commands(&prefix));
}

struct RecoverWithoutOwnPrincipalCap;

impl RecoverWithoutOwnPrincipalCap {
    fn new_value(slot: &Slot) -> i128 {
        own_remaining_principal(slot) + 1
    }
}

#[test]
fn lifecycle_recover_without_own_principal_cap_is_rejected() {
    let mut model = Model::new();
    model.slots[0] = Some(Slot {
        counter: 0,
        owner: 0,
        amount: 10,
        value: 9,
        released: 0,
        created_at: START_TS,
        expires_at: START_TS + DAY,
        rules: 0,
        max_loss: 10,
        penalty: 15,
        st: St::Active,
    });
    model.bal_contract = 100;
    let slot = model.slots[0].expect("local control slot");
    let own_remaining = own_remaining_principal(&slot);
    let mutant_value = RecoverWithoutOwnPrincipalCap::new_value(&slot);

    assert!(slot.value < mutant_value);
    assert!(mutant_value > own_remaining);

    assert!(!predicts_ok(
        &model,
        &Cmd::UpdateValue {
            slot: 0,
            value_cls: VALUE_RECOVER_WITHIN_OWN_PRINCIPAL,
            actor: Actor::Authorized,
        },
        &Args {
            nv: mutant_value,
            ..Args::default()
        },
    ));

    let mut invalid_slot = slot;
    invalid_slot.released = invalid_slot.amount + 1;
    assert!(std::panic::catch_unwind(|| own_remaining_principal(&invalid_slot)).is_err());
}

/// Negative control: an "any error" minimizer would drop the causal command
/// because the unrelated command also produces an error. The hardened minimizer
/// must retain the command that produces the requested failure signature.
#[test]
fn lifecycle_minimizer_preserves_causal_failure() {
    let causal = Cmd::SetFeeBps {
        bps: 10_001,
        outsider: false,
    };
    let unrelated = Cmd::WithdrawFees {
        amount_cls: 4,
        outsider: false,
    };
    let original = std::vec![causal, unrelated];
    let minimized = minimize_preserving(&original, "causal", |candidate| {
        if candidate
            .iter()
            .any(|cmd| matches!(cmd, Cmd::SetFeeBps { bps: 10_001, .. }))
        {
            Some(std::string::String::from("causal"))
        } else if candidate
            .iter()
            .any(|cmd| matches!(cmd, Cmd::WithdrawFees { .. }))
        {
            Some(std::string::String::from("different"))
        } else {
            None
        }
    });
    assert_eq!(minimized.len(), 1);
    assert!(matches!(minimized[0], Cmd::SetFeeBps { bps: 10_001, .. }));

    let first = Cmd::Create {
        owner: 0,
        amount_cls: 0,
        rules: 0,
    };
    let second = Cmd::Create {
        owner: 1,
        amount_cls: 1,
        rules: 1,
    };
    let target = Cmd::UpdateValue {
        slot: 0,
        value_cls: VALUE_RECOVER_WITHIN_OWN_PRINCIPAL,
        actor: Actor::Authorized,
    };
    assert!(!preserves_slot_bindings(
        &[first, second, target],
        &[second, target]
    ));

    let settle = failure_signature(
        "invariant status-mirror violated: record mismatch [cmd: Settle(slot=0)]",
    );
    let update = failure_signature(
        "invariant status-mirror violated: record mismatch [cmd: UpdateValue(slot=0)]",
    );
    assert_eq!(settle, "invariant:status-mirror:record|Settle(slot=0)");
    assert_ne!(settle, update);
    assert_eq!(
        failure_signature("plain failure"),
        "plain failure|unknown-command"
    );
}

// ---------------------------------------------------------------------------
// Hand-written regression scripts (portable counterexample coverage)
// ---------------------------------------------------------------------------

fn script(cmds: std::vec::Vec<Cmd>) -> u64 {
    run_scenario(&cmds)
        .unwrap_or_else(|f| panic!("regression script failed at step {}: {}", f.0, f.1))
        .digest
}

#[test]
fn safe_value_recovery_stays_within_slot_principal() {
    let e = Env::default();
    let w = setup_world(&e);
    let mut model = Model::new();
    let prefix = deterministic_coverage_prefix();

    assert!(apply(&w, &mut model, &prefix[0]).expect("create must verify"));
    let created = model.slots[0].expect("created slot");
    let principal = created.amount;
    let expected_tvl = model.tvl;
    let custody_before_updates = w.token.balance(&w.core.address);

    assert!(apply(&w, &mut model, &prefix[1]).expect("mild markdown must verify"));
    let markdown = model.slots[0].expect("markdown slot");
    assert_eq!(markdown.st, St::Active);
    assert_eq!(markdown.value, principal - 1);

    assert!(apply(&w, &mut model, &prefix[2]).expect("safe recovery must verify"));
    let recovered = model.slots[0].expect("recovered slot");
    let own_remaining = own_remaining_principal(&recovered);
    assert_eq!(recovered.st, St::Active);
    assert_eq!(recovered.value, principal);
    assert!(recovered.value <= own_remaining);
    assert_eq!(model.tvl, expected_tvl);
    assert_eq!(w.core.get_total_value_locked(), expected_tvl);
    assert_eq!(w.token.balance(&w.core.address), custody_before_updates);

    assert!(apply(&w, &mut model, &Cmd::AdvanceTime { seconds: DAY })
        .expect("time advance must verify"));
    let owner = addr(&e, OWNER_STRS[0]);
    let owner_before_settle = w.token.balance(&owner);
    assert!(apply(&w, &mut model, &Cmd::Settle { slot: 0 }).expect("settle must verify"));
    let settled = model.slots[0].expect("settled slot");
    assert_eq!(settled.released, principal);
    assert!(settled.released <= settled.amount);
    assert_eq!(w.token.balance(&owner) - owner_before_settle, principal);

    let owners = OWNER_STRS
        .iter()
        .map(|owner| w.token.balance(&addr(&e, owner)))
        .sum::<i128>();
    let observed_supply = owners
        + w.token.balance(&w.core.address)
        + w.token.balance(&addr(&e, RECIPIENT_STR))
        + w.token.balance(&addr(&e, POOL_STR));
    assert_eq!(observed_supply, OWNER_MINT * 3);
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
        Cmd::AdvanceTime { seconds: DAY },
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
            value_cls: VALUE_SAME,
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
            value_cls: VALUE_DEEP_LOSS,
            actor: Actor::Authorized
        }, // deep loss -> violated
        Cmd::UpdateValue {
            slot: 1,
            value_cls: VALUE_RECOVER_WITHIN_OWN_PRINCIPAL,
            actor: Actor::Authorized
        }, // violated rejects
        Cmd::AdvanceTime { seconds: 7 * DAY },
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
        Cmd::AdvanceTime { seconds: 30 * DAY },
        Cmd::Settle { slot: 2 },
        Cmd::Settle { slot: 2 }, // double settle
        Cmd::UpdateValue {
            slot: 2,
            value_cls: VALUE_RECOVER_WITHIN_OWN_PRINCIPAL,
            actor: Actor::Authorized
        },
        Cmd::EarlyExit {
            slot: 2,
            wrong_caller: false
        },
        Cmd::Allocate {
            slot: 2,
            amount_cls: 0,
            actor: Actor::Authorized
        },
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
            value_cls: VALUE_SAME,
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
        Cmd::AdvanceTime { seconds: 7 * DAY },
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
            // Aggressive permits a 100% loss, so value=1 stays active and the
            // 5% penalty genuinely truncates to zero before early_exit.
            rules: 2
        },
        Cmd::UpdateValue {
            slot: 1,
            value_cls: VALUE_TINY,
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

/// Exact temporal boundary: expiry-1 rejects, expiry accepts, and expiry+1 accepts
/// another active commitment; a repeated settle remains atomic (I4/I5).
#[test]
fn regression_settle_ordering_guards() {
    script(std::vec![
        Cmd::Create {
            owner: 1,
            amount_cls: 2,
            rules: 0
        },
        Cmd::Create {
            owner: 2,
            amount_cls: 2,
            rules: 0
        },
        Cmd::AdvanceTime { seconds: DAY - 1 },
        Cmd::Settle { slot: 0 }, // expiry - 1s -> rejected
        Cmd::AdvanceTime { seconds: 1 },
        Cmd::Settle { slot: 0 }, // exactly expiry -> accepted
        Cmd::AdvanceTime { seconds: 1 },
        Cmd::Settle { slot: 1 }, // expiry + 1s -> accepted
        Cmd::Settle { slot: 0 }, // duplicate terminal op -> rejected
    ]);
}

/// Stateful composition of the maximum valid creation fee and the first invalid
/// bps value. A 100% fee creates a zero-net commitment without losing fee custody.
#[test]
fn regression_fee_bps_boundaries_compose_with_lifecycle() {
    script(std::vec![
        Cmd::SetFeeBps {
            bps: 10_001,
            outsider: false
        }, // invalid and atomic
        Cmd::SetFeeBps {
            bps: 10_000,
            outsider: false
        },
        Cmd::Create {
            owner: 2,
            amount_cls: 0,
            rules: 2
        }, // 1000 gross, 0 net, 1000 collected fee
        Cmd::EarlyExit {
            slot: 0,
            wrong_caller: false
        },
        Cmd::WithdrawFees {
            amount_cls: 0,
            outsider: false
        },
    ]);
}

/// Zero and negative amounts reach the real entrypoints, reject atomically, and
/// do not poison a later healthy lifecycle or fee withdrawal.
#[test]
fn regression_signed_amount_boundaries_and_failed_op_recovery() {
    script(std::vec![
        Cmd::Create {
            owner: 0,
            amount_cls: 5,
            rules: 0
        },
        Cmd::Create {
            owner: 0,
            amount_cls: 6,
            rules: 0
        },
        Cmd::SetFeeBps {
            bps: 100,
            outsider: false
        },
        Cmd::Create {
            owner: 0,
            amount_cls: 0,
            rules: 2
        },
        Cmd::UpdateValue {
            slot: 0,
            value_cls: VALUE_NEGATIVE,
            actor: Actor::Authorized
        },
        Cmd::Allocate {
            slot: 0,
            amount_cls: 4,
            actor: Actor::Authorized
        },
        Cmd::WithdrawFees {
            amount_cls: 3,
            outsider: false
        },
        Cmd::WithdrawFees {
            amount_cls: 4,
            outsider: false
        },
        Cmd::WithdrawFees {
            amount_cls: 0,
            outsider: false
        }, // succeeds after unrelated rejected operations
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
        Cmd::AdvanceTime { seconds: 7 * DAY },
        Cmd::Settle { slot: 0 }, // settles the drained remainder
    ]);
}
