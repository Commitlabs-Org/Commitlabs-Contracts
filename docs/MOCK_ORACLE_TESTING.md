# mock_oracle in Local and CI Soroban Tests

This guide shows how to use `mock_oracle` for deterministic Soroban contract tests.

## Purpose

`mock_oracle` is a testing utility contract that lets tests control:

- per-asset prices
- freshness (staleness) boundaries
- failure modes (paused oracle, missing prices)
- feeder authorization

It is intended for local and CI test environments where reproducibility matters.

## Local Test Pattern

Use this flow in unit or integration tests:

1. Register contract in `Env`.
2. Initialize once with admin and staleness threshold.
3. Seed prices with `set_price`.
4. Assert reads with `get_price` / `get_price_data`.

```rust
let env = Env::default();
env.mock_all_auths_allowing_non_root_auth();
let oracle_id = env.register_contract(None, MockOracleContract);
let admin = Address::generate(&env);
let asset = Address::generate(&env);

env.as_contract(&oracle_id, || {
    MockOracleContract::initialize(env.clone(), admin.clone(), 3600).unwrap();
    MockOracleContract::set_price(
        env.clone(),
        admin.clone(),
        asset.clone(),
        100_000_000,
        8,
        1000,
    )
    .unwrap();

    assert_eq!(
        MockOracleContract::get_price(env.clone(), asset.clone()).unwrap(),
        100_000_000
    );
});
```

## CI Deterministic Pattern

Avoid real-time assumptions in CI by pinning ledger timestamp and using explicit
price timestamps.

1. Set a fixed ledger timestamp.
2. Write oracle data with `set_price_with_timestamp`.
3. Assert stale/fresh boundaries exactly.

```rust
const FIXED_TS: u64 = 1_704_067_200; // Jan 1, 2024 UTC

env.ledger().with_mut(|l| {
    l.timestamp = FIXED_TS;
});

env.as_contract(&oracle_id, || {
    MockOracleContract::initialize(env.clone(), admin.clone(), 60).unwrap();

    // Force stale result
    MockOracleContract::set_price_with_timestamp(
        env.clone(),
        admin.clone(),
        asset.clone(),
        90_000_000,
        FIXED_TS - 61,
        8,
        1000,
    )
    .unwrap();
    assert_eq!(
        MockOracleContract::get_price(env.clone(), asset.clone()),
        Err(OracleError::StalePrice)
    );
});
```

## Failure-Path Simulation

Use these methods to test downstream error handling:

- `pause` / `unpause`: simulate oracle outage and recovery.
- `remove_price`: simulate missing feed.
- `set_staleness_threshold`: tighten/relax freshness acceptance.

## Auth and Trust Boundaries

- `set_price` and `set_price_with_timestamp` require authenticated caller that is:
  - admin, or
  - authorized feeder.
- `add_feeder`, `remove_feeder`, `pause`, `unpause`, `remove_price`,
  `set_staleness_threshold` are admin-only.
- Latest-price overwrite model: authorized feeder updates replace previous value for that asset.

## Recommended Commands

Host tests:

```bash
cargo test -p mock_oracle
```

WASM build check:

```bash
cargo build -p mock_oracle --target wasm32v1-none --release
```

Combined host+WASM check script:

```bash
scripts/test-mock-oracle.sh
```

Note: `cargo test -p mock_oracle --target wasm32v1-none --release` is not
supported because Rust's `test` harness crate is unavailable on `wasm32v1-none`.
Use host tests plus WASM release build checks in CI.
