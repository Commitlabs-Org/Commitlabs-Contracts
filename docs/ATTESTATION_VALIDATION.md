# Attestation Validation Implementation

## Requirement
Attest for a commitment_id that does not exist in commitment_core should fail or be explicitly allowed (e.g. for pre-registration).

## Implementation

### Core Validation Logic

The attestation engine validates that commitments exist before allowing attestations through the `commitment_exists` function:

**Location:** `contracts/attestation_engine/src/lib.rs:465-486`

```rust
fn commitment_exists(e: &Env, commitment_id: &String) -> bool {
    let commitment_core: Address = match e.storage().instance().get(&DataKey::CoreContract) {
        Some(addr) => addr,
        None => return false,
    };

    // Try to get commitment from core contract
    let mut args = Vec::new(e);
    args.push_back(commitment_id.clone().into_val(e));

    // Use try_invoke_contract to handle potential failures
    let result = e.try_invoke_contract::<Val, soroban_sdk::Error>(
        &commitment_core,
        &Symbol::new(e, "get_commitment"),
        args,
    );

    match result {
        Ok(Ok(_)) => true,
        _ => false,
    }
}
```

### Attestation Flow with Validation

The `attest` function (lines 634-791) includes validation at step 5:

```rust
// 5. Validate commitment exists in core contract
if !Self::commitment_exists(&e, &commitment_id) {
    e.storage().instance().remove(&DataKey::ReentrancyGuard);
    return Err(AttestationError::CommitmentNotFound);
}
```

### Error Type

**Location:** `contracts/attestation_engine/src/lib.rs` (AttestationError enum)

```rust
pub enum AttestationError {
    ...
    CommitmentNotFound = 5,
    ...
}
```

## Test Cases

### Test 1: Attestation fails for nonexistent commitment

**Location:** `tests/integration/cross_contract_tests.rs:129-152`

```rust
#[test]
fn test_attestation_fails_for_nonexistent_commitment() {
    let harness = TestHarness::new();
    let verifier = &harness.accounts.verifier;

    let fake_commitment_id = String::from_str(&harness.env, "nonexistent_commitment");
    let attestation_data = harness.health_check_data();

    // Attempt to create attestation for non-existent commitment
    let result = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::attest(
                harness.env.clone(),
                verifier.clone(),
                fake_commitment_id,
                String::from_str(&harness.env, "health_check"),
                attestation_data,
                true,
            )
        });

    // Should fail with CommitmentNotFound error
    assert_eq!(result, Err(AttestationError::CommitmentNotFound));
}
```

### Test 2: Attestation succeeds after commitment is created

**Location:** `tests/integration/cross_contract_tests.rs:154-217`

```rust
#[test]
fn test_attestation_succeeds_after_commitment_created() {
    let harness = TestHarness::new();
    let user = &harness.accounts.user1;
    let verifier = &harness.accounts.verifier;
    let amount = 1_000_000_000_000i128;
    let commitment_id = String::from_str(&harness.env, "test_commitment_123");

    // First attempt: attestation should fail (commitment doesn't exist yet)
    let result_before = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::attest(
                harness.env.clone(),
                verifier.clone(),
                commitment_id.clone(),
                String::from_str(&harness.env, "health_check"),
                harness.health_check_data(),
                true,
            )
        });
    assert_eq!(result_before, Err(AttestationError::CommitmentNotFound));

    // Create commitment in core contract
    harness.approve_tokens(user, &harness.contracts.commitment_core, amount);
    let created_id = harness
        .env
        .as_contract(&harness.contracts.commitment_core, || {
            CommitmentCoreContract::create_commitment(
                harness.env.clone(),
                user.clone(),
                amount,
                harness.contracts.token.clone(),
                harness.default_rules(),
            )
        });

    // Second attempt: attestation should succeed (commitment now exists)
    let result_after = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::attest(
                harness.env.clone(),
                verifier.clone(),
                created_id.clone(),
                String::from_str(&harness.env, "health_check"),
                harness.health_check_data(),
                true,
            )
        });
    assert!(result_after.is_ok());

    // Verify attestation was stored
    let attestations = harness
        .env
        .as_contract(&harness.contracts.attestation_engine, || {
            AttestationEngineContract::get_attestations(harness.env.clone(), created_id)
        });
    assert_eq!(attestations.len(), 1);
}
```

### Test 3: Cross-contract verification

**Location:** `tests/integration/cross_contract_tests.rs:77-125`

```rust
#[test]
fn test_attestation_engine_verifies_commitment_exists() {
    // Creates commitment in core contract
    // Verifies attestation succeeds via cross-contract call
    // Confirms attestation was stored
}
```

## Acceptance Criteria

✅ **Behavior is defined:** Attestations for nonexistent commitments return `AttestationError::CommitmentNotFound`

✅ **Cross-contract validation:** The attestation engine calls `commitment_core.get_commitment()` to verify existence

✅ **Test coverage:**
- Test for failure case (nonexistent commitment)
- Test for success case (existing commitment)
- Test for transition (fails before creation, succeeds after)

## How It Works

1. When `attest()` is called, it validates the commitment_id is not empty
2. It then calls `commitment_exists()` which performs a cross-contract call to `commitment_core.get_commitment()`
3. If the commitment doesn't exist, the call fails and returns `false`
4. The attest function returns `Err(AttestationError::CommitmentNotFound)`
5. If the commitment exists, attestation proceeds normally

This ensures data integrity - attestations can only be created for commitments that actually exist in the core contract.

> **Update (v2):** `attest`, `record_fees`, `record_drawdown`, and batch items now
> require a caller-supplied `evidence_hash`; duplicate or zero evidence is
> rejected before fee collection or storage mutation. See
> `ATTESTATION_TYPES_AND_COMPLIANCE.md` → "Replay protection and type scopes".
