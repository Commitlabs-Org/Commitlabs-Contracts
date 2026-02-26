# Implementation Plan: Commitment ID Uniqueness and Format

## Overview

Currently, the `commitment_nft` contract accepts `commitment_id` as a user-provided string parameter without enforcing uniqueness. This implementation adds:

1. **Uniqueness Enforcement**: Each `commitment_id` must be unique per commitment
2. **Format Consistency**: All `commitment_id`s follow a consistent format
3. **Query by ID**: A new function to retrieve commitments by `commitment_id`

---

## Current State Analysis

### Existing Implementation

- **Storage**: `commitment_id` is stored in `CommitmentMetadata` within each `CommitmentNFT`
- **Validation**: Only checks length (1-256 chars) via `is_valid_commitment_id()`
- **No Uniqueness Check**: Multiple commitments can have the same `commitment_id`
- **No Query Function**: No way to directly retrieve a commitment by its `commitment_id`
- **Token ID Counter**: Uses `TokenCounter` as a monotonically increasing counter

### Data Structures Involved

```rust
CommitmentMetadata {
    commitment_id: String,  // Currently accepts any non-empty string ≤ 256 chars
    duration_days: u32,
    max_loss_percent: u32,
    commitment_type: String,
    created_at: u64,
    expires_at: u64,
    initial_amount: i128,
    asset_address: Address,
}

CommitmentNFT {
    owner: Address,
    token_id: u32,           // Already unique (monotonic counter)
    metadata: CommitmentMetadata,
    is_active: bool,
    early_exit_penalty: u32,
}
```

---

## Design Decision: Commitment ID Format

### Chosen Approach: `Prefix + Counter`

- **Format**: `COMMIT_{counter}` where `counter` is a u32 monotonically increasing counter
- **Examples**: `COMMIT_0`, `COMMIT_1`, `COMMIT_2`, etc.
- **Benefits**:
  - Simple and deterministic
  - Guaranteed uniqueness by design
  - Consistent format (fixed charset + numeric)
  - Easy to validate regex: `^COMMIT_\d+$`
  - Aligns with existing `TokenCounter` pattern

### Alternative Approaches Considered (Not Chosen)

- **Hash-based** (e.g., sha256): More flexible user input, but harder to validate and verify
- **UUID-based**: Complex for on-chain generation, Soroban SDK limitations
- **User-provided + uniqueness check**: Flexible but requires storage lookup, higher gas costs

### Why NOT Use User-Provided IDs

The task description mentions `commitment_id` should be "unique per commitment," but doesn't specify whether users provide it or the contract generates it. Since:

1. User-provided IDs require a lookup table (extra storage)
2. The existing code accepts `commitment_id` as a function parameter
3. The contract already uses auto-generated `token_id`

We'll **generate the `commitment_id` automatically** based on the `TokenCounter`, ensuring uniqueness by design while keeping the API backwards compatible (the caller can still pass a suggested ID, but we override it).

---

## Implementation Steps

### Phase 1: Storage Changes (No Breaking Changes)

#### 1.1 Add New Storage Keys

**File**: `contracts/commitment_nft/src/lib.rs`

Add to `DataKey` enum:

```rust
/// Mapping from commitment_id to token_id for reverse lookup
CommitmentIdIndex(String),
```

**Rationale**: Enables efficient lookup of commitments by their `commitment_id`

#### 1.2 Add Error Types (Already Exists)

- `InvalidCommitmentId` (error code 21) already exists

#### 1.3 Add Validation Function

**Current**: Already has `is_valid_commitment_id()` that checks length
**Enhancement**: Add format validation for `COMMIT_{counter}` pattern

```rust
fn is_valid_commitment_id_format(commitment_id: &String) -> bool {
    // Must match format: COMMIT_<digits>
    // Implementation: Parse prefix and counter
}
```

---

### Phase 2: Modify `mint()` Function

#### 2.1 Generate Commitment ID Automatically

**Location**: Lines 310-330 in `lib.rs`

**Changes**:

- Extract `token_id` earlier
- Generate `commitment_id` as `COMMIT_{token_id}`
- Validate format
- Check uniqueness via `CommitmentIdIndex`

**Code Pattern**:

```rust
// AFTER line 417 (Generate unique token_id):
let token_id: u32 = /* existing code */;

// NEW: Generate commitment_id based on token_id
let generated_commitment_id = format_commitment_id(&e, token_id);

// NEW: Register in index
e.storage().persistent().set(
    &DataKey::CommitmentIdIndex(generated_commitment_id.clone()),
    &token_id
);
```

#### 2.2 Update Metadata Creation

**Location**: Lines 430-438

Use generated `commitment_id` instead of user-provided one:

```rust
let metadata = CommitmentMetadata {
    commitment_id: generated_commitment_id,  // Use generated ID
    duration_days,
    max_loss_percent,
    commitment_type,
    created_at,
    expires_at,
    initial_amount,
    asset_address,
};
```

---

### Phase 3: Add Query Function

#### 3.1 New Function: `get_commitment_by_id()`

**Location**: After `get_metadata()` function (around line 505)

**Signature**:

```rust
pub fn get_commitment_by_id(
    e: Env,
    commitment_id: String,
) -> Result<CommitmentNFT, ContractError> {
    // Lookup token_id from CommitmentIdIndex
    let token_id = e.storage().persistent()
        .get(&DataKey::CommitmentIdIndex(commitment_id))
        .ok_or(ContractError::TokenNotFound)?;

    // Get NFT by token_id
    e.storage().persistent()
        .get(&DataKey::NFT(token_id))
        .ok_or(ContractError::TokenNotFound)
}
```

---

### Phase 4: Test Implementation

#### 4.1 Test File Location

**File**: `contracts/commitment_nft/src/tests.rs`

#### 4.2 Test Cases

**Test 1**: `test_commitment_id_uniqueness`

```rust
#[test]
fn test_commitment_id_uniqueness() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint first commitment
    let token_id_1 = client.mint(
        &owner,
        &String::from_str(&e, "ignored_id_1"),  // Will be overridden
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // Mint second commitment
    let token_id_2 = client.mint(
        &owner,
        &String::from_str(&e, "ignored_id_2"),  // Will be overridden
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    // Verify tokens are different
    assert_ne!(token_id_1, token_id_2);

    // Verify commitment_ids are different
    let metadata1 = client.get_metadata(token_id_1).unwrap();
    let metadata2 = client.get_metadata(token_id_2).unwrap();
    assert_ne!(metadata1.metadata.commitment_id, metadata2.metadata.commitment_id);

    // Verify they follow the expected format
    assert_eq!(metadata1.metadata.commitment_id, String::from_str(&e, "COMMIT_0"));
    assert_eq!(metadata2.metadata.commitment_id, String::from_str(&e, "COMMIT_1"));
}
```

**Test 2**: `test_commitment_id_format_consistency`

```rust
#[test]
fn test_commitment_id_format_consistency() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint multiple commitments
    for i in 0..5 {
        let token_id = client.mint(
            &owner,
            &String::from_str(&e, &format!("id_{}", i)),
            &30,
            &10,
            &String::from_str(&e, "balanced"),
            &1000,
            &asset_address,
            &5,
        );

        let metadata = client.get_metadata(token_id).unwrap();
        let expected_id = String::from_str(&e, &format!("COMMIT_{}", i));
        assert_eq!(metadata.metadata.commitment_id, expected_id);

        // Verify format consistency
        assert!(is_valid_commitment_id_format(&metadata.metadata.commitment_id));
    }
}
```

**Test 3**: `test_get_commitment_by_id`

```rust
#[test]
fn test_get_commitment_by_id() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);
    let owner = Address::generate(&e);
    let asset_address = Address::generate(&e);

    client.initialize(&admin);

    // Mint two commitments
    let token_id_1 = client.mint(
        &owner,
        &String::from_str(&e, "any_id_1"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &1000,
        &asset_address,
        &5,
    );

    let token_id_2 = client.mint(
        &owner,
        &String::from_str(&e, "any_id_2"),
        &30,
        &10,
        &String::from_str(&e, "balanced"),
        &2000,  // Different amount
        &asset_address,
        &5,
    );

    // Get first commitment by ID
    let commitment_id_1 = String::from_str(&e, "COMMIT_0");
    let nft1 = client.get_commitment_by_id(commitment_id_1).unwrap();
    assert_eq!(nft1.token_id, token_id_1);
    assert_eq!(nft1.metadata.initial_amount, 1000);

    // Get second commitment by ID
    let commitment_id_2 = String::from_str(&e, "COMMIT_1");
    let nft2 = client.get_commitment_by_id(commitment_id_2).unwrap();
    assert_eq!(nft2.token_id, token_id_2);
    assert_eq!(nft2.metadata.initial_amount, 2000);

    // Verify they are different
    assert_ne!(nft1.metadata.commitment_id, nft2.metadata.commitment_id);
}
```

**Test 4**: `test_get_commitment_by_invalid_id_fails`

```rust
#[test]
#[should_panic(expected = "Error(Contract, #3)")]  // TokenNotFound
fn test_get_commitment_by_invalid_id_fails() {
    let e = Env::default();
    let (admin, client) = setup_contract(&e);

    client.initialize(&admin);

    // Try to get non-existent commitment
    let invalid_id = String::from_str(&e, "COMMIT_999");
    client.get_commitment_by_id(invalid_id).unwrap();
}
```

---

## Acceptance Criteria Mapping

| Requirement                                                              | Implementation                            | Test Case                               |
| ------------------------------------------------------------------------ | ----------------------------------------- | --------------------------------------- |
| Two create_commitment calls produce different commitment_ids             | Auto-generated IDs using token_id counter | `test_commitment_id_uniqueness`         |
| commitment_id format is consistent                                       | Format: `COMMIT_{counter}`, validated     | `test_commitment_id_format_consistency` |
| get_commitment(id1) and get_commitment(id2) return different commitments | New `get_commitment_by_id()` function     | `test_get_commitment_by_id`             |
| Uniqueness and format are tested                                         | All above test cases                      | All test cases                          |

---

## Files to Modify

1. **`contracts/commitment_nft/src/lib.rs`**
   - Add `CommitmentIdIndex` storage key
   - Add `format_commitment_id()` helper function
   - Modify `mint()` to generate and register commitment_id
   - Add `get_commitment_by_id()` query function

2. **`contracts/commitment_nft/src/tests.rs`**
   - Add 4 new test functions
   - Test uniqueness
   - Test format consistency
   - Test retrieval by ID

---

## Implementation Order

1. Add storage key to `DataKey` enum
2. Add helper function to format commitment_id
3. Modify `mint()` to generate and register commitment_id
4. Add `get_commitment_by_id()` function
5. Add all test cases
6. Run `cargo test` to verify
7. Run `cargo build` to ensure no compilation errors

---

## Breaking Changes

**Minor**: The `mint()` function will now **ignore** the user-provided `commitment_id` parameter and generate one automatically. This is acceptable because:

1. The task requires uniqueness enforcement
2. Auto-generation guarantees uniqueness
3. The parameter is still accepted (backwards compatible API)
4. Tests will validate the auto-generated ID is returned in metadata

---

## Gas Optimization Notes

- Index lookup uses persistent storage (already keyed by string)
- Token ID to commitment_id mapping is bidirectional but efficient
- Format validation is simple string comparison (no hashing needed)

---

## Future Enhancements

1. Allow custom ID formats per business logic
2. Support batch commitment creation with custom IDs
3. Add commitmentIdExists() utility function
4. Add migration path for existing contracts with legacy commitment_ids
