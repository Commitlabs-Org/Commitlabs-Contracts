# Commitment Core Formal Verification Scope

This document is a placeholder and scoping artifact for future formal verification work on `contracts/commitment_core`.

It does not claim any proof has been completed. Its purpose is to:
- define the proof boundary for the highest-risk `commitment_core` flows
- list the first lemmas worth proving
- make review assumptions explicit before any tool-specific specification work begins

## Goals

The initial formal verification effort should focus on safety properties for:
- asset custody
- authorization
- reentrancy exclusion
- arithmetic soundness
- lifecycle-state consistency

The first pass should prioritize proving that no reachable execution of the modeled contract can:
- move funds without the intended authorization boundary
- leave `ReentrancyGuard` stuck `true`
- create negative or silently overflowed accounting values
- violate core conservation relations between commitment value, TVL, and collected fees

## Out Of Scope For The First Pass

These items should be reviewed later, but should not block the first formalization:
- event completeness or event ordering as an externally visible API guarantee
- gas/storage growth bounds for analytics helpers such as `get_commitments_created_between`
- off-chain indexer assumptions
- frontend, backend, or monitoring behavior
- end-to-end proofs spanning unrelated contracts outside the `commitment_core <-> commitment_nft` boundary

## Verification Boundary

### Contract Under Analysis

- `contracts/commitment_core/src/lib.rs`

### State Variables In Scope

- `DataKey::Commitment(String)`
- `DataKey::OwnerCommitments(Address)`
- `DataKey::TotalCommitments`
- `DataKey::TotalValueLocked`
- `DataKey::ReentrancyGuard`
- `DataKey::AuthorizedAllocator(Address)`
- `DataKey::AuthorizedUpdaters`
- `DataKey::AllCommitmentIds`
- `DataKey::FeeRecipient`
- `DataKey::CreationFeeBps`
- `DataKey::CollectedFees(Address)`

### Entry Points In Scope

Priority 1:
- `create_commitment`
- `settle`
- `early_exit`
- `allocate`
- `withdraw_fees`

Priority 2:
- `update_value`
- `set_creation_fee_bps`
- `set_fee_recipient`
- `add_authorized_contract`
- `remove_authorized_contract`
- `pause`
- `unpause`
- `set_emergency_mode`
- `emergency_withdraw`

### Trusted External Assumptions

The first proof pass should model the following as assumptions rather than re-prove them:
- Soroban transaction rollback is atomic for a failed invocation.
- `token::Client::transfer` and `token::Client::balance` match the token contract’s documented behavior.
- downstream `commitment_nft` calls are treated as external calls that may succeed or revert, but must not be trusted for local invariant preservation

Those assumptions should be stated explicitly in any future K/LEVER/Coq/Isabelle/Creusot/Prusti spec, rather than left implicit.

## Threat Model Alignment

This scope is derived from:
- [CORE_NFT_ATTESTATION_THREAT_REVIEW.md](/home/olowo/Desktop/Nathan/Commitlabs-Contracts/docs/CORE_NFT_ATTESTATION_THREAT_REVIEW.md)
- [SECURITY_CHECKLIST.md](/home/olowo/Desktop/Nathan/Commitlabs-Contracts/docs/SECURITY_CHECKLIST.md)

The proof effort should especially cover the threat-review concerns around:
- auth drift across privileged lifecycle mutations
- partial-state risk around outbound NFT calls
- reentrancy across token/NFT contract boundaries
- arithmetic corruption in TVL and fee accounting

## Suggested Specification Strategy

### Phase 1: Local Safety Invariants

Model only `commitment_core` storage transitions and external-call success or revert.

Focus on:
- preconditions
- postconditions
- state invariants
- revert-path cleanup

### Phase 2: Cross-Contract Stubs

Replace direct assumptions about `commitment_nft` and token contracts with explicit adversarial stubs:
- NFT call may revert at any time
- token transfer may revert
- external calls must not observe `ReentrancyGuard == false` during a guarded flow

### Phase 3: Strengthen To Refinement Claims

If the local model succeeds, add stronger claims that storage updates refine the intended lifecycle state machine.

## First Critical Lemmas List

These are the first lemmas worth expressing, even if they begin as placeholders or proof TODOs.

### AUTH-1: Owner authorization on commitment creation

Source:
- `create_commitment`

Statement:
- If `create_commitment` returns successfully, then `owner.require_auth()` must have been satisfied for the supplied `owner`.

Why it matters:
- This is the front door for asset custody and commitment creation.

### AUTH-2: Owner-only early exit

Source:
- `early_exit`

Statement:
- If `early_exit(commitment_id, caller)` returns successfully, then `caller == commitment.owner` for the pre-state commitment and `caller.require_auth()` held.

Why it matters:
- Early exit moves assets back to the caller and accrues protocol fees.

### AUTH-3: Admin-only privileged configuration

Source:
- `pause`
- `unpause`
- `add_authorized_contract`
- `remove_authorized_contract`
- `set_creation_fee_bps`
- `set_fee_recipient`
- `set_emergency_mode`
- `emergency_withdraw`

Statement:
- Successful execution of these entry points implies the caller passed `require_admin`, which itself implies authenticated caller identity and equality to `DataKey::Admin`.

Why it matters:
- These functions define the contract’s privileged trust boundary.

### AUTH-4: Authorized allocator gate

Source:
- `allocate`

Statement:
- If `allocate` returns successfully, then the caller was authenticated and satisfied `is_authorized`.

Why it matters:
- `allocate` moves assets out of custody and updates commitment value.

### REENT-1: Guarded entry exclusion

Source:
- `create_commitment`
- `settle`
- `early_exit`
- `allocate`
- `withdraw_fees`

Statement:
- If `ReentrancyGuard == true` in pre-state, each guarded entry point reverts before any protected state transition or external call.

Why it matters:
- This is the core non-reentrancy claim for custody flows.

### REENT-2: Guard cleanup on all local exit paths

Source:
- `create_commitment`
- `settle`
- `early_exit`
- `allocate`
- `withdraw_fees`

Statement:
- For every locally handled revert path after the guard is set, the contract resets `ReentrancyGuard` to `false` before failing.

Why it matters:
- Without this property, one failed operation can brick later operations.

### REENT-3: External calls only while guarded

Source:
- `create_commitment`
- `settle`
- `early_exit`
- `allocate`
- `withdraw_fees`

Statement:
- Every outbound token or NFT call from a guarded flow occurs only after `ReentrancyGuard` has been set to `true` and before it is cleared.

Why it matters:
- This is the strongest local defense against cross-contract reentry.

### ARITH-1: No negative commitment amount on creation

Source:
- `create_commitment`

Statement:
- On successful creation, stored `commitment.amount >= 0` and `commitment.current_value >= 0`.

Why it matters:
- Fee deduction and initial accounting must not create invalid negative balances.

### ARITH-2: Fee accounting does not overflow silently

Source:
- `create_commitment`
- `early_exit`
- `withdraw_fees`

Statement:
- Any update to `CollectedFees(asset)` either succeeds with exact checked arithmetic or reverts; silent wraparound is unreachable.

Why it matters:
- Protocol revenue accounting is a high-value target and must remain exact.

### ARITH-3: TVL accounting does not overflow silently

Source:
- `create_commitment`
- `update_value`
- `settle`
- `early_exit`

Statement:
- Any update to `TotalValueLocked` either succeeds with checked arithmetic or reverts; silent wraparound is unreachable.

Why it matters:
- TVL is a headline safety/accounting metric and a common invariant anchor for other proofs.

### LIFE-1: Successful creation produces an active commitment

Source:
- `create_commitment`

Statement:
- If `create_commitment` succeeds, the stored commitment exists, has status `"active"`, and its ID appears in both `OwnerCommitments(owner)` and `AllCommitmentIds`.

Why it matters:
- This is the root lifecycle-state initialization claim.

### LIFE-2: Settle removes active exposure

Source:
- `settle`

Statement:
- If `settle` succeeds, the commitment status becomes `"settled"` and the settled commitment ID is removed from `OwnerCommitments(owner)`.

Why it matters:
- Owner lists and settlement state must remain aligned.

### LIFE-3: Early exit zeroes live value

Source:
- `early_exit`

Statement:
- If `early_exit` succeeds, then post-state `commitment.status == "early_exit"` and `commitment.current_value == 0`.

Why it matters:
- This prevents double counting after user funds have been returned.

### LIFE-4: Allocate preserves non-negative current value

Source:
- `allocate`

Statement:
- If `allocate` succeeds, then post-state `commitment.current_value == pre.commitment.current_value - amount` and remains non-negative.

Why it matters:
- Allocation is a partial outflow and must not create underflowed commitment balances.

### CONS-1: Early exit conservation relation

Source:
- `early_exit`

Statement:
- If `early_exit` succeeds, then `pre.current_value == penalty + returned`, where `penalty` is added to `CollectedFees(asset)` and `returned` is the amount transferred to the owner.

Why it matters:
- This is the most important local conservation lemma for fee retention.

### CONS-2: Creation conservation relation

Source:
- `create_commitment`

Statement:
- If `create_commitment` succeeds, then `amount == creation_fee + net_amount`, with `net_amount` becoming both stored commitment amount/current value and the increment applied to `TotalValueLocked`.

Why it matters:
- This ties external funding, protocol fees, and stored accounting together.

### CONS-3: Fee withdrawal conservation relation

Source:
- `withdraw_fees`

Statement:
- If `withdraw_fees(asset, amount)` succeeds, then post-state `CollectedFees(asset) == pre.CollectedFees(asset) - amount`.

Why it matters:
- Prevents fee over-withdrawal and supports treasury accounting soundness.

### READ-1: Public read path is side-effect free

Source:
- `get_commitment`
- `get_owner_commitments`
- `get_total_commitments`
- `get_total_value_locked`
- `get_creation_fee_bps`
- `get_fee_recipient`
- `get_collected_fees`

Statement:
- These functions do not mutate contract storage.

Why it matters:
- Downstream contracts such as `attestation_engine` rely on these as read-only composition points.

## First Lemma Priority Order

Recommended execution order:
1. `AUTH-1`
2. `AUTH-2`
3. `AUTH-4`
4. `REENT-1`
5. `REENT-2`
6. `CONS-2`
7. `CONS-1`
8. `ARITH-2`
9. `ARITH-3`
10. `LIFE-2`

This ordering prioritizes catastrophic-failure classes first: unauthorized transfer, reentrancy, and broken accounting.

## Placeholder Deliverables For The Next Issue

The next formal-verification issue should produce:
- a machine-readable invariant/spec file for `commitment_core`
- explicit modeling decisions for token/NFT external calls
- one mechanically checked proof or bounded model for at least one Priority 1 lemma
- a traceability table mapping each lemma here to proof status: `unstarted`, `specified`, `proved`, or `blocked`

## Current Status

- Scope document: present
- Lemma inventory: present
- Machine-checked proofs: not started
- Tooling decision: not made
