# Timelock Parameter Runbook

## Timelock parameter runbook

This runbook covers how to operate the on-chain `time_lock` contract safely in production-leaning environments.

## Current on-chain parameter model

`time_lock` uses fixed minimum delays by action type plus a hard global maximum:

| Action type | Minimum delay | Intended use |
| --- | ---: | --- |
| `ParameterChange` | 86,400 seconds (1 day) | Routine operational parameter changes with limited blast radius |
| `FeeChange` | 86,400 seconds (1 day) | Fee schedule changes that should still allow user review |
| `AdminChange` | 172,800 seconds (2 days) | Transfer of governance authority or signer rotation |
| `Upgrade` | 259,200 seconds (3 days) | Contract code upgrades and other high-impact changes |
| Global maximum | 2,592,000 seconds (30 days) | Upper bound for any queued action |

## Operator goals

- Give users and reviewers enough time to inspect queued changes before execution.
- Match delay length to blast radius: larger governance power should mean longer delay.
- Keep delays deterministic and simple enough that downstream contracts and operators can reason about them.

## Recommended operating procedure

1. Classify the change before queueing it.
   Use `Upgrade` for code changes, `AdminChange` for authority transfers, and the narrower parameter types for operational tuning.

2. Choose a delay at or above the action minimum.
   Do not treat the minimum as the default for every change. If a parameter update could affect solvency, liquidation timing, or user withdrawal behavior, prefer a longer delay than the floor.

3. Encode the action data clearly.
   Store enough structured information in `data` for reviewers to understand the queued change without ambiguity.

4. Record the execution timestamp at queue time.
   Operators should capture the returned `action_id`, `queued_at`, and `executable_at` from `get_action(action_id)` and circulate that information internally.

5. Review before execution.
   Before executing, confirm the action is still intended, not cancelled, and that the target address and payload still match the approved change request.

6. Execute after the delay window opens.
   Execution is permissionless after `executable_at`, so any operator or automation that observes the contract can trigger it.

7. Cancel instead of superseding when intent changes.
   If the planned action is no longer correct, cancel the queued action and submit a new one rather than assuming reviewers will infer the latest intent.

## Parameter selection guidance

### `ParameterChange`

Use for lower-risk operational changes where a one-day review window is sufficient.

Choose longer than one day when:
- the parameter affects user balances or withdrawal timing
- the change interacts with liquidation or settlement paths
- the parameter is hard to roll back safely

### `FeeChange`

Use for fee schedule updates. The one-day minimum is a floor, not a recommendation for every market.

Choose longer than one day when:
- fees materially change user economics
- the fee change is bundled with another governance action
- users need advance notice for fairness or compliance reasons

### `AdminChange`

Use for signer rotation, multisig replacement, or governance transfer.

Operational guidance:
- verify the new admin can authenticate on Soroban before queueing
- use a longer delay if the change also alters operational processes or key custody

### `Upgrade`

Use for WASM upgrades or changes with equivalent blast radius.

Operational guidance:
- prefer the longest practical review window
- ensure code review, diff review, and test evidence are complete before execution
- never bundle emergency reasoning into a routine upgrade if the contract does not implement a separate emergency path

## Edge cases operators should know

- **Clock skew assumptions**: Soroban ledger timestamps are monotonic and provided by consensus. The timelock treats the ledger timestamp as the canonical clock. Delays are relative to the ledger time at the moment of `queue_action`.
- **Precision**: Timestamps are in seconds. Because ledgers close at discrete intervals, an action scheduled for time $T$ becomes executable in the first ledger where $T_{ledger} \ge T$.
- **Zero/Short delay**: Not permitted. Every action type has a minimum delay defined by `get_delay()` (minimum 1 day).
- **Max delay**: Hard capped at 30 days (`MAX_DELAY`) to prevent governance deadlock or operator error resulting in extreme lock periods.
- **Execution Window**: Execution remains open indefinitely after the delay unless explicitly cancelled by the admin.
- **Immutability**: Executed actions cannot be cancelled, and cancelled actions cannot be executed.

## Security notes

- Queueing and cancellation require admin authorization.
- Execution is intentionally permissionless after the delay to reduce liveness dependence on the admin.
- The contract stores metadata for queued actions but does not itself enforce off-chain review or approval workflow quality; operators must supply that process.

## Local and CI test guidance

For deterministic `time_lock` tests around schedule/execute/cancel behavior:

1. Seed explicit ledger timestamps before boundary checks.
2. Validate role checks separately:
   - schedule (`queue_action`) requires admin auth
   - cancel (`cancel_action`) requires admin auth
   - execute (`execute_action`) is permissionless once delay is met
3. Assert delay boundaries with exact and off-by-one cases:
   - `current_time == executable_at` should execute
   - `current_time == executable_at - 1` should fail with `DelayNotMet`

Recommended commands:

```bash
cargo test -p time_lock
cargo build -p time_lock --target wasm32v1-none --release
```

Note: `cargo test --target wasm32v1-none` is not supported on this target because
Rust's `test` harness crate is unavailable for `wasm32v1-none`.
