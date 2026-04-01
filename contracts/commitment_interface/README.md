# Integration Guide: Commitment Interface

This guide documents the interface-only ABI exported by `commitment_interface`.
As of interface version `2`, it mirrors the live `commitment_core` commitment
schema, key read-only getters, and event payload types so downstream bindings
can detect drift before deployment.

---

## 1. Interface Overview

The `CommitmentInterface` provides a standardized ABI surface for the live
commitment contracts on the Soroban network.

### Metadata & Constants

* **Interface Version:** `2`
* **Event Symbols:** `created`, `settled`, `exited`

### Function Signatures

| Function | Arguments | Return Type | Description |
|:---------|:----------|:------------|:------------|
| `initialize` | `env: Env, admin: Address, nft_contract: Address` | `Result<(), Error>` | Initializes admin and linked NFT contract. |
| `create_commitment` | `env: Env, owner: Address, amount: i128, asset_address: Address, rules: CommitmentRules` | `Result<String, Error>` | Creates a commitment and returns its string id. |
| `get_commitment` | `env: Env, commitment_id: String` | `Result<Commitment, Error>` | Fetches the full live commitment record. |
| `list_commitments_by_owner` | `env: Env, owner: Address` | `Result<Vec<String>, Error>` | Alias for owner-indexed commitment lookup used by UIs and indexers. |
| `get_owner_commitments` | `env: Env, owner: Address` | `Result<Vec<String>, Error>` | Lists commitment ids owned by an address. |
| `get_total_commitments` | `env: Env` | `Result<u64, Error>` | Reads the global commitment counter. |
| `get_total_value_locked` | `env: Env` | `Result<i128, Error>` | Reads total value locked across active commitments. |
| `get_commitments_created_between` | `env: Env, from_ts: u64, to_ts: u64` | `Result<Vec<String>, Error>` | Reads commitment ids created in an inclusive time range. |
| `get_admin` | `env: Env` | `Result<Address, Error>` | Reads the configured core-contract admin. |
| `get_nft_contract` | `env: Env` | `Result<Address, Error>` | Reads the linked NFT contract address. |
| `settle` | `env: Env, commitment_id: String` | `Result<(), Error>` | Settles an expired commitment. |
| `early_exit` | `env: Env, commitment_id: String, caller: Address` | `Result<(), Error>` | Exits an active commitment early. |

### Data Structures (Rust)

```rust
pub struct CommitmentRules {
    pub duration_days: u32,
    pub max_loss_percent: u32,
    pub commitment_type: String,
    pub early_exit_penalty: u32,
    pub min_fee_threshold: i128,
    pub grace_period_days: u32,
}

pub struct Commitment {
    pub commitment_id: String,
    pub owner: Address,
    pub nft_token_id: u32,
    pub rules: CommitmentRules,
    pub amount: i128,
    pub asset_address: Address,
    pub created_at: u64,
    pub expires_at: u64,
    pub current_value: i128,
    pub status: String,
}

pub struct CommitmentCreatedEvent {
    pub commitment_id: String,
    pub owner: Address,
    pub amount: i128,
    pub asset_address: Address,
    pub nft_token_id: u32,
    pub rules: CommitmentRules,
    pub timestamp: u64,
}

pub struct CommitmentSettledEvent {
    pub commitment_id: String,
    pub owner: Address,
    pub settlement_amount: i128,
    pub timestamp: u64,
}
```

---

## 2. Frontend Integration (TypeScript)

The TypeScript bindings are located in the root `/bindings` directory.

### Build Workflow

Before use, the definitions must be compiled into JavaScript:

```bash
cd bindings
npm install
npm run build
```

### Usage Example

```typescript
import { Contract, Networks } from '../bindings'; 

const contract = new Contract({
  networkPassphrase: Networks.Testnet, 
  rpcUrl: 'https://soroban-testnet.stellar.org',
});

// Example: Calling get_commitment
async function checkCommitment(commitment_id: string) {
  try {
    const commitment = await contract.get_commitment({ commitment_id });
    console.log('Commitment Details:', commitment);
  } catch (err) {
    console.error("Error fetching commitment:", err);
  }
}
```

---

## 3. Error Reference

Integration errors return a `u32` code mapped to the following definitions:

| Code | Name | Meaning | Recommended Action |
|:-----|:-----|:--------|:-------------------|
| 1 | `NotFound` | Requested resource does not exist. | Verify the commitment id exists via `get_commitment`. |
| 2 | `Unauthorized` | Caller failed an authorization check. | Ensure the transaction is signed by the correct address. |
| 3 | `AlreadyInitialized` | `initialize` called more than once. | Check contract state before initialization. |
| 4+ | `Validation / state errors` | Live contracts may reject invalid amounts, durations, or states. | Follow the core contract error surface for runtime handling. |

---

## 4. Maintenance & Synchronization

To keep the interface aligned with live contracts:

1. Update the interface crate types and signatures.
2. Run the drift checks:

   ```bash
   cargo test -p commitment_interface
   ```

   These tests compare source-defined `CommitmentRules`, `Commitment`,
   `CommitmentCreatedEvent`, and `CommitmentSettledEvent` structs against
   `commitment_core`, and verify the live core contract still exports the
   expected public signatures mirrored by this ABI crate.

3. Build WASM if bindings need regeneration:
   ```bash
   stellar contract build
   ```

4. Sync bindings if you publish them from this repo:
   ```bash
   stellar contract bindings typescript \
     --wasm target/wasm32v1-none/release/commitment_interface.wasm \
     --output-dir bindings \
     --overwrite
   ```

5. Rebuild Types:
   ```bash
   cd bindings && npm run build
   ```

---

## Additional Resources

- [Soroban Documentation](https://soroban.stellar.org/docs)
- [Stellar CLI Reference](https://developers.stellar.org/docs/tools/developer-tools)

---
